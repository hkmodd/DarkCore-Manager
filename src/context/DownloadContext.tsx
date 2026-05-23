import { createContext, useContext, useState, useEffect, useRef, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// Types
export interface DownloadItem {
    id: string; // Unique Queue ID
    appId: string;
    appName: string;
    type: "steam" | "direct";
    status: string;
    progress: number;
    speed: string;
    libraryPath?: string;
    selectedDlcs?: string[];
    addedAt: number;
}

interface DownloadContextType {
    queue: DownloadItem[];
    activeItem: DownloadItem | null;
    addToQueue: (game: Omit<DownloadItem, "id" | "status" | "progress" | "speed" | "addedAt">) => void;
    removeFromQueue: (id: string) => void;
    isDownloading: boolean;
    clearCompleted: () => void;
}

const DownloadContext = createContext<DownloadContextType | null>(null);

export function useDownload() {
    const context = useContext(DownloadContext);
    if (!context) throw new Error("useDownload must be used within DownloadProvider");
    return context;
}

export function DownloadProvider({ children }: { children: ReactNode }) {
    const [queue, setQueue] = useState<DownloadItem[]>([]);
    const [activeItemId, setActiveItemId] = useState<string | null>(null);
    const [isProcessing, setIsProcessing] = useState(false);

    // Derived State: The Single Source of Truth
    const activeItem = queue.find(i => i.id === activeItemId) || null;

    // Refs for listeners/timers to clean up
    const unlistenRef = useRef<UnlistenFn | null>(null);
    const pollTimerRef = useRef<number | null>(null); // Use number for window.setInterval

    // ===================================
    // 1. Queue Management
    // ===================================

    const addToQueue = (game: Omit<DownloadItem, "id" | "status" | "progress" | "speed" | "addedAt">) => {
        const newItem: DownloadItem = {
            ...game,
            id: crypto.randomUUID(),
            status: "Queued",
            progress: 0,
            speed: "0 B/s",
            addedAt: Date.now(),
        };
        setQueue(prev => [...prev, newItem]);
    };

    const removeFromQueue = (id: string) => {
        if (activeItemId === id) {
            // Cannot remove active item yet
            return;
        }
        setQueue(prev => prev.filter(item => item.id !== id));
    };

    const clearCompleted = () => {
        setQueue(prev => prev.filter(item => item.status !== "Completed" && item.status !== "Error"));
    };

    // ===================================
    // 2. Process Loop
    // ===================================

    useEffect(() => {
        // If not processing and we have items in queue
        if (!isProcessing && !activeItemId && queue.length > 0) {
            const next = queue.find(i => i.status === "Queued");
            if (next) {
                startDownload(next);
            }
        }
    }, [queue, isProcessing, activeItemId]);

    const startDownload = async (item: DownloadItem) => {
        setIsProcessing(true);
        setActiveItemId(item.id);

        // Update item in queue to "Initializing"
        updateItemStatus(item.id, "Initializing...", 0);

        try {
            if (item.type === "steam") {
                await startSteamInstall(item);
            } else {
                await startDirectInstall(item);
            }
        } catch (e) {
            const msg = `Failed to start: ${e}`;
            updateItemStatus(item.id, msg, 0, "Error");
            setIsProcessing(false);
            setActiveItemId(null);
        }
    };

    // PURE UPDATER - No side effects!
    const updateItemStatus = (id: string, status: string, progress: number, forceStatus?: string, speed?: string) => {
        setQueue(prev => prev.map(i => {
            if (i.id === id) {
                return {
                    ...i,
                    status: forceStatus || status,
                    progress,
                    speed: speed || i.speed
                };
            }
            return i;
        }));
    };

    // ===================================
    // 3. Steam Install Logic
    // ===================================
    const startSteamInstall = async (item: DownloadItem) => {
        // 1. Listen
        if (unlistenRef.current) {
            const unlisten = unlistenRef.current;
            try { await unlisten(); } catch { }
            unlistenRef.current = null;
        }

        const unlisten = await listen<{ step: string; message: string; progress: number }>("install-progress", (event) => {
            const { step, message, progress } = event.payload;
            updateItemStatus(item.id, message, progress * 100);

            if (step === "done" || (progress >= 1.0 && step !== "error")) {
                finalize(item.id, "Completed");
            } else if (step === "error") {
                finalize(item.id, "Error");
            }
        });
        unlistenRef.current = unlisten;

        // 2. Invoke
        await invoke("steam_protocol_install", {
            appid: item.appId,
            name: item.appName,
            libraryPath: item.libraryPath || "",
            installDir: null,
            selectedDlcs: item.selectedDlcs || [],
        });
    };

    // ===================================
    // 4. Direct Install Logic
    // ===================================
    const startDirectInstall = async (item: DownloadItem) => {
        await invoke("start_direct_download", {
            appId: item.appId,
            gameName: item.appName,
            libraryPath: item.libraryPath || "",
            userSelectedIds: item.selectedDlcs || []
        });

        // 2. Poll
        if (pollTimerRef.current) window.clearInterval(pollTimerRef.current);

        pollTimerRef.current = window.setInterval(async () => {
            try {
                const status: any = await invoke("get_download_status");

                // Only update if it matches our current game (backend global state)
                if (status.game_id === item.appId) {
                    if (status.status.startsWith("Downloading") || status.status === "Paused") {
                        updateItemStatus(item.id, status.status, status.progress_val, undefined, status.speed);
                    } else if (status.status === "Completed") {
                        updateItemStatus(item.id, "Download Complete", 100);
                        window.clearInterval(pollTimerRef.current!);
                        finalize(item.id, "Completed");
                    } else if (status.status.startsWith("Error")) {
                        updateItemStatus(item.id, status.status, 0, "Error");
                        window.clearInterval(pollTimerRef.current!);
                        finalize(item.id, "Error");
                    } else {
                        updateItemStatus(item.id, status.status, status.progress_val);
                    }
                }
            } catch (e) {
                console.error("Poll error", e);
            }
        }, 1000);
    };

    const finalize = (id: string, finalStatus: string) => {
        updateItemStatus(id, finalStatus, finalStatus === "Completed" ? 100 : 0);
        setIsProcessing(false);
        setActiveItemId(null);
        if (unlistenRef.current) {
            const u = unlistenRef.current;
            try { u(); } catch { }
            unlistenRef.current = null;
        }
        if (pollTimerRef.current) { window.clearInterval(pollTimerRef.current); pollTimerRef.current = null; }

        // Refresh Library
        invoke("get_active_games").catch(console.error);
    };

    return (
        <DownloadContext.Provider value={{
            queue,
            activeItem,
            addToQueue,
            removeFromQueue,
            isDownloading: isProcessing,
            clearCompleted
        }}>
            {children}
        </DownloadContext.Provider>
    );
}
