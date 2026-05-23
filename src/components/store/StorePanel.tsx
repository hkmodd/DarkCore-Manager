import { useState, useCallback } from "react";
import { Search, Globe, FileArchive } from "lucide-react";
import { GameCard } from "./GameCard";
import { ApiService, SearchResult } from "../../services/ApiService";
import { debounce } from "lodash";
import { motion, AnimatePresence } from "framer-motion";
import { InstallModal } from "./InstallModal";
import { ImportModal } from "../tools/ImportModal";

export function StorePanel() {

    const [query, setQuery] = useState("");
    const [results, setResults] = useState<SearchResult[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState("");
    const [installModal, setInstallModal] = useState<{ id: string; name: string } | null>(null);
    const [showImportModal, setShowImportModal] = useState(false);
    const [showFreeContent, setShowFreeContent] = useState(true);

    // Debounced Search
    const performSearch = useCallback(
        debounce(async (q: string) => {
            if (q.length < 2) return;
            setLoading(true);
            setError("");
            try {
                // invoke("search_games", { query: q })
                const data = await ApiService.searchGames(q);
                setResults(data);
            } catch (err: any) {
                console.error(err);
                const errMsg = typeof err === 'string' ? err : err?.message || "Unknown error";
                if (errMsg.startsWith("INVALID_KEY:")) {
                    setError(errMsg.replace("INVALID_KEY: ", ""));
                } else if (errMsg.startsWith("RATE_LIMIT:")) {
                    setError(errMsg.replace("RATE_LIMIT: ", ""));
                } else if (errMsg.startsWith("NETWORK_ERROR:")) {
                    setError(errMsg.replace("NETWORK_ERROR: ", ""));
                } else if (errMsg.startsWith("NOT_CONFIGURED:")) {
                    setError(errMsg.replace("NOT_CONFIGURED: ", ""));
                } else {
                    setError("Search failed. Check API configuration.");
                }
                setResults([]);
            } finally {
                setLoading(false);
            }
        }, 500),
        []
    );

    const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const val = e.target.value;
        setQuery(val);
        performSearch(val);
    };

    return (
        <div className="h-full flex flex-col p-6 max-w-7xl mx-auto w-full relative">
            {/* Header */}
            <div className="flex flex-col md:flex-row items-center justify-between gap-4 mb-8">
                <div>
                    <h1 className="text-3xl font-bold text-white flex items-center gap-3">
                        <Globe className="w-8 h-8 text-cyan-400" />
                        Install Games
                    </h1>
                    <p className="text-zinc-400 mt-1">Search the Steam Catalog or import manifests directly</p>
                </div>

                <button
                    onClick={() => setShowImportModal(true)}
                    className="bg-orange-600 hover:bg-orange-500 text-white px-6 py-2.5 rounded-xl font-bold transition-all flex items-center gap-2 shadow-lg shadow-orange-900/20 hover:scale-105 active:scale-95"
                >
                    <FileArchive className="w-5 h-5" />
                    IMPORT ZIP
                </button>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-hidden flex flex-col">
                <motion.div
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="flex flex-col h-full"
                >
                    {/* Search Bar */}
                    <div className="relative mb-6 flex gap-3 items-center">
                        <div className="relative flex-1">
                            <Search className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 text-zinc-500" />
                            <input
                                type="text"
                                placeholder="Search for a game..."
                                value={query}
                                onChange={handleSearchChange}
                                className="w-full bg-zinc-900/50 border border-zinc-700 focus:border-cyan-500 rounded-xl pl-12 pr-4 py-4 text-white text-lg outline-none transition-all shadow-inner"
                                autoFocus
                            />
                            {loading && (
                                <div className="absolute right-4 top-1/2 -translate-y-1/2">
                                    <div className="w-5 h-5 border-2 border-cyan-500 border-t-transparent rounded-full animate-spin" />
                                </div>
                            )}
                        </div>
                        {/* Free Content Filter */}
                        <label className="flex items-center gap-2 cursor-pointer shrink-0 bg-zinc-900/50 border border-zinc-700 rounded-xl px-4 py-4 hover:border-zinc-600 transition-colors">
                            <input
                                type="checkbox"
                                checked={showFreeContent}
                                onChange={(e) => setShowFreeContent(e.target.checked)}
                                className="accent-cyan-500 w-4 h-4"
                            />
                            <span className="text-xs text-zinc-400 font-bold uppercase tracking-wider whitespace-nowrap">Free</span>
                        </label>
                    </div>

                    {/* Error Message */}
                    {error && (
                        <div className={`p-4 rounded-lg mb-6 text-sm flex items-center gap-2 ${error.includes("Rate limit") ? "bg-amber-500/10 border border-amber-500/20 text-amber-400" :
                            error.includes("API key") || error.includes("Settings") ? "bg-red-500/10 border border-red-500/20 text-red-400" :
                                error.includes("connection") ? "bg-orange-500/10 border border-orange-500/20 text-orange-400" :
                                    "bg-red-500/10 border border-red-500/20 text-red-400"
                            }`}>
                            <span>{error.includes("Rate limit") ? "⏳" : error.includes("connection") ? "🔌" : "⚠️"}</span> {error}
                        </div>
                    )}

                    {/* Results Grid */}
                    <div className="flex-1 overflow-y-auto pr-2 custom-scrollbar">
                        {results.length === 0 && !loading && query.length > 2 ? (
                            <div className="flex flex-col items-center justify-center h-64 text-zinc-500">
                                <Search className="w-12 h-12 mb-4 opacity-20" />
                                <p>No results found for "{query}"</p>
                            </div>
                        ) : (
                            <div className="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4 pb-20">
                                {results.filter(g => showFreeContent || !g.is_free).map((game) => (
                                    <GameCard
                                        key={getId(game.app_id)}
                                        appId={getId(game.app_id)}
                                        name={game.game_name || game.name || "Unknown"}
                                        isFree={game.is_free}
                                        tinyImage={game.tiny_image}
                                        onInstall={() => setInstallModal({ id: getId(game.app_id), name: game.game_name || game.name || "Unknown" })}
                                    />
                                ))}
                            </div>
                        )}

                        {results.length === 0 && query.length < 2 && (
                            <div className="flex flex-col items-center justify-center h-full text-zinc-600">
                                <Globe className="w-16 h-16 mb-4 opacity-10" />
                                <p className="text-lg font-medium">Enter a game name to search</p>
                                <p className="text-sm opacity-50">Uses Steam Store API (Free)</p>
                            </div>
                        )}
                    </div>
                </motion.div>
            </div>

            <AnimatePresence>
                {installModal && (
                    <InstallModal
                        appId={installModal.id}
                        appName={installModal.name}
                        onClose={() => setInstallModal(null)}
                    />
                )}
                {showImportModal && (
                    <ImportModal onClose={() => setShowImportModal(false)} />
                )}
            </AnimatePresence>
        </div>
    );
}

// Helper to handle mixed ID types from API (Value enum)
function getId(id: any): string {
    if (typeof id === 'string') return id;
    if (typeof id === 'number') return id.toString();
    if (id && typeof id === 'object' && id.String) return id.String; // Handle obscure json cases
    return String(id);
}
