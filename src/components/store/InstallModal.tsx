import { useState } from "react";
import { motion } from "framer-motion";
import { X, Download, HardDrive, Loader2, ArrowLeft } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { DlcPickerModal } from "./DlcPickerModal";
import { useDownload } from "../../context/DownloadContext";

interface InstallModalProps {
    appId: string;
    appName: string;
    onClose: () => void;
}

export function InstallModal({ appId, appName, onClose }: InstallModalProps) {
    const { addToQueue } = useDownload();

    // Steps: "dlc_pick" -> "method" -> "library_pick"
    const [step, setStep] = useState<"dlc_pick" | "method" | "library_pick">("dlc_pick");
    const [selectedDlcs, setSelectedDlcs] = useState<string[]>([]);

    // Library State
    const [libraries, setLibraries] = useState<string[]>([]);
    const [selectedLibrary, setSelectedLibrary] = useState<string>("");
    const [loadingLibs, setLoadingLibs] = useState(false);

    // Method State
    const [selectedMethod, setSelectedMethod] = useState<"steam" | "direct" | null>(null);
    const [godmode, setGodmode] = useState(false);

    // =========================================================================
    // HANDLERS
    // =========================================================================

    const fetchLibraries = async () => {
        setLoadingLibs(true);
        try {
            const libs = await invoke<string[]>("get_library_folders");
            setLibraries(libs);
            if (libs.length > 0) setSelectedLibrary(libs[0]);
        } catch (e) {
            console.error("Failed to fetch libraries", e);
        } finally {
            setLoadingLibs(false);
        }
    };

    const handleMethodSelect = (method: "steam" | "direct") => {
        setSelectedMethod(method);
        setStep("library_pick");
        fetchLibraries();
    };

    const handleInstall = () => {
        if (!selectedLibrary || !selectedMethod) return;

        // Add to Global Queue
        addToQueue({
            appId,
            appName,
            type: selectedMethod,
            libraryPath: selectedLibrary,
            selectedDlcs,
        });

        // Close Modal
        onClose();
        // Ideally show a toast here, but we'll rely on the global widget for now.
    };

    // =========================================================================
    // RENDER STEPS
    // =========================================================================

    // 1. DLC Picker (First Step)
    if (step === "dlc_pick") {
        return (
            <DlcPickerModal
                appId={appId}
                appName={appName}
                onConfirm={(dlcIds) => {
                    setSelectedDlcs(dlcIds);
                    setStep("method");
                }}
                onCancel={onClose}
            />
        );
    }

    // 2. Library Selection (Final Step)
    if (step === "library_pick") {
        return (
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
                <motion.div
                    initial={{ opacity: 0, scale: 0.95 }}
                    animate={{ opacity: 1, scale: 1 }}
                    className="bg-zinc-900 border border-zinc-700 w-full max-w-lg rounded-2xl shadow-2xl p-6"
                >
                    <div className="flex justify-between items-center mb-6">
                        <div className="flex items-center gap-3">
                            <button
                                onClick={() => setStep("method")}
                                className="p-2 -ml-2 text-zinc-500 hover:text-white hover:bg-zinc-800 rounded-full transition-colors"
                            >
                                <ArrowLeft className="w-5 h-5" />
                            </button>
                            <h2 className="text-2xl font-bold text-white">Select Library</h2>
                        </div>
                        <button onClick={onClose} className="text-zinc-500 hover:text-white"><X /></button>
                    </div>

                    {loadingLibs ? (
                        <div className="flex justify-center p-8"><Loader2 className="animate-spin text-cyan-400" /></div>
                    ) : (
                        <div className="space-y-3">
                            <p className="text-zinc-400 text-sm mb-4">Choose where to install (Steam Library):</p>
                            {libraries.map((lib, idx) => (
                                <div
                                    key={idx}
                                    onClick={() => setSelectedLibrary(lib)}
                                    className={`p-4 rounded-xl border cursor-pointer transition-all flex items-center gap-3 ${selectedLibrary === lib
                                        ? "bg-cyan-500/10 border-cyan-500 text-white"
                                        : "bg-zinc-800 border-zinc-700 text-zinc-300 hover:border-zinc-500"
                                        }`}
                                >
                                    <HardDrive className={`w-5 h-5 ${selectedLibrary === lib ? "text-cyan-400" : "text-zinc-500"}`} />
                                    <div className="flex-1 overflow-hidden">
                                        <div className="font-mono text-sm truncate">{lib}</div>
                                    </div>
                                    {selectedLibrary === lib && <div className="w-3 h-3 rounded-full bg-cyan-500 shadow-[0_0_10px_cyan]" />}
                                </div>
                            ))}
                        </div>
                    )}

                    <button
                        onClick={handleInstall}
                        disabled={!selectedLibrary}
                        className={`w-full mt-6 py-3 rounded-xl font-bold transition-all ${selectedLibrary
                            ? "bg-cyan-500 hover:bg-cyan-400 text-black shadow-[0_0_20px_rgba(6,182,212,0.3)]"
                            : "bg-zinc-800 text-zinc-500 cursor-not-allowed"
                            }`}
                    >
                        Add to Queue ({selectedMethod === "steam" ? "Unlock & Play" : "Direct Download"})
                    </button>
                </motion.div>
            </div>
        );
    }

    // 3. Method Selection (Middle Step)
    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="bg-zinc-900 border border-zinc-700 w-full max-w-lg rounded-2xl shadow-2xl p-6"
            >
                {/* Header with Back Button */}
                <div className="flex justify-between items-center mb-6">
                    <div className="flex items-center gap-3">
                        <button
                            onClick={() => setStep("dlc_pick")}
                            className="p-2 -ml-2 text-zinc-500 hover:text-white hover:bg-zinc-800 rounded-full transition-colors"
                        >
                            <ArrowLeft className="w-5 h-5" />
                        </button>
                        <div>
                            <h2 className="text-2xl font-bold text-white">Select Method</h2>
                            <p className="text-xs text-zinc-400">{selectedDlcs.length} DLCs Selected</p>
                        </div>
                    </div>
                    <button onClick={onClose} className="text-zinc-500 hover:text-white"><X /></button>
                </div>

                <div className="grid gap-4">
                    {/* Steam Method */}
                    <div
                        onClick={() => handleMethodSelect("steam")}
                        className="bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 p-4 rounded-xl text-left group transition-all hover:border-cyan-500 cursor-pointer"
                    >
                        <div className="flex items-center gap-3 mb-2">
                            <div className="p-2 bg-blue-500/20 text-blue-400 rounded-lg group-hover:bg-blue-500 group-hover:text-white transition-colors">
                                <Download className="w-6 h-6" />
                            </div>
                            <span className="font-bold text-white text-lg">Unlock & Play (Steam)</span>
                        </div>
                        <p className="text-zinc-400 text-sm">Unlocks the game via GreenLuma and launches Steam install.</p>

                        {/* Godmode Toggle (Visual Only for now as we just queue) */}
                        <div
                            className="mt-3 flex items-center gap-2 p-2 bg-black/20 rounded-lg border border-white/5 hover:bg-black/40 transition-colors"
                            onClick={(e) => {
                                e.stopPropagation();
                                setGodmode(!godmode);
                            }}
                        >
                            <div className={`w-5 h-5 rounded border flex items-center justify-center transition-colors ${godmode ? 'bg-cyan-500 border-cyan-500 text-black' : 'bg-zinc-800 border-zinc-600'} `}>
                                {godmode && <Download className="w-3 h-3" />}
                            </div>
                            <div className="text-sm text-zinc-300 pointer-events-none">
                                <span className="font-bold text-cyan-400">Enable Godmode?</span> (Claim Ownership)
                            </div>
                        </div>
                    </div>

                    {/* Direct Method */}
                    <button
                        onClick={() => handleMethodSelect("direct")}
                        className="bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 p-4 rounded-xl text-left group transition-all hover:border-cyan-500"
                    >
                        <div className="flex items-center gap-3 mb-2">
                            <div className="p-2 bg-purple-500/20 text-purple-400 rounded-lg group-hover:bg-purple-500 group-hover:text-white transition-colors">
                                <HardDrive className="w-6 h-6" />
                            </div>
                            <span className="font-bold text-white text-lg">Download Files (Direct)</span>
                        </div>
                        <p className="text-zinc-400 text-sm">Downloads manifests directly to disk. Ideal for backups/offline.</p>
                    </button>
                </div>
            </motion.div>
        </div>
    );
}
