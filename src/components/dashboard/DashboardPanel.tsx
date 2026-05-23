import { useEffect, useState } from "react";
import { VaultService } from "../../services/VaultService";
import { ConfigService, AppConfig } from "../../services/ConfigService";
import { Shield, Database, Terminal, Ghost, Zap } from "lucide-react";
import { motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";

export function DashboardPanel() {
    const [stats, setStats] = useState({
        backups: 0,
    });
    const [config, setConfig] = useState<AppConfig | null>(null);

    useEffect(() => {
        loadData();
        invoke("run_startup_scan").catch(console.error);
    }, []);

    const loadData = async () => {
        try {
            const [backups, cfg] = await Promise.all([
                VaultService.listGames(),
                ConfigService.getConfig()
            ]);

            setStats({
                backups: backups.length,
            });
            setConfig(cfg);
        } catch (e) {
            console.error(e);
        }
    };

    return (
        <div className="h-full w-full flex flex-col p-8 gap-8 relative overflow-hidden">
            {/* Cyberpunk Background Elements */}
            <div className="absolute top-0 right-0 w-96 h-96 bg-neon-cyan/5 rounded-full blur-[100px] pointer-events-none" />
            <div className="absolute bottom-0 left-0 w-96 h-96 bg-neon-pink/5 rounded-full blur-[100px] pointer-events-none" />

            {/* Header */}
            <header className="relative z-10 mb-6 select-none">
                <AsciiHeader />

                <div className="flex items-center gap-4 text-neon-cyan/60 font-mono text-sm uppercase tracking-widest pl-2 border-l-2 border-neon-cyan/30 mt-4">
                    <span className="flex items-center gap-2">
                        <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse shadow-[0_0_10px_#22c55e]" />
                        SYSTEM ONLINE
                    </span>
                    <span className="w-px h-4 bg-white/10" />
                    <span>WELCOME BACK, OPERATOR.</span>
                </div>
            </header>

            {/* Main Action Area (The "Protagonists") */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-8 relative z-10 flex-1 min-h-[300px]">
                {/* 1. STEALTH START - HERO CARD */}
                <HeroButton
                    title="STEALTH START"
                    subtitle="INJECT & PLAY"
                    description="Launches Steam with GreenLuma active. Renames processes to evade detection."
                    icon={Ghost}
                    color="green"
                    onClick={() => invoke("launch_greenluma_stealth")}
                />

                {/* 2. CLEAN STEAM - HERO CARD */}
                <HeroButton
                    title="CLEAN STEAM"
                    subtitle="FACTORY RESET"
                    description="Terminates all injection processes. Launches a pristine, vanilla Steam instance."
                    icon={Shield}
                    color="blue"
                    onClick={() => invoke("relaunch_steam")}
                />
            </div>

            {/* Secondary Info Grid */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6 relative z-10 h-40">

                {/* Vault Status */}
                <StatCard
                    icon={Database}
                    label="VAULT STATUS"
                    value={stats.backups.toString()}
                    sub="MANIFESTS SECURED"
                    color="text-fuchsia-400"
                    bg="bg-fuchsia-400/5 hover:bg-fuchsia-400/10"
                    border="border-fuchsia-500/20"
                />

                {/* Security Status */}
                <StatCard
                    icon={Zap}
                    label="SECURITY PROTOCOL"
                    value={config?.enable_stealth_mode ? "ACTIVE" : "STANDBY"}
                    sub={config?.enable_stealth_mode ? "STEALTH ENGAGED" : "STANDARD MODE"}
                    color={config?.enable_stealth_mode ? "text-yellow-400" : "text-zinc-500"}
                    bg={config?.enable_stealth_mode ? "bg-yellow-400/5 hover:bg-yellow-400/10" : "bg-zinc-900"}
                    border="border-yellow-500/20"
                />

                {/* Update Status (New) */}
                <StatCard
                    icon={Database} // Using Database icon as placeholder or maybe RefreshCw if I import it
                    label="SYSTEM VERSION"
                    value="v2.0.0"
                    sub="LATEST BUILD"
                    color="text-cyan-400"
                    bg="bg-cyan-400/5 hover:bg-cyan-400/10"
                    border="border-cyan-500/20"
                />
            </div>
        </div>
    );
}

function HeroButton({ title, subtitle, description, icon: Icon, color, onClick }: any) {
    const isGreen = color === 'green';
    const baseColor = isGreen ? 'text-green-400' : 'text-cyan-400';
    const borderColor = isGreen ? 'border-green-500/30' : 'border-cyan-500/30';
    const glowColor = isGreen ? 'group-hover:shadow-[0_0_50px_rgba(74,222,128,0.2)]' : 'group-hover:shadow-[0_0_50px_rgba(34,211,238,0.2)]';
    const gradient = isGreen ? 'from-green-500/10 to-transparent' : 'from-cyan-500/10 to-transparent';

    return (
        <motion.button
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
            onClick={onClick}
            className={`
                relative h-full flex flex-col items-start justify-between p-8 text-left
                bg-black/60 backdrop-blur-md rounded-3xl border ${borderColor}
                transition-all duration-300 group overflow-hidden ${glowColor}
            `}
        >
            {/* Background Gradient */}
            <div className={`absolute inset-0 bg-gradient-to-br ${gradient} opacity-0 group-hover:opacity-100 transition-opacity duration-500`} />

            {/* Scanline Effect */}
            <div className="absolute inset-0 bg-[linear-gradient(transparent_2px,rgba(0,0,0,0.5)_3px)] bg-[length:4px_4px] opacity-20 pointer-events-none" />

            <div className="relative z-10 w-full">
                <div className="flex justify-between items-start mb-4">
                    <div className={`p-4 rounded-2xl bg-black/50 border ${borderColor} ${baseColor}`}>
                        <Icon className="w-10 h-10" />
                    </div>
                    <div className={`text-6xl font-black opacity-10 font-mono ${baseColor}`}>
                        {title.split(" ")[0].substring(0, 2)}
                    </div>
                </div>

                <h2 className={`text-4xl font-black tracking-tighter italic mb-1 text-white group-hover:text-white/90`}>
                    {title}
                </h2>
                <div className={`text-sm font-bold tracking-[0.3em] uppercase ${baseColor} mb-6`}>
                    {subtitle}
                </div>

                <p className="text-zinc-400 text-sm max-w-sm leading-relaxed border-l-2 border-white/10 pl-4">
                    {description}
                </p>
            </div>

            <div className="relative z-10 w-full flex justify-end mt-4">
                <div className={`px-6 py-2 rounded-full border ${borderColor} bg-black/50 text-[10px] uppercase tracking-widest font-bold text-white group-hover:bg-white group-hover:text-black transition-all flex items-center gap-2`}>
                    <span>INITIATE SEQUENCE</span>
                    <Terminal className="w-3 h-3" />
                </div>
            </div>
        </motion.button>
    );
}

function StatCard({ icon: Icon, label, value, sub, color, bg, border }: any) {
    return (
        <div className={`${bg} border ${border} rounded-2xl p-6 flex items-center gap-5 transition-all group`}>
            <div className={`${color} p-3 rounded-xl bg-black/20`}>
                <Icon className="w-8 h-8" />
            </div>
            <div>
                <div className="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-1">{label}</div>
                <div className={`text-3xl font-black text-white mb-0.5 tracking-tight`}>{value}</div>
                <div className={`text-[10px] font-mono ${color} uppercase opacity-80`}>{sub}</div>
            </div>
        </div>
    );
}

function AsciiHeader() {
    return (
        <div className="relative">
            {/* Base Layer */}
            <motion.div
                initial={{ opacity: 0, filter: "blur(10px)" }}
                animate={{ opacity: 1, filter: "blur(0px)" }}
                transition={{ duration: 1 }}
                className="font-mono text-[5px] xs:text-[6px] sm:text-[8px] md:text-[10px] lg:text-[11px] text-cyan-400 whitespace-pre leading-none select-none drop-shadow-[0_0_15px_rgba(34,211,238,0.5)] relative z-10"
            >
                {`
██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝██╔════╝██╔══██╗██╔══██╗██╔════╝
██║  ██║███████║██████╔╝█████╔╝ ██║     ██║  ██║██████╔╝█████╗  
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██║     ██║  ██║██╔══██╗██╔══╝  
██████╔╝██║  ██║██║  ██║██║  ██╗╚██████╗██████╔╝██║  ██║███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═════╝ ╚═╝  ╚═╝╚══════╝
`}
            </motion.div>

            {/* Glitch Layer (Red/Blue Offset) */}
            <motion.div
                animate={{
                    opacity: [0, 0, 0.8, 0, 0],
                    x: [0, 2, -2, 0],
                    y: [0, -1, 1, 0]
                }}
                transition={{
                    duration: 0.2,
                    repeat: Infinity,
                    repeatDelay: 5,
                    repeatType: "reverse"
                }}
                className="absolute inset-0 font-mono text-[5px] xs:text-[6px] sm:text-[8px] md:text-[10px] lg:text-[11px] text-red-500/50 whitespace-pre leading-none select-none pointer-events-none mix-blend-screen z-0"
            >
                {`
██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗██║ ██╔╝██╔════╝██╔══██╗██╔══██╗██╔════╝
██║  ██║███████║██████╔╝█████╔╝ ██║     ██║  ██║██████╔╝█████╗  
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██║     ██║  ██║██╔══██╗██╔══╝  
██████╔╝██║  ██║██║  ██║██║  ██╗╚██████╗██████╔╝██║  ██║███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═════╝ ╚═╝  ╚═╝╚══════╝
`}
            </motion.div>

            <motion.div
                animate={{
                    opacity: [0, 0, 0.8, 0, 0],
                    x: [0, -2, 2, 0],
                    y: [0, 1, -1, 0]
                }}
                transition={{
                    duration: 0.2,
                    repeat: Infinity,
                    repeatDelay: 3, // Different delay
                    repeatType: "reverse"
                }}
                className="absolute inset-0 font-mono text-[5px] xs:text-[6px] sm:text-[8px] md:text-[10px] lg:text-[11px] text-blue-500/50 whitespace-pre leading-none select-none pointer-events-none mix-blend-screen z-0"
            >
                {`
██████╗  █████╗ ██████╗ ██╗  ██╗ ██████╗██████╗ ██████╗ ███████╗
██╔══██╗██╔══██╗██╔══██╗██╔╝ ██╔════╝██╔══██╗██╔══██╗██╔════╝
██║  ██║███████║██████╔╝█████╔╝ ██║     ██║  ██║██████╔╝█████╗  
██║  ██║██╔══██║██╔══██╗██╔═██╗ ██║     ██║  ██║██╔══██╗██╔══╝  
██████╔╝██║  ██║██║  ██║██║  ██╗╚██████╗██████╔╝██║  ██║███████╗
╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═════╝ ╚═╝  ╚═╝╚══════╝
`}
            </motion.div>
        </div>
    );
}
