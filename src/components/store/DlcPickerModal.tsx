import { useState, useEffect, useMemo } from "react";
import { motion } from "framer-motion";
import { Search, X, Check } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

interface DlcItem {
    app_id: string;
    name: string;
    depots_count: number;
    available: boolean;
}

interface DlcPickerModalProps {
    appId: string;
    appName: string;
    onConfirm: (selectedDlcs: string[]) => void;
    onCancel: () => void;
}

export function DlcPickerModal({ appId, appName, onConfirm, onCancel }: DlcPickerModalProps) {
    const [dlcs, setDlcs] = useState<DlcItem[]>([]);
    const [selected, setSelected] = useState<Set<string>>(new Set());
    const [search, setSearch] = useState("");
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState("");
    const [applistCount, setApplistCount] = useState(0);

    useEffect(() => {
        loadDlcs();
    }, [appId]);

    const loadDlcs = async () => {
        setLoading(true);
        try {
            const [items, count] = await Promise.all([
                invoke<DlcItem[]>("scan_dlcs", { appid: appId }),
                invoke<number>("get_applist_count"),
            ]);
            setDlcs(items);
            setApplistCount(count);
            // Auto-select all available DLCs
            const autoSelect = new Set<string>();
            items.forEach(d => {
                autoSelect.add(d.app_id);
            });
            setSelected(autoSelect);
        } catch (e) {
            setError(`Failed to scan DLCs: ${e}`);
        } finally {
            setLoading(false);
        }
    };

    const filtered = useMemo(() => {
        if (!search) return dlcs;
        const s = search.toLowerCase();
        return dlcs.filter(d =>
            d.name.toLowerCase().includes(s) || d.app_id.includes(s)
        );
    }, [dlcs, search]);

    const toggle = (id: string) => {
        setSelected(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    };

    const selectAll = () => setSelected(new Set(dlcs.map(d => d.app_id)));
    const deselectAll = () => setSelected(new Set());
    const selectFirstN = (n: number) => {
        const ids = dlcs.slice(0, n).map(d => d.app_id);
        setSelected(new Set(ids));
    };

    // Calculate how many total entries this install will use
    const estimatedEntries = selected.size + 5; // base game + ~4 depots + selected DLCs
    const totalAfterInstall = applistCount + estimatedEntries;

    return (
        <div className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
            <motion.div
                initial={{ scale: 0.9, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.9, opacity: 0 }}
                className="bg-zinc-900/95 border border-zinc-700/50 rounded-2xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col overflow-hidden"
            >
                {/* Header */}
                <div className="p-4 border-b border-zinc-800 flex items-center justify-between">
                    <div>
                        <h3 className="text-lg font-bold text-white">DLC Selection</h3>
                        <p className="text-xs text-zinc-500 mt-0.5">{appName} — {dlcs.length} DLCs found</p>
                    </div>
                    <button onClick={onCancel} className="p-2 text-zinc-500 hover:text-white transition-colors">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Slot Counter */}
                <div className="px-4 py-2 bg-zinc-800/50 border-b border-zinc-800 flex items-center justify-between text-xs">
                    <span className="text-zinc-400">
                        Current AppList: <span className="text-cyan-400 font-mono">{applistCount}</span> entries
                    </span>
                    <span className="text-zinc-400">
                        Selected: <span className="text-cyan-400 font-bold">{selected.size}</span> / {dlcs.length} DLCs
                    </span>
                    <span className={`font-bold ${totalAfterInstall > 10000 ? "text-red-400" : "text-green-400"}`}>
                        After install: ~{totalAfterInstall}
                    </span>
                </div>

                {/* Search + Actions */}
                <div className="px-4 py-2 border-b border-zinc-800 flex gap-2 items-center">
                    <div className="relative flex-1">
                        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                        <input
                            type="text"
                            value={search}
                            onChange={e => setSearch(e.target.value)}
                            placeholder="Filter DLCs..."
                            className="w-full bg-zinc-800 border border-zinc-700 rounded-lg pl-10 pr-3 py-2 text-sm text-white placeholder-zinc-600 focus:outline-none focus:border-cyan-500/50"
                        />
                    </div>
                    <button onClick={selectAll} className="px-3 py-2 text-xs bg-cyan-600/20 text-cyan-400 rounded-lg hover:bg-cyan-600/30 transition-colors font-bold">
                        All
                    </button>
                    <button onClick={deselectAll} className="px-3 py-2 text-xs bg-zinc-700/50 text-zinc-400 rounded-lg hover:bg-zinc-700 transition-colors font-bold">
                        None
                    </button>
                    <button onClick={() => selectFirstN(50)} className="px-3 py-2 text-xs bg-amber-600/20 text-amber-400 rounded-lg hover:bg-amber-600/30 transition-colors font-bold">
                        First 50
                    </button>
                </div>

                {/* DLC List */}
                <div className="flex-1 overflow-y-auto min-h-0">
                    {loading ? (
                        <div className="flex items-center justify-center h-32 text-zinc-500">
                            <div className="animate-spin w-5 h-5 border-2 border-cyan-400 border-t-transparent rounded-full mr-3" />
                            Scanning DLCs...
                        </div>
                    ) : error ? (
                        <div className="p-4 text-red-400 text-sm">{error}</div>
                    ) : (
                        <div className="divide-y divide-zinc-800/50">
                            {filtered.map(dlc => (
                                <label
                                    key={dlc.app_id}
                                    className="flex items-center gap-3 px-4 py-2 hover:bg-white/5 cursor-pointer transition-colors"
                                >
                                    <input
                                        type="checkbox"
                                        checked={selected.has(dlc.app_id)}
                                        onChange={() => toggle(dlc.app_id)}
                                        className="accent-cyan-500 w-4 h-4"
                                    />
                                    <span className="font-mono text-xs text-cyan-400/70 w-20 shrink-0">{dlc.app_id}</span>
                                    <span className="text-sm text-zinc-300 flex-1 truncate">{dlc.name}</span>
                                </label>
                            ))}
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div className="p-4 border-t border-zinc-800 flex items-center justify-between">
                    <button
                        onClick={onCancel}
                        className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={() => onConfirm(Array.from(selected))}
                        disabled={loading}
                        className="px-6 py-2.5 bg-gradient-to-r from-cyan-600 to-blue-600 text-white text-sm font-bold rounded-xl hover:from-cyan-500 hover:to-blue-500 transition-all shadow-lg shadow-cyan-900/20 disabled:opacity-50"
                    >
                        <Check className="w-4 h-4 inline mr-2" />
                        Install with {selected.size} DLCs
                    </button>
                </div>
            </motion.div>
        </div>
    );
}
