import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import { X, FileArchive, Download, HardDrive, Loader2, AlertCircle, CheckCircle } from "lucide-react";

interface ImportModalProps {
    onClose: () => void;
}

interface ImportMetadata {
    script_data?: {
        app_id?: number;
        app_name?: string;
        depots: any[];
    };
    manifest_count: number;
    depot_count: number;
    file_path: string;
}

export function ImportModal({ onClose }: ImportModalProps) {
    const [step, setStep] = useState<"select" | "review" | "importing" | "done">("select");
    const [path, setPath] = useState("");
    const [metadata, setMetadata] = useState<ImportMetadata | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState("");
    const [status, setStatus] = useState("");

    const handleBrowse = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Zip Archives', extensions: ['zip'] }]
            });
            if (selected && typeof selected === "string") {
                setPath(selected);
                scanZip(selected);
            }
        } catch (e) {
            console.error(e);
        }
    };

    const scanZip = async (filePath: string) => {
        setLoading(true);
        setError("");
        try {
            const data = await invoke<ImportMetadata>("scan_zip_for_import", { path: filePath });
            setMetadata(data);
            setStep("review");
        } catch (e) {
            setError(`Failed to scan ZIP: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const handleImport = async (method: "steam" | "direct") => {
        if (!metadata) return;
        setLoading(true);
        setStep("importing");
        setStatus(method === "steam" ? "Adding to AppList & Launching Steam..." : "Extracting Manifests & Adding to AppList...");

        try {
            const msg = await invoke<string>("import_zip_action", {
                path: metadata.file_path,
                method
            });
            setStatus(msg);
            setStep("done");
        } catch (e) {
            setError(`Import failed: ${e}`);
            setStep("review"); // Go back to review on error
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="bg-zinc-900 border border-zinc-700 w-full max-w-lg rounded-2xl shadow-2xl p-6"
            >
                <div className="flex justify-between items-center mb-6">
                    <h2 className="text-2xl font-bold text-white flex items-center gap-2">
                        <FileArchive className="text-orange-500" /> Import Package
                    </h2>
                    <button onClick={onClose} className="text-zinc-500 hover:text-white"><X /></button>
                </div>

                {step === "select" && (
                    <div className="flex flex-col items-center justify-center py-12 border-2 border-dashed border-zinc-800 rounded-xl hover:border-zinc-600 transition-colors cursor-pointer" onClick={handleBrowse}>
                        <FileArchive className="w-16 h-16 text-zinc-600 mb-4" />
                        <p className="text-zinc-400 font-bold">Click to select a .zip file</p>
                        <p className="text-zinc-600 text-sm mt-1">Supports Morrenus Packs (.lua + .manifest)</p>
                        {loading && <Loader2 className="w-6 h-6 animate-spin mt-4 text-orange-500" />}
                    </div>
                )}

                {step === "review" && metadata && (
                    <div className="space-y-6">
                        <div className="bg-zinc-800/50 p-4 rounded-xl border border-zinc-700">
                            <h3 className="text-lg font-bold text-white mb-1">
                                {metadata.script_data?.app_name || "Unknown Game"}
                            </h3>
                            <div className="flex flex-col gap-1 text-xs text-zinc-400 font-mono">
                                <div className="flex gap-4">
                                    <span>AppID: {metadata.script_data?.app_id || "???"}</span>
                                    <span>Depots: {metadata.depot_count}</span>
                                    <span>Manifests: {metadata.manifest_count}</span>
                                </div>
                                <div className="truncate text-zinc-600" title={path}>{path}</div>
                            </div>
                        </div>

                        <div className="grid gap-3">
                            <button
                                onClick={() => handleImport("steam")}
                                className="bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 p-4 rounded-xl text-left group transition-all hover:border-blue-500"
                            >
                                <div className="flex items-center gap-3 mb-1">
                                    <div className="p-2 bg-blue-500/20 text-blue-400 rounded-lg group-hover:bg-blue-500 group-hover:text-white transition-colors">
                                        <Download className="w-5 h-5" />
                                    </div>
                                    <span className="font-bold text-white">Unlock & Steam Install</span>
                                </div>
                                <p className="text-zinc-400 text-xs">Adds to AppList and triggers Steam download.</p>
                            </button>

                            <button
                                onClick={() => handleImport("direct")}
                                className="bg-zinc-800 hover:bg-zinc-700 border border-zinc-700 p-4 rounded-xl text-left group transition-all hover:border-purple-500"
                            >
                                <div className="flex items-center gap-3 mb-1">
                                    <div className="p-2 bg-purple-500/20 text-purple-400 rounded-lg group-hover:bg-purple-500 group-hover:text-white transition-colors">
                                        <HardDrive className="w-5 h-5" />
                                    </div>
                                    <span className="font-bold text-white">Direct Import</span>
                                </div>
                                <p className="text-zinc-400 text-xs">Extracts manifests directly to valid depotcache. No download needed if pack is complete.</p>
                            </button>
                        </div>
                    </div>
                )}

                {step === "importing" && (
                    <div className="flex flex-col items-center justify-center py-12 space-y-4">
                        <Loader2 className="w-12 h-12 text-cyan-400 animate-spin" />
                        <p className="text-zinc-400 text-center animate-pulse">{status}</p>
                    </div>
                )}

                {step === "done" && (
                    <div className="flex flex-col items-center justify-center py-8 space-y-6">
                        <CheckCircle className="w-16 h-16 text-green-500" />
                        <div className="text-center">
                            <h3 className="text-xl font-bold text-white mb-2">Import Successful!</h3>
                            <p className="text-zinc-400 text-sm">{status}</p>
                        </div>
                        <button
                            onClick={onClose}
                            className="bg-zinc-800 hover:bg-zinc-700 text-white px-8 py-2 rounded-lg font-bold"
                        >
                            Close
                        </button>
                    </div>
                )}

                {error && (
                    <div className="mt-4 p-4 bg-red-900/20 border border-red-900/50 rounded-lg flex items-center gap-3 text-red-200">
                        <AlertCircle className="w-5 h-5 flex-shrink-0" />
                        <p className="text-sm">{error}</p>
                    </div>
                )}
            </motion.div>
        </div>
    );
}
