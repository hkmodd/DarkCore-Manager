import { useEffect, useState } from "react";
import { VaultService, VaultGame } from "../../services/VaultService";
import { motion, AnimatePresence } from "framer-motion";
import { Save, RefreshCw, Shield, AlertTriangle, Clock } from "lucide-react";
import clsx from "clsx";

export function VaultPanel() {
    const [games, setGames] = useState<VaultGame[]>([]);
    const [loading, setLoading] = useState(false);
    const [status, setStatus] = useState<string | null>(null);



    const loadGames = async () => {
        setLoading(true);
        try {
            const data = await VaultService.listGames();
            setGames(data);
        } catch (e) {
            console.error(e);
            setStatus(`Error loading vault: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadGames();
    }, []);

    const handleRestore = async (game: VaultGame) => {
        if (!confirm(`Restore ${game.name} logic? This will overwrite current Steam manifests.`)) return;
        setStatus(`Restoring ${game.name}...`);
        try {
            const res = await VaultService.restoreGame(game.app_id);
            setStatus(`Success: ${res}`);
        } catch (e) {
            setStatus(`Restore Failed: ${e}`);
        }
    };

    return (
        <div className="p-6 space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-3xl font-bold tracking-tight text-white flex items-center gap-3">
                        <Shield className="w-8 h-8 text-cyan-400" />
                        SECURE VAULT
                    </h2>
                    <p className="text-zinc-400 mt-1">Offline Backup & Restore System</p>
                </div>
                <button
                    onClick={loadGames}
                    className="p-2 bg-zinc-800/50 hover:bg-zinc-700/50 rounded-lg border border-zinc-700 transition-colors"
                >
                    <RefreshCw className={clsx("w-5 h-5 text-cyan-400", loading && "animate-spin")} />
                </button>
            </div>

            {status && (
                <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="p-4 bg-zinc-900/80 border border-cyan-500/30 rounded-lg text-cyan-300 flex items-center gap-3"
                >
                    <AlertTriangle className="w-5 h-5" />
                    {status}
                </motion.div>
            )}

            {loading && games.length === 0 ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    {[1, 2, 3].map((i) => (
                        <div key={i} className="h-32 bg-zinc-800/30 animate-pulse rounded-lg border border-zinc-800" />
                    ))}
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    <AnimatePresence>
                        {games.map((game) => (
                            <motion.div
                                key={game.app_id}
                                layout
                                initial={{ opacity: 0, scale: 0.9 }}
                                animate={{ opacity: 1, scale: 1 }}
                                exit={{ opacity: 0, scale: 0.9 }}
                                className="group relative overflow-hidden bg-zinc-900/40 border border-zinc-800 hover:border-cyan-500/50 rounded-xl transition-all duration-300"
                            >
                                {/* Cyberpunk Glow */}
                                <div className="absolute inset-0 bg-gradient-to-br from-cyan-500/5 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />

                                <div className="p-5 relative z-10">
                                    <div className="flex justify-between items-start mb-4">
                                        <div className="w-10 h-10 rounded-lg bg-zinc-800 flex items-center justify-center border border-zinc-700 font-mono text-xs text-zinc-400">
                                            {game.app_id.substring(0, 3)}..
                                        </div>
                                        <div className="flex gap-2">
                                            <button
                                                onClick={() => handleRestore(game)}
                                                title="Restore Backup"
                                                className="p-2 hover:bg-cyan-500/20 rounded-lg text-zinc-400 hover:text-cyan-400 transition-colors"
                                            >
                                                <RefreshCw className="w-4 h-4" />
                                            </button>
                                        </div>
                                    </div>

                                    <h3 className="text-lg font-bold text-white truncate mb-1">{game.name}</h3>
                                    <div className="flex items-center gap-4 text-xs text-zinc-500 font-mono">
                                        <span className="flex items-center gap-1">
                                            <Save className="w-3 h-3" />
                                            {game.size_gb}
                                        </span>
                                        <span className="flex items-center gap-1">
                                            <Clock className="w-3 h-3" />
                                            {new Date(game.timestamp * 1000).toLocaleDateString()}
                                        </span>
                                    </div>

                                    <div className="mt-4 pt-4 border-t border-zinc-800/50 flex gap-2">
                                        <button
                                            onClick={() => handleRestore(game)}
                                            className="flex-1 py-2 bg-zinc-800 hover:bg-cyan-600/20 hover:text-cyan-400 border border-zinc-700 hover:border-cyan-500/50 rounded-lg text-xs font-bold transition-all"
                                        >
                                            RESTORE
                                        </button>
                                    </div>
                                </div>
                            </motion.div>
                        ))}
                    </AnimatePresence>
                </div>
            )}

            {games.length === 0 && !loading && (
                <div className="text-center py-20 text-zinc-500">
                    <Shield className="w-12 h-12 mx-auto mb-4 opacity-20" />
                    <p>Vault is empty. Backup some games!</p>
                </div>
            )}
        </div>
    );
}
