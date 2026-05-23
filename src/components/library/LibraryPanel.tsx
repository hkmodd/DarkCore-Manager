import { useEffect, useState, useMemo } from "react";
import { AppListService, GameProfile } from "../../services/AppListService";
import { ProfileService } from "../../services/ProfileService";
import { Trash2, RefreshCw, Gamepad2, Search, Shield, Settings, ListOrdered } from "lucide-react";
import { motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { DeleteModal } from "./DeleteModal";

export function LibraryPanel() {
    const [games, setGames] = useState<GameProfile[]>([]);
    const [loading, setLoading] = useState(false);
    const [search, setSearch] = useState("");
    const [profiles, setProfiles] = useState<string[]>([]);
    const [selectedProfile, setSelectedProfile] = useState("Default");
    const [newProfileName, setNewProfileName] = useState("");
    const [isCreating, setIsCreating] = useState(false);
    const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
    const [updating, setUpdating] = useState<Set<string>>(new Set());
    const [isScanning, setIsScanning] = useState(false);
    const [unsavedChanges, setUnsavedChanges] = useState(false);

    const _fetchGames = async (cancelled: { current: boolean }) => {
        setLoading(true);
        try {
            const list = await AppListService.getActiveGames();
            // Guard: if component was unmounted during the async call, don't update state
            if (cancelled.current) return;
            setGames(list);
        } catch (e) {
            console.error("Failed to load games", e);
            // DON'T clear games on error — keep previous data visible
        } finally {
            if (!cancelled.current) setLoading(false);
        }
    };

    const fetchGames = () => {
        _fetchGames({ current: false });
    };

    useEffect(() => {
        const cancelled = { current: false };
        _fetchGames(cancelled);
        fetchProfiles();
        return () => { cancelled.current = true; };
    }, []);



    const fetchProfiles = async () => {
        try {
            const list = await ProfileService.listProfiles();
            setProfiles(list);
            if (list.length > 0 && !selectedProfile) {
                setSelectedProfile(list[0]);
            } else if (list.length === 0) {
                setSelectedProfile("");
            }
        } catch (e) {
            console.error(e);
        }
    };

    const openDeleteModal = (id: string, name: string) => {
        setDeleteTarget({ id, name });
    };

    // Wrapped delete callback to set unsaved changes
    const onGameDeleted = () => {
        fetchGames();
        setUnsavedChanges(true);
    };


    const handleSaveProfile = async () => {
        const name = isCreating ? newProfileName : selectedProfile;
        if (!name) return;
        if (!confirm(`Save current AppList (${games.length} games) to profile '${name}'?`)) return;

        try {
            const appIds = games.map(g => g.app_id);
            await ProfileService.saveProfile({ name, app_ids: appIds });
            setIsCreating(false);
            setNewProfileName("");
            await fetchProfiles();
            setSelectedProfile(name);
            setUnsavedChanges(false);
            alert("Profile saved successfully.");
        } catch (e) {
            alert(`Failed to save: ${e}`);
        }
    };

    const handleDeleteProfile = async () => {
        if (!confirm(`Delete profile '${selectedProfile}'?`)) return;
        try {
            await ProfileService.deleteProfile(selectedProfile);
            await fetchProfiles();
            setSelectedProfile(profiles[0] || "");
            setUnsavedChanges(false);
        } catch (e) {
            alert(`Failed to delete: ${e}`);
        }
    };

    const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

    const toggleExpand = (id: string) => {
        setExpandedGroups(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    };

    const handleReorder = async () => {
        setLoading(true);
        try {
            await invoke("reorder_list");
            await fetchGames();
            setUnsavedChanges(true); // Reorder modifies list
        } catch (e) {
            alert(`Reorder failed: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    // Grouping Logic
    const gameGroups = useMemo(() => {
        const groups: GameGroup[] = [];
        const roots = games.filter(g => !g.parent_id);
        const children = games.filter(g => g.parent_id);

        // Filter by search
        const matchesSearch = (g: GameProfile) =>
            g.name.toLowerCase().includes(search.toLowerCase()) ||
            g.app_id.includes(search);

        roots.forEach(root => {
            const myChildren = children.filter(c => c.parent_id === root.app_id);

            // If search is active, only show if root matches OR any child matches
            if (search) {
                const rootMatches = matchesSearch(root);
                const childMatches = myChildren.some(matchesSearch);
                if (!rootMatches && !childMatches) return;
            }

            groups.push({ root, children: myChildren });
        });

        // Add Orphans (children with no parent in list)??
        // Usually shouldn't happen if properly filtered.
        // If search is active, we might miss the parent?
        // Actually if we filter roots, we might lose children.
        // Let's keep it simple: Filter applies into the structure.

        return groups;
    }, [games, search]);

    const triggerScan = async () => {
        setIsScanning(true);
        try {
            await AppListService.scanForUpdates();
            // Backend emits 'library-update' when done, which triggers fetchGames
        } catch (e) {
            console.error(e);
            setIsScanning(false);
        }
    };

    // Auto-scan on mount
    useEffect(() => {
        triggerScan();
    }, []);

    // Auto-refresh when backend resolves names or updates
    useEffect(() => {
        const unlisten = listen("library-update", () => {
            fetchGames();
            setIsScanning(false);
        });
        return () => { unlisten.then(fn => fn()); };
    }, []);

    const handleUpdate = async (appId: string) => {
        setUpdating(prev => { const n = new Set(prev); n.add(appId); return n; });
        try {
            const msg = await AppListService.updateGameManifests(appId);
            alert(msg);
            // Re-fetch to clear the pending flag
            await fetchGames();
        } catch (e) {
            alert(`Update failed: ${e}`);
        } finally {
            setUpdating(prev => { const n = new Set(prev); n.delete(appId); return n; });
        }
    };

    const handleRepair = async () => {
        if (!confirm("Start Library Repair Scan?\n\nThis will scan all installed games and fetch metadata from Steam to fix missing groupings (DLCs/Depots).\nThis may take a minute.")) return;

        setLoading(true);
        try {
            const result = await invoke<string>("repair_library_relationships");
            alert(result);
            await fetchGames();
        } catch (e) {
            alert(`Repair failed: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="p-6 max-w-6xl mx-auto h-full flex flex-col">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-4">
                    <div className="p-3 bg-fuchsia-500/10 rounded-lg text-fuchsia-400">
                        <Gamepad2 className="w-8 h-8" />
                    </div>
                    <div>
                        <h2 className="text-3xl font-bold text-white">My Library</h2>
                        <p className="text-zinc-400">Manage injected games and depots</p>
                    </div>
                </div>

                <div className="flex gap-2">
                    <div className="flex gap-2">
                        <button
                            onClick={triggerScan}
                            disabled={isScanning}
                            className={`px-4 py-2 rounded-lg font-bold transition-colors flex items-center gap-2 ${isScanning ? "bg-zinc-800 text-zinc-500" : "bg-zinc-800 text-zinc-400 hover:text-white"}`}
                            title="Check for Manifest Updates"
                        >
                            <RefreshCw className={`w-4 h-4 ${isScanning ? "animate-spin" : ""}`} />
                            {isScanning ? "Scanning..." : "Check Updates"}
                        </button>
                        <button
                            onClick={handleReorder}
                            className="p-2 hover:bg-white/10 rounded-lg transition-colors text-zinc-400 hover:text-white"
                            title="Reorder List (Alphabetical)"
                        >
                            <ListOrdered className="w-5 h-5" />
                        </button>
                        <button
                            onClick={handleRepair}
                            className="p-2 hover:bg-white/10 rounded-lg transition-colors text-zinc-400 hover:text-cyan-400"
                            title="Repair Library Relationships (Fix Grouping)"
                        >
                            <Shield className={`w-5 h-5 ${loading ? "animate-pulse" : ""}`} />
                        </button>
                        <button
                            onClick={fetchGames}
                            className="p-2 hover:bg-white/10 rounded-lg transition-colors text-zinc-400 hover:text-white"
                            title="Refresh List"
                        >
                            <RefreshCw className={`w-5 h-5 ${loading ? "animate-spin" : ""}`} />
                        </button>
                    </div>
                </div>
            </div>

            {/* Profile Manager Bar - Material Glass Redesign */}
            <div className="bg-zinc-900/40 border border-white/5 p-4 rounded-xl mb-6 flex items-center justify-between backdrop-blur-md shadow-lg relative overflow-hidden group/profile">
                {/* Decorative Glow */}
                <div className="absolute top-0 left-0 w-1/3 h-full bg-gradient-to-r from-cyan-500/10 to-transparent opacity-0 group-hover/profile:opacity-100 transition-opacity pointer-events-none" />

                <div className="flex items-center gap-4 relative z-10">
                    <div className="p-2 bg-gradient-to-br from-cyan-500/20 to-purple-500/20 rounded-lg border border-white/5">
                        <Settings className="w-5 h-5 text-cyan-400/80" />
                    </div>

                    <div className="flex flex-col">
                        <div className="flex items-center gap-2 mb-0.5">
                            <span className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Active Profile</span>
                            {unsavedChanges && (
                                <span className="text-[9px] font-bold bg-amber-500/20 text-amber-500 px-1.5 rounded animate-pulse">
                                    UNSAVED
                                </span>
                            )}
                        </div>

                        {isCreating ? (
                            <div className="flex items-center gap-2">
                                <input
                                    autoFocus
                                    type="text"
                                    value={newProfileName}
                                    onChange={e => setNewProfileName(e.target.value)}
                                    placeholder="Enter profile name..."
                                    className="bg-black/50 border border-cyan-500/50 rounded px-3 py-1 text-sm text-white w-48 focus:border-cyan-400 focus:shadow-[0_0_10px_rgba(34,211,238,0.2)] outline-none transition-all placeholder:text-zinc-600"
                                />
                                <button
                                    onClick={() => setIsCreating(false)}
                                    className="text-xs text-red-400 hover:text-red-300 hover:bg-red-500/10 px-2 py-1 rounded transition-colors"
                                >
                                    Cancel
                                </button>
                            </div>
                        ) : (
                            <div className="flex items-center gap-3">
                                <select
                                    value={selectedProfile}
                                    onChange={async (e) => {
                                        const newProfile = e.target.value;
                                        if (newProfile === selectedProfile) return;

                                        // Safety Prompt
                                        if (unsavedChanges) {
                                            const choice = confirm(`You have unsaved changes in '${selectedProfile}'.\n\nClick OK to DISCARD changes and switch.\nClick Cancel to stay.`);
                                            if (!choice) return;
                                        }

                                        if (confirm(`Switch to profile '${newProfile}'? This will REPLACE your current AppList.`)) {
                                            setSelectedProfile(newProfile);
                                            setLoading(true);
                                            try {
                                                const profile = await ProfileService.loadProfile(newProfile);
                                                await AppListService.nukeAndSort();
                                                if (profile.app_ids.length > 0) await AppListService.addGames(profile.app_ids);
                                                await fetchGames();
                                                setUnsavedChanges(false);
                                            } catch (e) {
                                                alert(`Failed to load profile: ${e}`);
                                            } finally {
                                                setLoading(false);
                                            }
                                        }
                                    }}
                                    className="bg-black/50 border border-white/10 rounded px-3 py-1 text-sm text-white w-48 focus:border-cyan-500 outline-none hover:bg-black/70 transition-colors cursor-pointer appearance-none"
                                    style={{ backgroundImage: 'none' }} // Hide default arrow if we want custom, but native is safer for now
                                >
                                    {profiles.length === 0 && <option value="">No Profiles</option>}
                                    {profiles.map(p => <option key={p} value={p}>{p}</option>)}
                                </select>

                                <button
                                    onClick={() => setIsCreating(true)}
                                    className="text-[10px] font-bold bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 px-2 py-1 rounded hover:bg-cyan-500/20 transition-colors"
                                >
                                    + NEW
                                </button>
                            </div>
                        )}
                    </div>
                </div>

                <div className="flex items-center gap-2 relative z-10">
                    <button
                        onClick={handleSaveProfile}
                        className="px-4 py-2 bg-gradient-to-r from-emerald-600 to-emerald-500 hover:from-emerald-500 hover:to-emerald-400 text-white text-xs font-bold rounded-lg shadow-lg shadow-emerald-900/20 transition-all flex items-center gap-2 group/btn"
                    >
                        <Settings className="w-3 h-3 group-hover/btn:rotate-90 transition-transform" />
                        SAVE
                    </button>
                    {!isCreating && (
                        <button
                            onClick={handleDeleteProfile}
                            disabled={!selectedProfile}
                            className="px-4 py-2 bg-white/5 hover:bg-red-500/20 text-zinc-400 hover:text-red-400 border border-white/5 hover:border-red-500/30 text-xs font-bold rounded-lg transition-all disabled:opacity-30 disabled:pointer-events-none"
                        >
                            DELETE
                        </button>
                    )}
                </div>
            </div>

            {/* Search Bar */}
            <div className="flex gap-2 mb-6">
                <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                    <input
                        type="text"
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        placeholder="Search games..."
                        className="w-full bg-black/50 border border-zinc-800 rounded-lg pl-10 pr-4 py-2 text-white text-sm focus:border-fuchsia-500 outline-none transition-colors"
                    />
                </div>
            </div>

            {/* Games Table */}
            <div className="bg-zinc-900/50 border border-zinc-800 rounded-xl overflow-hidden flex-1 overflow-y-auto custom-scrollbar">
                <table className="w-full text-sm text-left">
                    <thead className="bg-zinc-800/50 text-zinc-400 font-mono text-xs uppercase sticky top-0 backdrop-blur-md z-10">
                        <tr>
                            <th className="p-4 w-16"></th>
                            <th className="p-4">AppID</th>
                            <th className="p-4">Game Name</th>
                            <th className="p-4">Filename</th>
                            <th className="p-4 text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-zinc-800/50">
                        {gameGroups.length === 0 ? (
                            <tr>
                                <td colSpan={5} className="p-8 text-center text-zinc-500 italic">
                                    No games found in AppList.
                                </td>
                            </tr>
                        ) : (
                            gameGroups.map(group => (
                                <GameGroupRow
                                    key={group.root.app_id}
                                    group={group}
                                    expanded={expandedGroups.has(group.root.app_id)}
                                    toggleExpand={() => toggleExpand(group.root.app_id)}
                                    // Remove pendingUpdates prop, rely on group.root.pending_update
                                    updating={updating}
                                    handleUpdate={handleUpdate}
                                    handleDelete={openDeleteModal}
                                />
                            ))
                        )}
                    </tbody>
                </table>
            </div>
            <div className="mt-4 text-xs text-zinc-500 text-right">
                AppList Entries: {games.length} | Root Games: {gameGroups.length}
            </div>

            {/* Delete Modal */}
            <DeleteModal
                isOpen={!!deleteTarget}
                onClose={() => setDeleteTarget(null)}
                onDeleted={onGameDeleted}
                gameId={deleteTarget?.id || ""}
                gameName={deleteTarget?.name || ""}
            />
        </div>
    );
}

interface GameGroup {
    root: GameProfile;
    children: GameProfile[];
}

function GameGroupRow({ group, expanded, toggleExpand, updating, handleUpdate, handleDelete }: any) {
    const { root, children } = group;
    const hasChildren = children.length > 0;

    return (
        <>
            <motion.tr
                layout
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="hover:bg-white/5 transition-colors group cursor-pointer"
                onClick={(e) => {
                    // Don't toggle if clicking buttons
                    if ((e.target as HTMLElement).closest('button')) return;
                    if (hasChildren) toggleExpand();
                }}
            >
                <td className="p-4 text-center">
                    {hasChildren && (
                        <button
                            className={`p-1 rounded hover:bg-white/10 transition-colors ${expanded ? "text-cyan-400" : "text-zinc-600"}`}
                        >
                            {expanded ? "▼" : "▶"}
                        </button>
                    )}
                </td>
                <td className="p-4 font-mono text-cyan-400">
                    {root.app_id}
                    {root.pending_update && (
                        <div className="mt-1 text-[10px] text-amber-500 font-bold animate-pulse">UPDATE</div>
                    )}
                </td>
                <td className="p-4 font-bold text-white">
                    {root.name}
                    {/* Injection Status Badges */}
                    {root.injection_status === "family_godmode" && (
                        <span className="ml-2 text-[9px] font-bold bg-violet-500/20 text-violet-400 px-1.5 py-0.5 rounded-full border border-violet-500/30">FAMILY GODMODE</span>
                    )}
                    {root.injection_status === "injected" && (
                        <span className="ml-2 text-[9px] font-bold bg-emerald-500/20 text-emerald-400 px-1.5 py-0.5 rounded-full border border-emerald-500/30">INJECTED</span>
                    )}
                    {root.injection_status === "family_shared" && (
                        <span className="ml-2 text-[9px] font-bold bg-blue-500/20 text-blue-400 px-1.5 py-0.5 rounded-full border border-blue-500/30">FAMILY</span>
                    )}
                    {root.item_type === "dlc" && (
                        <span className="ml-2 text-[9px] font-bold bg-purple-500/20 text-purple-400 px-1.5 py-0.5 rounded-full">DLC</span>
                    )}
                </td>
                <td className="p-4 text-zinc-500 font-mono text-xs">{root.filename}</td>
                <td className="p-4 text-right flex justify-end gap-2">
                    {/* UPDATE BUTTON */}
                    {root.pending_update && !updating.has(root.app_id) && (
                        <button
                            onClick={(e) => { e.stopPropagation(); handleUpdate(root.app_id); }}
                            className="px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white text-xs font-bold rounded transition-colors shadow-lg shadow-green-900/20 animate-pulse"
                            title={root.pending_update}
                        >
                            UPDATE
                        </button>
                    )}
                    {updating.has(root.app_id) && (
                        <div className="px-3 py-1.5 bg-zinc-800 text-zinc-400 text-xs font-bold rounded flex items-center gap-2">
                            <RefreshCw className="w-3 h-3 animate-spin" /> Updating...
                        </div>
                    )}

                    {/* Steamless (DRM Removal) — ONLY on INSTALLED + INJECTED root games (NOT Family) */}
                    {root.is_installed && root.injection_status !== "family_shared" && (
                        <button
                            onClick={async (e) => {
                                e.stopPropagation();
                                try {
                                    const exePath = await open({
                                        multiple: false,
                                        directory: false,
                                        title: `Select executable for ${root.name}`,
                                        filters: [{ name: 'Executables', extensions: ['exe'] }],
                                    });
                                    if (!exePath || typeof exePath !== 'string') return;
                                    const result = await invoke<string>('run_steamless', { exePath });
                                    alert(`Steamless: ${result}`);
                                } catch (e) {
                                    alert(`Steamless failed: ${e}`);
                                }
                            }}
                            className="p-2 text-zinc-600 hover:text-fuchsia-400 hover:bg-fuchsia-500/10 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
                            title="Run Steamless (DRM Removal)"
                        >
                            <Shield className="w-4 h-4" />
                        </button>
                    )}
                    {/* Goldberg Config — ONLY on INSTALLED + INJECTED root games (NOT Family) */}
                    {root.is_installed && root.injection_status !== "family_shared" && (
                        <button
                            onClick={async (e) => {
                                e.stopPropagation();
                                if (!confirm(`Generate Goldberg emulator config for ${root.name}?`)) return;
                                try {
                                    await invoke('generate_goldberg', { appid: root.app_id });
                                    alert(`Goldberg config generated for ${root.name}!`);
                                } catch (e) {
                                    alert(`Goldberg failed: ${e}`);
                                }
                            }}
                            className="p-2 text-zinc-600 hover:text-cyan-400 hover:bg-cyan-500/10 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
                            title="Generate Goldberg Config"
                        >
                            <Settings className="w-4 h-4" />
                        </button>
                    )}

                    <button
                        onClick={(e) => { e.stopPropagation(); handleDelete(root.app_id, root.name); }}
                        className="p-2 text-zinc-600 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
                        title="Remove Game & DLCs"
                    >
                        <Trash2 className="w-4 h-4" />
                    </button>
                </td>
            </motion.tr>

            {expanded && children.map((child: GameProfile) => (
                <motion.tr
                    key={child.app_id}
                    initial={{ opacity: 0, x: -10 }}
                    animate={{ opacity: 1, x: 0 }}
                    className="bg-black/20 hover:bg-white/5 transition-colors"
                >
                    <td className="p-4"></td> {/* Indent */}
                    <td className="p-4 font-mono text-zinc-500 text-xs pl-8 border-l-2 border-zinc-800">
                        {child.app_id}
                    </td>
                    <td className="p-4 text-zinc-400 text-sm">
                        {child.name.replace("(Content)", "")}
                        {child.item_type === "dlc" && (
                            <span className="text-[9px] font-bold bg-purple-500/20 text-purple-400 px-1.5 py-0.5 rounded-full ml-2">DLC</span>
                        )}
                        {child.item_type !== "dlc" && (
                            <span className="text-[10px] text-zinc-600 bg-zinc-900 px-1 rounded ml-2">DEPOT</span>
                        )}
                    </td>
                    <td className="p-4 text-zinc-600 font-mono text-[10px]">{child.filename}</td>
                    <td className="p-4 text-right">
                        <button
                            onClick={() => handleDelete(child.app_id, child.name)}
                            className="p-1.5 text-zinc-700 hover:text-red-400 hover:bg-red-500/10 rounded transition-colors"
                            title="Remove DLC"
                        >
                            <Trash2 className="w-3 h-3" />
                        </button>
                    </td>
                </motion.tr>
            ))}
        </>
    );
}
