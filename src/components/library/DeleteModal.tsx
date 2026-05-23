import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { Trash2, Unlink, AlertTriangle, Shield, X, Loader2 } from "lucide-react";

interface DeleteResult {
    backed_up: number;
    children_removed: string[];
    vdf_keys_removed: number;
    files_deleted: boolean;
}

interface DeleteModalProps {
    isOpen: boolean;
    onClose: () => void;
    onDeleted: () => void;
    gameId: string;
    gameName: string;
}

export function DeleteModal({ isOpen, onClose, onDeleted, gameId, gameName }: DeleteModalProps) {
    const [children, setChildren] = useState<[string, string][]>([]);
    const [scanning, setScanning] = useState(true);
    const [deleting, setDeleting] = useState(false);
    const [result, setResult] = useState<DeleteResult | null>(null);
    const [error, setError] = useState("");
    const [confirmWipe, setConfirmWipe] = useState(false);

    useEffect(() => {
        if (isOpen && gameId) {
            setScanning(true);
            setResult(null);
            setError("");
            setConfirmWipe(false);
            invoke<[string, string][]>("scan_delete_children", { parentId: gameId })
                .then(setChildren)
                .catch(() => setChildren([]))
                .finally(() => setScanning(false));
        }
    }, [isOpen, gameId]);

    const handleDelete = async (mode: "unlink" | "full_wipe") => {
        setDeleting(true);
        setError("");
        try {
            const childrenIds = children.map(([id]) => id);
            const res = await invoke<DeleteResult>("full_delete_game", {
                appId: gameId,
                childrenIds,
                mode,
            });
            setResult(res);
            // Give the user time to see the result, then close
            setTimeout(() => {
                onDeleted();
                onClose();
            }, 2000);
        } catch (e) {
            setError(String(e));
        } finally {
            setDeleting(false);
        }
    };

    if (!isOpen) return null;

    return (
        <AnimatePresence>
            <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center"
                onClick={onClose}
            >
                <motion.div
                    initial={{ opacity: 0, y: 20, scale: 0.95 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 20, scale: 0.95 }}
                    className="bg-zinc-900 border border-zinc-700 rounded-xl w-full max-w-lg mx-4 overflow-hidden"
                    onClick={(e) => e.stopPropagation()}
                >
                    {/* Header */}
                    <div className="p-5 border-b border-zinc-800 flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-red-500/10 rounded-lg">
                                <Trash2 className="w-5 h-5 text-red-500" />
                            </div>
                            <div>
                                <h3 className="text-white font-bold text-lg">DELETE: {gameName}</h3>
                                <p className="text-zinc-500 text-xs font-mono">ID: {gameId}</p>
                            </div>
                        </div>
                        <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    {/* Content */}
                    <div className="p-5 space-y-4">
                        {/* Success Result */}
                        {result && (
                            <motion.div
                                initial={{ opacity: 0, scale: 0.95 }}
                                animate={{ opacity: 1, scale: 1 }}
                                className="p-4 bg-green-500/10 border border-green-500/30 rounded-lg space-y-1"
                            >
                                <p className="text-green-400 font-bold text-sm">✓ Delete Complete</p>
                                {result.backed_up > 0 && (
                                    <p className="text-xs text-zinc-400">Vault backup: {result.backed_up} manifests saved</p>
                                )}
                                {result.children_removed.length > 0 && (
                                    <p className="text-xs text-zinc-400">{result.children_removed.length} child entries removed</p>
                                )}
                                {result.vdf_keys_removed > 0 && (
                                    <p className="text-xs text-zinc-400">{result.vdf_keys_removed} VDF keys cleaned</p>
                                )}
                                {result.files_deleted && (
                                    <p className="text-xs text-zinc-400">Game files deleted</p>
                                )}
                            </motion.div>
                        )}

                        {/* Error */}
                        {error && (
                            <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
                                <p className="text-red-400 text-sm">{error}</p>
                            </div>
                        )}

                        {/* DLC Scan Results */}
                        {!result && (
                            <>
                                {scanning ? (
                                    <div className="flex items-center gap-2 text-zinc-400">
                                        <Loader2 className="w-4 h-4 animate-spin" />
                                        <span className="text-sm">Scanning for associated DLCs...</span>
                                    </div>
                                ) : children.length > 0 ? (
                                    <div className="space-y-2">
                                        <p className="text-xs text-zinc-400 uppercase tracking-wider font-bold">
                                            Associated Content ({children.length} items)
                                        </p>
                                        <div className="max-h-32 overflow-y-auto space-y-1 bg-black/30 rounded-lg p-2 border border-zinc-800">
                                            {children.map(([id, name]) => (
                                                <div key={id} className="flex items-center justify-between text-xs py-1 px-2">
                                                    <span className="text-zinc-300 truncate">{name}</span>
                                                    <span className="text-zinc-600 font-mono ml-2">{id}</span>
                                                </div>
                                            ))}
                                        </div>
                                        <p className="text-xs text-yellow-500">
                                            These will also be removed from AppList.
                                        </p>
                                    </div>
                                ) : (
                                    <p className="text-xs text-zinc-500">No associated DLCs or depots found.</p>
                                )}

                                {/* Vault Safety Note */}
                                <div className="flex items-start gap-2 p-3 bg-cyan-500/5 border border-cyan-500/20 rounded-lg">
                                    <Shield className="w-4 h-4 text-cyan-500 shrink-0 mt-0.5" />
                                    <p className="text-xs text-zinc-400">
                                        Manifests will be backed up to <span className="text-cyan-400 font-mono">Vault/</span> before deletion for safe recovery.
                                    </p>
                                </div>
                            </>
                        )}
                    </div>

                    {/* Actions */}
                    {!result && (
                        <div className="p-5 border-t border-zinc-800 space-y-3">
                            {/* UNLINK (Safe) */}
                            <button
                                onClick={() => handleDelete("unlink")}
                                disabled={deleting || scanning}
                                className="w-full flex items-center justify-center gap-2 py-3 px-4 bg-cyan-500/10 border border-cyan-500/30 rounded-lg text-cyan-400 hover:bg-cyan-500/20 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                            >
                                {deleting ? (
                                    <Loader2 className="w-4 h-4 animate-spin" />
                                ) : (
                                    <Unlink className="w-4 h-4" />
                                )}
                                <span className="font-bold text-sm">UNLINK ID (SAFE)</span>
                                <span className="text-xs text-cyan-600 ml-2">Removes from AppList only — game files stay</span>
                            </button>

                            {/* FULL WIPE (Destructive) */}
                            {!confirmWipe ? (
                                <button
                                    onClick={() => setConfirmWipe(true)}
                                    disabled={deleting || scanning}
                                    className="w-full flex items-center justify-center gap-2 py-3 px-4 bg-red-500/5 border border-red-500/20 rounded-lg text-red-400/60 hover:bg-red-500/10 hover:text-red-400 hover:border-red-500/40 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                                >
                                    <Trash2 className="w-4 h-4" />
                                    <span className="font-bold text-sm">FULL UNINSTALL (DESTROY)</span>
                                </button>
                            ) : (
                                <motion.div
                                    initial={{ opacity: 0, height: 0 }}
                                    animate={{ opacity: 1, height: "auto" }}
                                    className="space-y-2"
                                >
                                    <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg flex items-start gap-2">
                                        <AlertTriangle className="w-5 h-5 text-red-500 shrink-0" />
                                        <div>
                                            <p className="text-red-400 text-sm font-bold">DESTRUCTIVE ACTION</p>
                                            <p className="text-xs text-zinc-400 mt-1">
                                                This will delete game files, ACF manifests, depot manifests, and VDF keys.
                                                Vault backup is created first for recovery.
                                            </p>
                                        </div>
                                    </div>
                                    <div className="flex gap-2">
                                        <button
                                            onClick={() => setConfirmWipe(false)}
                                            className="flex-1 py-2 px-3 bg-zinc-800 rounded-lg text-zinc-400 hover:bg-zinc-700 text-sm transition-colors"
                                        >
                                            Cancel
                                        </button>
                                        <button
                                            onClick={() => handleDelete("full_wipe")}
                                            disabled={deleting}
                                            className="flex-1 py-2 px-3 bg-red-500/20 border border-red-500/40 rounded-lg text-red-400 font-bold hover:bg-red-500/30 text-sm transition-colors disabled:opacity-40"
                                        >
                                            {deleting ? (
                                                <Loader2 className="w-4 h-4 animate-spin mx-auto" />
                                            ) : (
                                                "CONFIRM FULL WIPE"
                                            )}
                                        </button>
                                    </div>
                                </motion.div>
                            )}
                        </div>
                    )}
                </motion.div>
            </motion.div>
        </AnimatePresence>
    );
}
