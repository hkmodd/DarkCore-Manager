import { useEffect, useState, useRef } from "react";
import { AppConfig, ConfigService } from "../../services/ConfigService";
import { open } from "@tauri-apps/plugin-dialog";
import { Settings, Shield, AlertCircle, FolderOpen, CheckCircle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";

export function SettingsPanel() {
    const [config, setConfig] = useState<AppConfig | null>(null);
    const [status, setStatus] = useState<{ msg: string, type: 'success' | 'error' | 'info' } | null>(null);
    const [hasLegacyData, setHasLegacyData] = useState(false);
    const [pathValid, setPathValid] = useState<Record<string, boolean>>({});
    const [glitchText, setGlitchText] = useState("");
    const [apiKeyFocused, setApiKeyFocused] = useState(false);

    // Track if it's the initial load to prevent saving on mount
    const isInitialLoad = useRef(true);

    useEffect(() => {
        load();
        checkLegacy();
    }, []);

    // Path validation — debounced check if paths exist
    useEffect(() => {
        if (!config) return;
        const timer = setTimeout(async () => {
            const results: Record<string, boolean> = {};
            for (const key of ['steam_path', 'gl_path', 'steamless_path'] as const) {
                if (config[key]) {
                    try {
                        results[key] = await invoke<boolean>("validate_path", { path: config[key] });
                    } catch {
                        results[key] = false;
                    }
                }
            }
            setPathValid(results);
        }, 300);
        return () => clearTimeout(timer);
    }, [config?.steam_path, config?.gl_path, config?.steamless_path]);

    // Glitch effect animation for API key field
    useEffect(() => {
        if (!apiKeyFocused || !config?.api_key) {
            setGlitchText("");
            return;
        }
        const glyphs = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*_+-=|<>?";
        const interval = setInterval(() => {
            const len = config.api_key.length;
            let result = "";
            for (let i = 0; i < len; i++) {
                result += glyphs[Math.floor(Math.random() * glyphs.length)];
            }
            setGlitchText(result);
        }, 50);
        return () => clearInterval(interval);
    }, [apiKeyFocused, config?.api_key]);

    const load = async () => {
        try {
            const cfg = await ConfigService.getConfig();
            setConfig(cfg);
            // Give it a moment to settle so we don't auto-save immediately
            setTimeout(() => { isInitialLoad.current = false; }, 500);
        } catch (e) {
            console.error(e);
        }
    };

    const checkLegacy = async () => {
        try {
            const exists = await invoke<boolean>("check_legacy_exists");
            setHasLegacyData(exists);
        } catch {
            setHasLegacyData(false);
        }
    };

    // Auto-Save Logic
    useEffect(() => {
        if (!config || isInitialLoad.current) return;

        const timer = setTimeout(async () => {
            try {
                await ConfigService.saveConfig(config);
                showNotification("Configuration Saved", "success");
            } catch (e) {
                showNotification(`Save Failed: ${e}`, "error");
            }
        }, 800); // 800ms debounce

        return () => clearTimeout(timer);
    }, [config]);

    const showNotification = (msg: string, type: 'success' | 'error' | 'info') => {
        setStatus({ msg, type });
        setTimeout(() => setStatus(null), 3000);
    };

    const handleBrowse = async (field: keyof AppConfig, isFile: boolean = false) => {
        try {
            const selected = await open({
                multiple: false,
                directory: !isFile,
                filters: isFile ? [{ name: 'Executables', extensions: ['exe'] }] : undefined
            });
            if (selected && typeof selected === "string") {
                setConfig({ ...config!, [field]: selected });
            }
        } catch (e) {
            console.error(e);
        }
    };

    if (!config) return <div className="p-8 text-zinc-500 flex items-center gap-2"><div className="w-4 h-4 border-2 border-zinc-500 border-t-transparent rounded-full animate-spin" /> Loading settings...</div>;

    return (
        <div className="p-6 max-w-4xl mx-auto space-y-8 relative">
            {/* Premium Notification Toast */}
            <AnimatePresence>
                {status && (
                    <motion.div
                        initial={{ opacity: 0, y: -20, x: "-50%" }}
                        animate={{ opacity: 1, y: 0, x: "-50%" }}
                        exit={{ opacity: 0, y: -20, x: "-50%" }}
                        className={`fixed top-8 left-1/2 z-50 px-6 py-3 rounded-xl backdrop-blur-md border shadow-2xl flex items-center gap-3 ${status.type === 'success' ? 'bg-green-500/10 border-green-500/20 text-green-400' :
                                status.type === 'error' ? 'bg-red-500/10 border-red-500/20 text-red-400' :
                                    'bg-blue-500/10 border-blue-500/20 text-blue-400'
                            }`}
                    >
                        {status.type === 'success' && <CheckCircle className="w-5 h-5" />}
                        {status.type === 'error' && <AlertCircle className="w-5 h-5" />}
                        <span className="font-bold text-sm tracking-wide">{status.msg}</span>
                    </motion.div>
                )}
            </AnimatePresence>

            {/* Header */}
            <div className="flex items-center gap-4">
                <div className="p-3 bg-zinc-800 rounded-lg text-zinc-400">
                    <Settings className="w-8 h-8" />
                </div>
                <div>
                    <h2 className="text-3xl font-bold text-white">System Configuration</h2>
                    <p className="text-zinc-400">Core paths and security preferences</p>
                </div>
            </div>

            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="bg-zinc-900/50 border border-zinc-800 rounded-xl p-6 space-y-8"
            >
                {/* Paths Section */}
                <div className="space-y-5">
                    <h3 className="text-zinc-500 text-xs font-bold uppercase tracking-widest border-b border-zinc-800 pb-2 flex items-center gap-2">
                        <FolderOpen className="w-3 h-3" /> Environment Paths
                    </h3>

                    <div className="grid gap-6">
                        {([
                            { key: 'steam_path' as const, label: 'Steam Installation Path', isFile: false },
                            { key: 'gl_path' as const, label: 'GreenLuma 2025 Path', isFile: false },
                            { key: 'steamless_path' as const, label: 'Steamless CLI Path', isFile: true },
                        ]).map(({ key, label, isFile }) => (
                            <div key={key} className="space-y-2">
                                <label className="text-sm font-bold text-zinc-300 flex items-center gap-2">
                                    {label}
                                    {config[key] && pathValid[key] !== undefined && (
                                        <span className={`text-[9px] font-bold px-1.5 py-0.5 rounded ${pathValid[key] ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>
                                            {pathValid[key] ? 'VALID' : 'NOT FOUND'}
                                        </span>
                                    )}
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={config[key]}
                                        onChange={e => setConfig({ ...config, [key]: e.target.value })}
                                        className={`flex-1 bg-black/50 rounded-lg px-4 py-3 text-white font-mono text-sm transition-colors outline-none focus:bg-black/80 border ${
                                            !config[key] ? 'border-zinc-700/50 focus:border-cyan-500' :
                                            pathValid[key] === true ? 'border-green-500/40 focus:border-green-400' :
                                            pathValid[key] === false ? 'border-red-500/40 focus:border-red-400' :
                                            'border-zinc-700/50 focus:border-cyan-500'
                                        }`}
                                    />
                                    <button onClick={() => handleBrowse(key, isFile)} className="bg-zinc-800 px-4 rounded-lg hover:bg-zinc-700 text-white transition-colors"><FolderOpen className="w-4 h-4" /></button>
                                </div>
                            </div>
                        ))}
                    </div>
                </div>

                {/* API Key */}
                <div className="space-y-5">
                    <h3 className="text-zinc-500 text-xs font-bold uppercase tracking-widest border-b border-zinc-800 pb-2 flex items-center gap-2">
                        <Shield className="w-3 h-3" /> WuDRM / Morrenus Integration
                    </h3>
                    <div className="space-y-2">
                        <label className="text-sm font-bold text-zinc-300">Morrenus API Key</label>
                        <div className="relative">
                            <input
                                type="password"
                                value={config.api_key}
                                onChange={e => setConfig({ ...config, api_key: e.target.value })}
                                onFocus={() => setApiKeyFocused(true)}
                                onBlur={() => setApiKeyFocused(false)}
                                placeholder="smm_..."
                                className={`w-full bg-black/50 rounded-lg px-4 py-3 font-mono text-sm transition-all outline-none focus:bg-black/80 ${
                                    apiKeyFocused
                                        ? 'border border-green-500/50 shadow-[0_0_15px_rgba(0,255,0,0.1)] text-transparent'
                                        : 'border border-zinc-700/50 focus:border-cyan-500 text-white'
                                }`}
                            />
                            {/* Glitch overlay — shows scrambled chars when focused */}
                            {apiKeyFocused && config.api_key && (
                                <div className="absolute inset-0 px-4 py-3 font-mono text-sm text-green-400 pointer-events-none rounded-lg overflow-hidden">
                                    {glitchText}
                                </div>
                            )}
                        </div>
                        <p className="text-[10px] text-zinc-500">Secure key for fetching ZIPs from Morrenus. WuDRM is used automatically for manifests.</p>
                    </div>

                    <div className="space-y-2">
                        <label className="text-sm font-bold text-zinc-300">Target Language (Goldberg)</label>
                        <select
                            value={config.target_language}
                            onChange={e => setConfig({ ...config, target_language: e.target.value })}
                            className="w-full bg-black/50 border border-zinc-700/50 rounded-lg px-4 py-3 text-white font-mono text-sm focus:border-cyan-500 transition-colors outline-none focus:bg-black/80"
                        >
                            <option value="english">English</option>
                            <option value="italian">Italian</option>
                            <option value="german">German</option>
                            <option value="spanish">Spanish</option>
                            <option value="french">French</option>
                            <option value="russian">Russian</option>
                            <option value="schinese">Simplified Chinese</option>
                            <option value="japanese">Japanese</option>
                            <option value="brazilian">Portuguese-Brazil</option>
                        </select>
                    </div>
                </div>

                {/* Toggles */}
                <div className="space-y-5">
                    <h3 className="text-zinc-500 text-xs font-bold uppercase tracking-widest border-b border-zinc-800 pb-2">Stealth & Behavior</h3>

                    {config.steam_path && config.gl_path && config.gl_path.toLowerCase().includes(config.steam_path.toLowerCase()) && (
                        <div className="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg flex items-start gap-3">
                            <AlertCircle className="w-6 h-6 text-yellow-500 shrink-0" />
                            <div>
                                <h4 className="font-bold text-yellow-400 text-sm">⚠ STEALTH RISK DETECTED</h4>
                                <p className="text-xs text-zinc-300 mt-1">
                                    GreenLuma appears to be located <b>inside</b> your Steam folder.
                                    This increases ban risk and detection probability.
                                    <br />
                                    Recommended: Move GreenLuma to a separate folder (e.g., <code>C:\GreenLuma</code>).
                                </p>
                            </div>
                        </div>
                    )}

                    <div className="flex items-center justify-between p-4 bg-gradient-to-r from-zinc-900 to-black rounded-lg border border-zinc-800 hover:border-zinc-700 transition-colors">
                        <div className="flex items-center gap-3">
                            <div className={`p-2 rounded-lg ${config.enable_stealth_mode ? "bg-green-500/20 text-green-400" : "bg-zinc-800 text-zinc-500"}`}>
                                <Shield className="w-5 h-5" />
                            </div>
                            <div>
                                <div className="text-sm font-bold text-white">Stealth Mode</div>
                                <div className="text-xs text-zinc-500">Renames injection process to evade basic detection</div>
                            </div>
                        </div>
                        <button
                            onClick={() => setConfig({ ...config, enable_stealth_mode: !config.enable_stealth_mode })}
                            className={`w-12 h-6 rounded-full p-1 transition-colors ${config.enable_stealth_mode ? "bg-green-500/20" : "bg-zinc-700"}`}
                        >
                            <div className={`w-4 h-4 rounded-full bg-white transition-transform shadow-sm ${config.enable_stealth_mode ? "translate-x-6 bg-green-400" : ""}`} />
                        </button>
                    </div>
                </div>

                {/* API Stats Graph */}
                <div className="pt-4 border-t border-zinc-800/50">
                    <h3 className="text-zinc-500 text-xs font-bold uppercase tracking-widest mb-3">API Usage (Daily)</h3>
                    <div className="h-24 bg-black/40 rounded-lg border border-zinc-800 relative overflow-hidden flex items-end">
                        <ApiUsageBar />
                    </div>
                </div>

                {/* Legacy Import (Conditional) */}
                {hasLegacyData && (
                    <div className="space-y-4 pt-4 animate-in fade-in slide-in-from-bottom-4 duration-500">
                        <h3 className="text-zinc-500 text-xs font-bold uppercase tracking-widest border-b border-zinc-800 pb-2">Data Migration</h3>
                        <div className="flex items-center justify-between p-4 bg-zinc-800/20 rounded-lg border border-zinc-800">
                            <div>
                                <div className="text-sm font-bold text-white">Legacy AppList Import</div>
                                <div className="text-xs text-zinc-500">Import games from detected old GreenLuma installation</div>
                            </div>
                            <button
                                onClick={async () => {
                                    try {
                                        const count = await invoke("import_legacy_applist");
                                        showNotification(`Imported ${count} games from Legacy AppList.`, "success");
                                    } catch (e) {
                                        showNotification(`Import failed: ${e}`, "error");
                                    }
                                }}
                                className="px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-white rounded text-xs font-bold transition-colors border border-zinc-700"
                            >
                                IMPORT LEGACY
                            </button>
                        </div>
                    </div>
                )}
            </motion.div>
        </div>
    );
}

function ApiUsageBar() {
    const [stats, setStats] = useState<{ usage: number, limit: number } | null>(null);

    useEffect(() => {
        invoke("get_api_stats").then((s: any) => {
            setStats({ usage: s.daily_usage, limit: s.daily_limit });
        }).catch(() => { });
    }, []);

    if (!stats) return <div className="w-full h-full flex items-center justify-center text-xs text-zinc-600">Loading Stats...</div>;

    const percent = Math.min(100, Math.max(0, (stats.usage / stats.limit) * 100));
    const isCrisis = percent > 90;

    return (
        <div className="w-full h-full p-4 flex flex-col justify-center">
            <div className="flex justify-between text-xs text-zinc-400 mb-1">
                <span>Requests Today</span>
                <span>{stats.usage} / {stats.limit}</span>
            </div>
            <div className="h-4 bg-zinc-800 rounded-full overflow-hidden relative">
                <motion.div
                    initial={{ width: 0 }}
                    animate={{ width: `${percent}%` }}
                    className={`h-full ${isCrisis ? 'bg-red-500 shadow-[0_0_10px_rgba(239,68,68,0.5)]' : 'bg-cyan-500 shadow-[0_0_10px_rgba(6,182,212,0.5)]'}`}
                />
            </div>
            {isCrisis && <div className="text-[10px] text-red-500 mt-1 animate-pulse">Warning: API Limit Approaching!</div>}
        </div>
    );
}
