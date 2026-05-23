import { useState, useRef, useCallback, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Download, Copy, ExternalLink } from "lucide-react";
import { ApiService, GameDetails } from "../../services/ApiService";

interface GameCardProps {
    appId: string;
    name: string;
    isFree: boolean;
    tinyImage?: string;
    onInstall: () => void;
}

// Module-level cache for game details to avoid re-fetching
const detailsCache: Record<string, GameDetails> = {};

export function GameCard({ appId, name, isFree, tinyImage, onInstall }: GameCardProps) {
    const coverUrl = `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/library_600x900.jpg`;
    const fallbackUrl = tinyImage || `https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`;
    const [imgSrc, setImgSrc] = useState(coverUrl);
    const [imgFailed, setImgFailed] = useState(false);

    // Hover detail popup state
    const [showDetails, setShowDetails] = useState(false);
    const [details, setDetails] = useState<GameDetails | null>(null);
    const [loadingDetails, setLoadingDetails] = useState(false);
    const hoverTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
    const cardRef = useRef<HTMLDivElement>(null);

    // Context menu state
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

    // Shine effect
    const [mousePos, setMousePos] = useState({ x: 0, y: 0 });

    const handleMouseMove = useCallback((e: React.MouseEvent) => {
        if (!cardRef.current) return;
        const rect = cardRef.current.getBoundingClientRect();
        setMousePos({
            x: ((e.clientX - rect.left) / rect.width) * 100,
            y: ((e.clientY - rect.top) / rect.height) * 100,
        });
    }, []);

    const handleMouseEnter = useCallback(() => {
        hoverTimeout.current = setTimeout(async () => {
            setShowDetails(true);
            if (detailsCache[appId]) {
                setDetails(detailsCache[appId]);
                return;
            }
            setLoadingDetails(true);
            try {
                const d = await ApiService.getGameDetails(appId);
                detailsCache[appId] = d;
                setDetails(d);
            } catch {
                // Silent fail — popup just won't show details
            } finally {
                setLoadingDetails(false);
            }
        }, 350);
    }, [appId]);

    const handleMouseLeave = useCallback(() => {
        if (hoverTimeout.current) clearTimeout(hoverTimeout.current);
        setShowDetails(false);
        setContextMenu(null);
    }, []);

    const handleContextMenu = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        setContextMenu({ x: e.clientX, y: e.clientY });
    }, []);

    // Close context menu on click outside
    useEffect(() => {
        if (!contextMenu) return;
        const close = () => setContextMenu(null);
        window.addEventListener("click", close);
        return () => window.removeEventListener("click", close);
    }, [contextMenu]);

    const copyAppId = () => {
        navigator.clipboard.writeText(appId);
        setContextMenu(null);
    };

    const openSteamStore = () => {
        window.open(`https://store.steampowered.com/app/${appId}`, "_blank");
        setContextMenu(null);
    };

    return (
        <motion.div
            ref={cardRef}
            layout
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            whileHover={{ scale: 1.05 }}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
            className="group relative rounded-xl overflow-hidden cursor-pointer h-full flex flex-col"
            onMouseMove={handleMouseMove}
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
            onContextMenu={handleContextMenu}
        >
            {/* Cover Image — Vertical (2:3 ratio like Steam library) */}
            <div className="relative aspect-[2/3] bg-zinc-950 overflow-hidden rounded-xl border border-zinc-800 group-hover:border-cyan-500/40 transition-all duration-300 group-hover:shadow-[0_0_30px_rgba(0,243,255,0.15)]">
                <img
                    src={imgSrc}
                    alt={name}
                    loading="lazy"
                    className="w-full h-full object-cover transition-transform duration-700 group-hover:scale-105"
                    onError={() => {
                        if (!imgFailed) {
                            setImgFailed(true);
                            setImgSrc(fallbackUrl);
                        }
                    }}
                />

                {/* Gradient Overlay */}
                <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent opacity-60 group-hover:opacity-80 transition-opacity duration-300" />

                {/* Shine/Reflection Effect */}
                <div
                    className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
                    style={{
                        background: `radial-gradient(circle at ${mousePos.x}% ${mousePos.y}%, rgba(255,255,255,0.15) 0%, transparent 50%)`,
                    }}
                />

                {/* Neon Edge Glow on Hover */}
                <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 pointer-events-none rounded-xl border border-cyan-400/30 shadow-[inset_0_0_20px_rgba(0,243,255,0.1)]" />

                {/* Price Tag */}
                <div className="absolute top-2 right-2 z-10">
                    <span className={`px-2 py-1 text-[9px] font-black uppercase rounded-md backdrop-blur-sm ${
                        isFree
                            ? "bg-green-500/30 text-green-300 border border-green-500/40"
                            : "bg-blue-500/30 text-blue-300 border border-blue-500/40"
                    }`}>
                        {isFree ? "Free" : "Paid"}
                    </span>
                </div>

                {/* Bottom Info Overlay (always visible) */}
                <div className="absolute bottom-0 left-0 right-0 p-3 z-10">
                    <h3 className="font-bold text-white text-sm leading-tight line-clamp-2 drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]">
                        {name}
                    </h3>
                    <div className="text-[10px] text-zinc-400 font-mono mt-1 drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]">
                        {appId}
                    </div>
                </div>

                {/* INSTALL Button Overlay — visible on hover */}
                <div className="absolute inset-0 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all duration-300 z-20">
                    <button
                        onClick={(e) => { e.stopPropagation(); onInstall(); }}
                        className="bg-cyan-600/90 hover:bg-cyan-500 text-white text-xs font-black py-2.5 px-6 rounded-lg flex items-center gap-2 transition-all shadow-[0_0_25px_rgba(0,243,255,0.4)] hover:shadow-[0_0_35px_rgba(0,243,255,0.6)] backdrop-blur-sm border border-cyan-400/30 hover:scale-105 active:scale-95"
                    >
                        <Download className="w-4 h-4" />
                        INSTALL
                    </button>
                </div>
            </div>

            {/* Hover Detail Popup */}
            <AnimatePresence>
                {showDetails && (details || loadingDetails) && (
                    <motion.div
                        initial={{ opacity: 0, y: 5, scale: 0.95 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 5, scale: 0.95 }}
                        transition={{ duration: 0.15 }}
                        className="absolute left-0 right-0 top-full mt-2 z-50 bg-zinc-900/95 backdrop-blur-xl border border-cyan-500/20 rounded-xl p-3 shadow-[0_10px_40px_rgba(0,0,0,0.8)] pointer-events-none"
                    >
                        {loadingDetails && !details ? (
                            <div className="flex items-center gap-2 text-xs text-zinc-500">
                                <div className="w-3 h-3 border border-cyan-500 border-t-transparent rounded-full animate-spin" />
                                Loading...
                            </div>
                        ) : details ? (
                            <div className="space-y-2">
                                {/* Metacritic + Reviews */}
                                <div className="flex items-center gap-2">
                                    {details.metacritic_score != null && (
                                        <span className={`text-xs font-black px-2 py-0.5 rounded ${
                                            details.metacritic_score >= 75 ? "bg-green-600 text-white" :
                                            details.metacritic_score >= 50 ? "bg-yellow-600 text-white" :
                                            "bg-red-600 text-white"
                                        }`}>
                                            {details.metacritic_score}
                                        </span>
                                    )}
                                    {details.recommendations != null && (
                                        <span className="text-[10px] text-zinc-400">
                                            {details.recommendations > 1000000
                                                ? `${(details.recommendations / 1000000).toFixed(1)}M reviews`
                                                : details.recommendations > 1000
                                                ? `${(details.recommendations / 1000).toFixed(0)}K reviews`
                                                : `${details.recommendations} reviews`}
                                        </span>
                                    )}
                                </div>

                                {/* Platforms */}
                                <div className="flex gap-1">
                                    {details.platforms[0] && <span className="text-[9px] font-bold bg-zinc-800 text-zinc-300 px-1.5 py-0.5 rounded">WIN</span>}
                                    {details.platforms[1] && <span className="text-[9px] font-bold bg-zinc-800 text-zinc-300 px-1.5 py-0.5 rounded">MAC</span>}
                                    {details.platforms[2] && <span className="text-[9px] font-bold bg-zinc-800 text-zinc-300 px-1.5 py-0.5 rounded">LNX</span>}
                                    {details.required_age > 0 && (
                                        <span className="text-[9px] font-bold bg-red-900/50 text-red-400 px-1.5 py-0.5 rounded">
                                            {details.required_age}+
                                        </span>
                                    )}
                                </div>

                                {/* Developer */}
                                {details.developers.length > 0 && (
                                    <div className="text-[10px] text-zinc-500">
                                        <span className="text-zinc-600">DEV </span>
                                        <span className="text-zinc-300">{details.developers[0]}</span>
                                    </div>
                                )}

                                {/* Genres */}
                                {details.genres.length > 0 && (
                                    <div className="flex flex-wrap gap-1">
                                        {details.genres.slice(0, 4).map((g, i) => (
                                            <span key={i} className="text-[9px] bg-cyan-500/10 text-cyan-400/80 px-1.5 py-0.5 rounded-full border border-cyan-500/10">
                                                {g}
                                            </span>
                                        ))}
                                    </div>
                                )}

                                {/* Description */}
                                {details.short_description && (
                                    <p className="text-[10px] text-zinc-400 line-clamp-2 leading-relaxed">
                                        {details.short_description.replace(/<[^>]*>/g, '')}
                                    </p>
                                )}
                            </div>
                        ) : null}
                    </motion.div>
                )}
            </AnimatePresence>

            {/* Context Menu */}
            <AnimatePresence>
                {contextMenu && (
                    <motion.div
                        initial={{ opacity: 0, scale: 0.9 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.9 }}
                        className="fixed z-[100] bg-zinc-900/95 backdrop-blur-xl border border-zinc-700 rounded-lg shadow-[0_10px_40px_rgba(0,0,0,0.8)] py-1 min-w-[160px]"
                        style={{ left: contextMenu.x, top: contextMenu.y }}
                    >
                        <button
                            onClick={(e) => { e.stopPropagation(); onInstall(); setContextMenu(null); }}
                            className="w-full px-4 py-2 text-left text-sm text-white hover:bg-cyan-500/20 flex items-center gap-2 transition-colors"
                        >
                            <Download className="w-3.5 h-3.5 text-cyan-400" /> Install
                        </button>
                        <button
                            onClick={(e) => { e.stopPropagation(); openSteamStore(); }}
                            className="w-full px-4 py-2 text-left text-sm text-white hover:bg-cyan-500/20 flex items-center gap-2 transition-colors"
                        >
                            <ExternalLink className="w-3.5 h-3.5 text-zinc-400" /> View on Steam
                        </button>
                        <button
                            onClick={(e) => { e.stopPropagation(); copyAppId(); }}
                            className="w-full px-4 py-2 text-left text-sm text-white hover:bg-cyan-500/20 flex items-center gap-2 transition-colors"
                        >
                            <Copy className="w-3.5 h-3.5 text-zinc-400" /> Copy AppID
                        </button>
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
}
