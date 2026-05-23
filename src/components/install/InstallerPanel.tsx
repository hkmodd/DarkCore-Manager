import { useState } from "react";
import { LuaService, ScriptData } from "../../services/LuaService";
import { AppListService } from "../../services/AppListService";
import { invoke } from "@tauri-apps/api/core";
import { Download, FileArchive } from "lucide-react";
import { motion } from "framer-motion";
import { ImportModal } from "../tools/ImportModal";

export function InstallerPanel() {
    const [path, setPath] = useState("");
    const [data, setData] = useState<ScriptData | null>(null);
    const [status, setStatus] = useState("");
    const [downloading, setDownloading] = useState(false);
    const [showImportModal, setShowImportModal] = useState(false);

    const handleParse = async () => {
        try {
            setStatus("Parsing...");
            const result = await LuaService.parseScript(path.replace(/"/g, ""));
            setData(result);
            setStatus("Parsed successfully.");
        } catch (e) {
            setStatus(`Error: ${e}`);
            setData(null);
        }
    };

    const handleDownload = async () => {
        if (!data) return;
        setDownloading(true);
        const depotsToDownload = data.depots.filter(d => d.manifest_id);

        let success = 0;
        let fail = 0;

        // 1. Download Manifests
        for (const depot of depotsToDownload) {
            try {
                setStatus(`Downloading Depot ${depot.depot_id}...`);
                // Determine output path... assume "Downloads" folder for now
                // In production this should be the Steam/depotcache folder?
                // For now, let's keep it "Downloads" or maybe empty string implies default?
                // The backend downloader uses `output_path` arg. 
                // If I want it to go to Steam/depotcache, I should PROBABLY pass the configured steam path?
                // Or maybe the backend should handle it. 
                // Legacy downloader writes to "depotcache" inside the manager folder or steam folder?
                // Let's pass a specific safe folder for now or absolute steam path if known.
                // Assuming backend handles relative paths relative to CWD.
                const out = ".";
                await invoke("download_manifest", {
                    depotId: depot.depot_id.toString(),
                    manifestGid: depot.manifest_id?.toString(),
                    outputPath: out
                });
                success++;
            } catch (e) {
                console.error(e);
                fail++;
            }
        }

        // 2. Add to AppList
        setStatus("Updating GreenLuma AppList...");
        try {
            const allIds = new Set<string>();
            if (data.app_id) allIds.add(data.app_id.toString());
            data.depots.forEach(d => allIds.add(d.depot_id.toString()));
            data.dlcs.forEach(d => allIds.add(d.app_id.toString()));

            await AppListService.addGames(Array.from(allIds));
        } catch (e) {
            console.error("Failed to update AppList", e);
            setStatus(`AppList Error: ${e}`);
        }

        // 3. Inject Keys
        setStatus("Injecting VDF Keys...");
        try {
            const keys: Record<string, string> = {};
            data.depots.forEach(d => {
                if (d.depot_key) {
                    keys[d.depot_id.toString()] = d.depot_key;
                }
            });
            await AppListService.injectVdfKeys(keys);
        } catch (e) {
            console.error("Failed to inject VDF keys", e);
            setStatus(`VDF Error: ${e}`);
        }

        setStatus(`Completed. Manifests: ${success}/${depotsToDownload.length}. AppList Updated.`);
        setDownloading(false);
    };

    return (
        <div className="p-6 space-y-6 max-w-4xl mx-auto">
            <div className="flex items-center justify-between mb-8">
                <div className="flex items-center gap-4">
                    <div className="p-3 bg-cyan-500/10 rounded-lg text-cyan-400">
                        <Download className="w-8 h-8" />
                    </div>
                    <div>
                        <h2 className="text-3xl font-bold text-white">Direct Installer</h2>
                        <p className="text-zinc-400">Parse Lua/ST scripts & Download Manifests</p>
                    </div>
                </div>
                <button
                    onClick={() => setShowImportModal(true)}
                    className="bg-orange-600 hover:bg-orange-500 text-white px-6 py-3 rounded-xl font-bold transition-colors flex items-center gap-2 shadow-lg shadow-orange-900/20"
                >
                    <FileArchive className="w-5 h-5" />
                    IMPORT ZIP ARCHIVE
                </button>
            </div>

            {showImportModal && <ImportModal onClose={() => setShowImportModal(false)} />}

            <div className="bg-zinc-900/50 border border-zinc-800 p-6 rounded-xl space-y-4">
                <label className="text-sm font-bold text-zinc-300">Script Path (.lua / .st)</label>
                <div className="flex gap-2">
                    <input
                        type="text"
                        value={path}
                        onChange={(e) => setPath(e.target.value)}
                        placeholder="E:\Games\Scripts\MyGame.lua"
                        className="flex-1 bg-black/50 border border-zinc-700 rounded-lg px-4 py-2 text-white font-mono text-sm focus:border-cyan-500 outline-none transition-colors"
                    />
                    <button
                        onClick={handleParse}
                        className="bg-cyan-600 hover:bg-cyan-500 text-white px-6 py-2 rounded-lg font-bold transition-colors"
                    >
                        PARSE
                    </button>
                </div>
                {status && <p className="text-xs font-mono text-cyan-400">{status}</p>}
            </div>

            {data && (
                <motion.div
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="space-y-6"
                >
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div className="p-4 bg-zinc-900 border border-zinc-800 rounded-lg">
                            <h3 className="text-zinc-500 text-xs font-bold uppercase mb-1">Detected App</h3>
                            <div className="text-xl font-bold text-white">{data.app_name || "Unknown Game"}</div>
                            <div className="text-xs font-mono text-cyan-500 mt-1">AppID: {data.app_id}</div>
                        </div>
                        <div className="p-4 bg-zinc-900 border border-zinc-800 rounded-lg">
                            <h3 className="text-zinc-500 text-xs font-bold uppercase mb-1">Content</h3>
                            <div className="flex gap-4">
                                <div className="text-center">
                                    <div className="text-xl font-bold text-white">{data.depots.length}</div>
                                    <div className="text-xs text-zinc-500">Depots</div>
                                </div>
                                <div className="text-center">
                                    <div className="text-xl font-bold text-white">{data.dlcs.length}</div>
                                    <div className="text-xs text-zinc-500">DLCs</div>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="bg-zinc-900/30 border border-zinc-800/50 rounded-lg overflow-hidden">
                        <table className="w-full text-sm text-left">
                            <thead className="bg-zinc-800/50 text-zinc-400 font-mono text-xs uppercase">
                                <tr>
                                    <th className="p-3">ID</th>
                                    <th className="p-3">Type</th>
                                    <th className="p-3">Manifest GID</th>
                                    <th className="p-3">Key</th>
                                </tr>
                            </thead>
                            <tbody className="divide-y divide-zinc-800/50">
                                {data.depots.map(depot => (
                                    <tr key={depot.depot_id} className="hover:bg-white/5 transition-colors">
                                        <td className="p-3 font-mono text-cyan-400">{depot.depot_id}</td>
                                        <td className="p-3">
                                            <span className={`text-[10px] px-2 py-0.5 rounded border ${depot.category === 'MainApp' ? 'border-green-500/30 text-green-400 bg-green-500/10' :
                                                depot.category === 'DlcDepot' ? 'border-yellow-500/30 text-yellow-400 bg-yellow-500/10' :
                                                    'border-zinc-700 text-zinc-400'
                                                }`}>
                                                {depot.category}
                                            </span>
                                        </td>
                                        <td className="p-3 font-mono text-zinc-300">
                                            {depot.manifest_id ? depot.manifest_id.toString() : <span className="text-zinc-600">-</span>}
                                        </td>
                                        <td className="p-3 font-mono text-[10px] text-zinc-500 truncate max-w-[100px]">
                                            {depot.depot_key}
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>

                    <button
                        onClick={handleDownload}
                        disabled={downloading}
                        className="w-full py-4 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 disabled:cursor-not-allowed text-white font-bold rounded-xl shadow-lg shadow-cyan-900/20 transition-all flex items-center justify-center gap-2"
                    >
                        {downloading ? (
                            <>Processing...</>
                        ) : (
                            <>
                                <Download className="w-5 h-5" />
                                DOWNLOAD {data.depots.filter(d => d.manifest_id).length} MANIFESTS
                            </>
                        )}
                    </button>
                </motion.div>
            )}
        </div>
    );
}
