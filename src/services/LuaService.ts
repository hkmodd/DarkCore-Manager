import { invoke } from "@tauri-apps/api/core";

export interface DepotInfo {
    depot_id: number;
    depot_key: string;
    manifest_id?: number;
    name?: string;
    category: "Unknown" | "MainApp" | "MainDepot" | "SharedDepot" | "DlcDepot";
}

export interface DlcInfo {
    app_id: number;
    name: string;
}

export interface ScriptData {
    app_id?: number;
    app_name?: string;
    depots: DepotInfo[];
    dlcs: DlcInfo[];
}

export const LuaService = {
    async parseScript(path: string): Promise<ScriptData> {
        return await invoke("parse_lua_script", { path });
    }
};
