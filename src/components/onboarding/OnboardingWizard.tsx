import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AppConfig, ConfigService } from "../../services/ConfigService";
import { FolderOpen, AlertTriangle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

export function OnboardingWizard() {
    const [visible, setVisible] = useState(false);
    const [config, setConfig] = useState<AppConfig | null>(null);

    useEffect(() => {
        checkConfig();
    }, []);

    const checkConfig = async () => {
        try {
            const cfg = await ConfigService.getConfig();
            console.log("Loaded config:", cfg);
            if (!cfg.gl_path || cfg.gl_path.length < 3) {
                setConfig(cfg);
                setVisible(true);
            }
        } catch (e) {
            console.error(e);
        }
    };

    const handleSave = async () => {
        if (!config) return;
        try {
            await ConfigService.saveConfig(config);
            setVisible(false);
            window.location.reload();
        } catch (e) {
            alert("Failed to save config: " + e);
        }
    };

    const handleBrowseGL = async () => {
        if (!config) return;
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: "Select GreenLuma 2024 (v1.7.0) Folder",
            });
            if (selected && typeof selected === "string") {
                setConfig({ ...config, gl_path: selected });
            }
        } catch (e) {
            console.error("Dialog failed", e);
        }
    };

    const handleBrowseSteam = async () => {
        if (!config) return;
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                title: "Select Steam Folder",
            });
            if (selected && typeof selected === "string") {
                setConfig({ ...config, steam_path: selected });
            }
        } catch (e) {
            console.error("Dialog failed", e);
        }
    };



    if (!visible || !config) return null;

    return (
        <AnimatePresence>
            <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4"
            >
                <motion.div
                    initial={{ scale: 0.9, y: 20 }}
                    animate={{ scale: 1, y: 0 }}
                    className="bg-zinc-900 border border-neon-green/30 rounded-2xl p-8 max-w-lg w-full shadow-[0_0_50px_rgba(0,255,65,0.1)] relative overflow-hidden"
                >
                    {/* Matrix Rain Decoration Background? Maybe too much. Keep it clean. */}

                    <div className="flex items-center gap-4 mb-6 text-neon-green">
                        <AlertTriangle className="w-8 h-8" />
                        <h2 className="text-2xl font-bold">System Initialization</h2>
                    </div>

                    <p className="text-zinc-400 mb-8">
                        Welcome to <span className="text-white font-bold">DarkCore Manager</span>.
                        To begin, we need to locate your GreenLuma installation and Verify Steam path.
                    </p>

                    <div className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-sm font-bold text-white uppercase tracking-wider block">
                                GreenLuma 1.7.0 2025 Location
                            </label>
                            <p className="text-xs text-red-500 font-bold mb-1">
                                ⚠ MUST be v1.7.0! (v1.7.1/1.7.2 cause download errors)
                            </p>
                            <div className="relative flex gap-2">
                                <div className="relative flex-1">
                                    <input
                                        type="text"
                                        value={config.gl_path}
                                        onChange={e => setConfig({ ...config, gl_path: e.target.value })}
                                        placeholder="E:\GreenLuma_2025"
                                        className="w-full bg-black border border-zinc-700 rounded-lg pl-10 pr-4 py-3 text-white font-mono text-sm focus:border-neon-green outline-none transition-all"
                                    />
                                    <FolderOpen className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                                </div>
                                <button
                                    onClick={handleBrowseGL}
                                    className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 rounded-lg font-bold transition-colors"
                                >
                                    BROWSE
                                </button>
                            </div>
                            <p className="text-xs text-zinc-500">Folder containing GreenLuma_2025_x64.dll</p>
                        </div>

                        <div className="space-y-2">
                            <label className="text-sm font-bold text-white uppercase tracking-wider block">Steam Installation</label>
                            <div className="relative flex gap-2">
                                <div className="relative flex-1">
                                    <input
                                        type="text"
                                        value={config.steam_path}
                                        onChange={e => setConfig({ ...config, steam_path: e.target.value })}
                                        placeholder="C:\Program Files (x86)\Steam"
                                        className="w-full bg-black border border-zinc-700 rounded-lg pl-10 pr-4 py-3 text-white font-mono text-sm focus:border-neon-green outline-none transition-all"
                                    />
                                    <FolderOpen className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                                </div>
                                <button
                                    onClick={handleBrowseSteam}
                                    className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 rounded-lg font-bold transition-colors"
                                >
                                    BROWSE
                                </button>
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="text-sm font-bold text-white uppercase tracking-wider">GMRC API Key (Optional)</label>
                            <div className="relative">
                                <input
                                    type="password"
                                    value={config.api_key}
                                    onChange={e => setConfig({ ...config, api_key: e.target.value })}
                                    placeholder="smm_..."
                                    className="w-full bg-black border border-zinc-700 rounded-lg px-4 py-3 text-white font-mono text-sm focus:border-neon-green outline-none transition-all"
                                />
                            </div>
                        </div>
                    </div>

                    <div className="mt-8 flex justify-end">
                        <button
                            onClick={handleSave}
                            disabled={!config.gl_path || !config.steam_path}
                            className="bg-neon-green text-black font-bold uppercase tracking-widest px-8 py-3 rounded hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            Initialize System
                        </button>
                    </div>
                </motion.div>
            </motion.div>
        </AnimatePresence>
    );
}
