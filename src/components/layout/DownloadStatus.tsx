import { useEffect, useState } from "react";
import { useDownload } from "../../context/DownloadContext";
import { motion, AnimatePresence } from "framer-motion";
import { Pause, Play, Download } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

export function DownloadStatus() {
    const { activeItem } = useDownload();
    const [paused, setPaused] = useState(false);

    // Handle Paused state visual
    useEffect(() => {
        if (activeItem?.status === "Paused") {
            setPaused(true);
        } else {
            setPaused(false);
        }
    }, [activeItem?.status]);

    // PROTECTIVE GUARD: If no active item, do not render.
    if (!activeItem) return null;

    const handlePauseResume = async () => {
        if (paused) {
            await invoke("resume_download");
            // Optimistic update handled by context poll
        } else {
            await invoke("pause_download");
        }
    };

    return (
        <AnimatePresence>
            {activeItem && (
                <motion.div
                    initial={{ y: 100, opacity: 0 }}
                    animate={{ y: 0, opacity: 1 }}
                    exit={{ y: 100, opacity: 0 }}
                    className="fixed bottom-12 right-6 z-50 w-80 bg-black/90 backdrop-blur-xl border border-neon-cyan/30 rounded-lg shadow-[0_0_20px_rgba(0,243,255,0.2)] overflow-hidden"
                >
                    {/* Header */}
                    <div className="flex items-center justify-between px-4 py-2 bg-white/5 border-b border-white/10">
                        <div className="flex items-center gap-2 overflow-hidden">
                            <Download className="w-4 h-4 text-neon-cyan animate-pulse" />
                            <span className="text-xs font-bold text-white truncate max-w-[150px]">
                                {activeItem.appName}
                            </span>
                        </div>
                        <div className="flex items-center gap-1">
                            <button onClick={handlePauseResume} className="p-1 hover:bg-white/10 rounded text-neon-cyan transition-colors">
                                {paused ? <Play className="w-4 h-4" /> : <Pause className="w-4 h-4" />}
                            </button>
                            {/* <button onClick={handleCancel} className="p-1 hover:bg-red-500/20 rounded text-red-500 transition-colors">
                                <X className="w-4 h-4" />
                             </button> */}
                        </div>
                    </div>

                    {/* Body */}
                    <div className="p-4 space-y-3">
                        <div className="flex justify-between text-[10px] text-gray-400 font-mono">
                            <span>{activeItem.status}</span>
                            <span className="text-neon-cyan">{activeItem.speed}</span>
                        </div>

                        {/* Progress Bar */}
                        <div className="h-1.5 w-full bg-white/10 rounded-full overflow-hidden relative">
                            <motion.div
                                className="absolute top-0 left-0 bottom-0 bg-neon-cyan shadow-[0_0_10px_rgba(0,243,255,0.8)]"
                                initial={{ width: 0 }}
                                animate={{ width: `${activeItem.progress}%` }}
                                transition={{ type: "tween", ease: "linear", duration: 0.5 }}
                            />
                        </div>

                        <div className="flex justify-end text-[10px] font-bold text-white">
                            {Math.round(activeItem.progress)}%
                        </div>
                    </div>
                </motion.div>
            )}
        </AnimatePresence>
    );
}
