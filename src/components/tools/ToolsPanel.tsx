import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Wrench, FileCode, Cpu, Play } from "lucide-react";

export function ToolsPanel() {

    return (
        <div className="p-8 max-w-5xl mx-auto h-full overflow-y-auto">
            <h1 className="text-3xl font-black text-white mb-2">ADVANCED TOOLS</h1>
            <p className="text-zinc-500 mb-8">DRM Removal and Emulation Utilities</p>

            <div className="grid gap-6">
                {/* Import Tool Card - Moved to Store Panel */}

                {/* <ImportToolCard /> Removed */}

                <SteamlessTool />
                <GoldbergTool />
                {/* <WatcherTool /> -- Maybe add later */}
            </div>
        </div>
    );
}

function SteamlessTool() {
    const [path, setPath] = useState("");
    const [loading, setLoading] = useState(false);

    const handleBrowse = async () => {
        try {
            const selected = await open({
                multiple: false,
                filters: [{ name: 'Executables', extensions: ['exe'] }]
            });
            if (selected && typeof selected === "string") {
                setPath(selected);
            }
        } catch (e) {
            console.error(e);
        }
    };

    const handleRun = async () => {
        if (!path) return;
        setLoading(true);
        try {
            await invoke("run_steamless", { exe_path: path });
            alert("Steamless executed successfully! Original file backed up.");
        } catch (e) {
            alert(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="bg-zinc-900/50 border border-zinc-800 rounded-xl p-6">
            <div className="flex items-center gap-4 mb-4">
                <div className="p-3 bg-blue-500/10 rounded-lg text-blue-400">
                    <FileCode className="w-6 h-6" />
                </div>
                <div>
                    <h2 className="text-xl font-bold text-white">Steamless DRM Remover</h2>
                    <p className="text-zinc-500 text-sm">Unpack SteamStub DRM from executables</p>
                </div>
            </div>

            <div className="flex gap-2">
                <input
                    type="text"
                    value={path}
                    readOnly
                    placeholder="Select executable..."
                    className="flex-1 bg-black border border-zinc-700 rounded px-4 py-2 text-sm text-white font-mono"
                />
                <button
                    onClick={handleBrowse}
                    className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 rounded font-bold transition-colors"
                >
                    BROWSE
                </button>
                <button
                    onClick={handleRun}
                    disabled={!path || loading}
                    className="bg-blue-600 hover:bg-blue-500 text-white px-6 rounded font-bold transition-colors disabled:opacity-50 flex items-center gap-2"
                >
                    {loading ? <Cpu className="w-4 h-4 animate-spin" /> : <Play className="w-4 h-4" />}
                    RUN
                </button>
            </div>
        </div>
    );
}

function GoldbergTool() {
    const [appId, setAppId] = useState("");
    const [loading, setLoading] = useState(false);

    const handleGenerate = async () => {
        if (!appId) return;
        setLoading(true);
        try {
            await invoke("generate_goldberg", { appid: appId });
            alert("Goldberg Emulator generated successfully in game folder.");
        } catch (e) {
            alert(`Error: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="bg-zinc-900/50 border border-zinc-800 rounded-xl p-6">
            <div className="flex items-center gap-4 mb-4">
                <div className="p-3 bg-yellow-500/10 rounded-lg text-yellow-500">
                    <Cpu className="w-6 h-6" />
                </div>
                <div>
                    <h2 className="text-xl font-bold text-white">Goldberg Emulator Generator</h2>
                    <p className="text-zinc-500 text-sm">Generate steam_api.dll and interfaces for offline play</p>
                </div>
            </div>

            <div className="flex gap-2 items-center">
                <span className="text-zinc-400 font-mono">AppID:</span>
                <input
                    type="text"
                    value={appId}
                    onChange={(e) => setAppId(e.target.value)}
                    placeholder="e.g. 1091500"
                    className="w-48 bg-black border border-zinc-700 rounded px-4 py-2 text-sm text-white font-mono focus:border-yellow-500 outline-none"
                />
                <button
                    onClick={handleGenerate}
                    disabled={!appId || loading}
                    className="bg-yellow-600 hover:bg-yellow-500 text-black px-6 rounded font-bold transition-colors disabled:opacity-50 flex items-center gap-2 py-2"
                >
                    <Wrench className="w-4 h-4" />
                    GENERATE
                </button>
            </div>
            <p className="text-xs text-zinc-600 mt-2">
                * Requires the game to be in the AppList or manually located.
            </p>
        </div>
    );
}
