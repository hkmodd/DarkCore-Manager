import { ReactNode, useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { Play, Settings, Terminal, Info, Shield, Activity } from "lucide-react";
import { AudioProvider, useAudio } from "../../context/AudioContext";
import { OnboardingWizard } from "../onboarding/OnboardingWizard";
import { DownloadStatus } from "./DownloadStatus";
import { invoke } from "@tauri-apps/api/core";
import logo from "../../assets/logo.png";

export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface AppLayoutProps {
    children: ReactNode;
    activeTab: string;
    onTabChange: (tab: string) => void;
}

export function AppLayout({ children, activeTab, onTabChange }: AppLayoutProps) {
    return (
        <AudioProvider>
            <div className="h-screen w-screen bg-obsidian text-neon-cyan font-mono overflow-hidden flex flex-col relative select-none">
                {/* Global Background Effects - Subtle Grid */}
                <div className="absolute inset-0 pointer-events-none bg-[linear-gradient(rgba(0,243,255,0.02)_1px,transparent_1px),linear-gradient(90deg,rgba(0,243,255,0.02)_1px,transparent_1px)] z-0 bg-[length:30px_30px]"></div>
                <div className="absolute inset-0 pointer-events-none bg-radial-gradient(circle_at_center,transparent_0%,rgba(18,20,28,0.8)_100%) z-0"></div>

                <div className="flex-1 flex overflow-hidden z-10">
                    {/* Sidebar */}
                    <Sidebar activeTab={activeTab} onTabChange={onTabChange} />

                    {/* Main Content Area */}
                    <main className="flex-1 flex flex-col overflow-hidden bg-obsidian/95 backdrop-blur-sm relative border-l border-neon-cyan/5">
                        {/* Top Drag Region (Invisible but functional) */}
                        <div data-tauri-drag-region className="h-6 w-full shrink-0 z-50 absolute top-0 left-0 right-0 cursor-grab active:cursor-grabbing" />

                        {/* Content Scroll Wrapper */}
                        <div className="flex-1 overflow-y-auto custom-scrollbar relative p-0 pt-6">
                            {children}
                        </div>
                    </main>
                </div>

                {/* Global System Log Footer - DISABLED per user request (Duplicate UI) */}
                {/* {activeTab !== 'about' && <SystemLogFooter />} */}

                <DownloadStatus />
                <OnboardingWizard />
            </div>
        </AudioProvider>
    );
}

function Sidebar({ activeTab, onTabChange }: { activeTab: string, onTabChange: (t: string) => void }) {
    const { volume, setVolume, isPlaying, togglePlay } = useAudio();
    const [isCollapsed, setIsCollapsed] = useState(false);

    const navItems = [
        { id: "dashboard", icon: Terminal, label: "Dashboard", desc: "System Status" },
        { id: "install", icon: Play, label: "Install", desc: "Game Catalog" },
        { id: "library", icon: Shield, label: "Library", desc: "Manage Games" },
        { id: "settings", icon: Settings, label: "Settings", desc: "Configuration" },
        { id: "about", icon: Info, label: "About", desc: "Manifesto" },
    ];

    return (
        <motion.aside
            animate={{ width: isCollapsed ? 80 : 280 }}
            className="shrink-0 bg-black/40 backdrop-blur-xl flex flex-col z-20 shadow-[5px_0_30px_rgba(0,0,0,0.5)] relative overflow-hidden border-r border-white/5 transition-all duration-500 ease-spring"
        >
            {/* Glass Reflection */}
            <div className="absolute top-0 left-0 w-full h-full bg-gradient-to-b from-white/5 via-transparent to-transparent pointer-events-none" />

            {/* 1. LOGO AREA - REVISED */}
            <div
                className="h-32 flex flex-col items-center justify-center relative cursor-pointer group"
                onClick={() => setIsCollapsed(!isCollapsed)}
            >
                <motion.div
                    layout
                    className="relative z-10"
                    whileHover={{ scale: 1.1, rotate: isCollapsed ? 0 : 5 }}
                    whileTap={{ scale: 0.95 }}
                >
                    <img
                        src={logo}
                        alt="DarkCore"
                        className="w-16 h-auto object-contain drop-shadow-[0_0_20px_rgba(0,243,255,0.4)] transition-all duration-500"
                    />
                </motion.div>

                {!isCollapsed && (
                    <motion.div
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0 }}
                        className="mt-3 text-center"
                    >
                        <div className="text-[10px] font-black text-white tracking-[0.2em] glitch-text" data-text="DARKCORE">
                            DARKCORE
                        </div>
                        <div className="text-[8px] tracking-[0.3em] font-bold text-neon-pink/80 mt-1">MANAGER v2.0</div>
                    </motion.div>
                )}
            </div>

            {/* 2. NAVIGATION - FLUID HOVER SWITCHING */}
            <nav className="flex-1 flex flex-col gap-2 px-3 py-4 overflow-hidden relative">
                {navItems.map((item) => {
                    const isActive = activeTab === item.id;
                    return (
                        <button
                            key={item.id}
                            onMouseEnter={() => onTabChange(item.id)}
                            onClick={() => onTabChange(item.id)}
                            className={cn(
                                "relative group flex items-center gap-4 px-3 py-3 transition-all duration-300 rounded-xl overflow-hidden",
                                isActive
                                    ? "bg-white/10 shadow-[inner_0_0_20px_rgba(255,255,255,0.05)] border border-white/10"
                                    : "hover:bg-white/5 border border-transparent"
                            )}
                        >
                            {/* Active Glass Glow */}
                            {isActive && (
                                <motion.div
                                    layoutId="activeTabGlow"
                                    className="absolute inset-0 bg-neon-cyan/5 blur-md"
                                />
                            )}

                            {/* Indicator Line */}
                            {isActive && (
                                <motion.div
                                    layoutId="activeTabIndicator"
                                    className="absolute left-0 top-2 bottom-2 w-1 bg-neon-cyan/80 rounded-r-full shadow-[0_0_10px_rgba(0,243,255,0.5)]"
                                />
                            )}

                            <div className={cn("p-2 rounded-lg transition-colors relative z-10", isActive ? "text-neon-cyan" : "text-zinc-500 group-hover:text-zinc-300")}>
                                <item.icon className={cn("w-6 h-6", isActive && "drop-shadow-[0_0_8px_rgba(0,243,255,0.6)]")} />
                            </div>

                            {!isCollapsed && (
                                <motion.div
                                    initial={{ opacity: 0, x: -10 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    className="flex flex-col relative z-10 text-left"
                                >
                                    <span className={cn("text-sm font-bold uppercase tracking-wider transition-colors", isActive ? "text-white" : "text-zinc-500 group-hover:text-zinc-300")}>
                                        {item.label}
                                    </span>
                                    {isActive && (
                                        <motion.span
                                            initial={{ opacity: 0 }}
                                            animate={{ opacity: 1 }}
                                            className="text-[9px] text-neon-cyan/60 uppercase tracking-widest"
                                        >
                                            {item.desc}
                                        </motion.span>
                                    )}
                                </motion.div>
                            )}
                        </button>
                    );
                })}
            </nav>

            {/* 3. AUDIO PLAYER - PREMIUM ANIMATED */}
            <div className="p-4 border-t border-white/5 bg-black/40 backdrop-blur-md relative overflow-hidden group/audio">
                {/* Background Glow */}
                {isPlaying && <div className="absolute inset-0 bg-neon-cyan/5 animate-pulse pointer-events-none" />}

                <div className={cn("flex items-center gap-4 transition-all relative z-10", isCollapsed ? "justify-center" : "")}>

                    {/* Play/Pause Button */}
                    <button
                        onClick={togglePlay}
                        className="w-12 h-12 shrink-0 flex items-center justify-center rounded-full bg-black/50 hover:bg-neon-cyan/20 text-neon-cyan transition-all border border-neon-cyan/30 hover:border-neon-cyan shadow-[0_0_15px_rgba(0,243,255,0.1)] hover:shadow-[0_0_25px_rgba(0,243,255,0.3)]"
                    >
                        {isPlaying ? (
                            <div className="flex gap-1 h-4 items-end">
                                <div className="w-1 bg-neon-cyan rounded-full animate-equalizer" style={{ animationDuration: '0.6s' }}></div>
                                <div className="w-1 bg-neon-cyan rounded-full animate-equalizer" style={{ animationDuration: '0.4s' }}></div>
                                <div className="w-1 bg-neon-cyan rounded-full animate-equalizer" style={{ animationDuration: '0.7s' }}></div>
                            </div>
                        ) : (
                            <span className="text-sm ml-1">▶</span>
                        )}
                    </button>

                    {!isCollapsed && (
                        <div className="flex-1 flex flex-col gap-1.5 overflow-hidden">
                            <div className="flex justify-between items-end px-0.5">
                                <span className={cn(
                                    "text-[10px] font-bold tracking-widest transition-colors",
                                    isPlaying ? "text-neon-cyan animate-pulse" : "text-zinc-600"
                                )}>
                                    {isPlaying ? "SYSTEM AUDIO" : "PAUSED"}
                                </span>
                                <span className="text-[10px] text-neon-cyan font-mono">{Math.round(volume * 100)}%</span>
                            </div>

                            {/* Animated Volume Bar */}
                            <div className={cn(
                                "h-1.5 bg-white/5 rounded-full relative transition-all duration-300",
                                isPlaying ? "group-hover/audio:h-2" : ""
                            )}>
                                {/* Active Fill */}
                                <div
                                    className="absolute top-0 left-0 bottom-0 bg-gradient-to-r from-cyan-600 to-neon-cyan rounded-full shadow-[0_0_10px_rgba(0,243,255,0.5)] transition-all duration-75"
                                    style={{ width: `${volume * 100}%` }}
                                />

                                {/* Glow Head */}
                                <div
                                    className="absolute top-1/2 -translate-y-1/2 w-3 h-3 bg-white rounded-full shadow-[0_0_15px_rgba(255,255,255,0.8)] opacity-0 group-hover/audio:opacity-100 transition-opacity"
                                    style={{ left: `calc(${volume * 100}% - 6px)` }}
                                />

                                {/* Input Range */}
                                <input
                                    type="range" min="0" max="1" step="0.01" value={volume}
                                    onChange={(e) => setVolume(parseFloat(e.target.value))}
                                    className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                                />
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </motion.aside>
    );
}

export function SystemLogFooter() {
    const [latestLog, setLatestLog] = useState<string>("SYSTEM INITIALIZED_");
    const [logHistory, setLogHistory] = useState<string[]>([]);
    const [isExpanded, setIsExpanded] = useState(false);

    useEffect(() => {
        const interval = setInterval(async () => {
            try {
                const logs = await invoke<string[]>("get_logs");
                if (logs.length > 0) {
                    const newest = logs[logs.length - 1]; // Last log is newest in backend push
                    if (newest !== latestLog) {
                        setLatestLog(newest);
                        setLogHistory(logs.slice(-5).reverse()); // Keep last 5, reversed for display
                    }
                }
            } catch (e) { }
        }, 1000);
        return () => clearInterval(interval);
    }, [latestLog]);

    // Parse log Level for Color
    const getLogColor = (log: string) => {
        if (log.includes("[ERROR]") || log.includes("Error") || log.includes("Failed")) return "text-red-500 drop-shadow-[0_0_3px_rgba(239,68,68,0.8)]";
        if (log.includes("[WARNING]") || log.includes("Warn")) return "text-yellow-400";
        if (log.includes("[SUCCESS]") || log.includes("Success")) return "text-green-400 drop-shadow-[0_0_3px_rgba(74,222,128,0.8)]";
        return "text-neon-cyan/70";
    };

    return (
        <div
            className={cn(
                "border-t border-neon-cyan/20 bg-black/80 backdrop-blur-md transition-all duration-300 flex flex-col z-50",
                isExpanded ? "h-48" : "h-8"
            )}
        >
            <div
                className="h-8 flex items-center px-4 gap-4 cursor-pointer hover:bg-white/5 transition-colors"
                onClick={() => setIsExpanded(!isExpanded)}
            >
                <div className="flex items-center gap-2 text-[10px] font-bold text-neon-cyan uppercase tracking-widest shrink-0">
                    <Activity className="w-3 h-3 animate-pulse" />
                    <span>SYSTEM LOG</span>
                </div>

                <div className="flex-1 font-mono text-[10px] truncate flex items-center gap-2">
                    <span className="text-gray-600">LATEST &gt;</span>
                    <span className={cn("truncate", getLogColor(latestLog))}>
                        {latestLog}
                    </span>
                </div>

                <div className="text-[10px] text-gray-500 uppercase tracking-wider">
                    {isExpanded ? "COLLAPSE [-]" : "EXPAND [+]"}
                </div>
            </div>

            {/* Expanded History */}
            <AnimatePresence>
                {isExpanded && (
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        className="flex-1 overflow-y-auto p-2 bg-black/50 font-mono text-[10px] space-y-1 custom-scrollbar"
                    >
                        {logHistory.map((log, i) => (
                            <div key={i} className={cn("border-l-2 pl-2 py-0.5 border-white/10", getLogColor(log).replace("text-", "border-").split(" ")[0])}>
                                <span className="opacity-50 mr-2">[{i}]</span>
                                <span className={getLogColor(log)}>{log}</span>
                            </div>
                        ))}
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}
