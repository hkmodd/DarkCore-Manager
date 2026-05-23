import { invoke } from "@tauri-apps/api/core";

export interface GameProfile {
    app_id: string;
    name: string;
    filename: string;
    parent_id: string | null;
    item_type: "game" | "dlc" | "depot";
    is_installed: boolean;
    injection_status: "injected" | "family_godmode" | "family_shared";
    pending_update: string | null;
}

export const AppListService = {
    // Get active games from GreenLuma AppList folder
    async getActiveGames(): Promise<GameProfile[]> {
        return await invoke('get_active_games');
    },

    // Add list of AppIDs to AppList
    async addGames(ids: string[]): Promise<void> {
        await invoke('add_games_to_list', { ids });
    },

    // Nuke and Sort AppList (Clean up)
    async nukeAndSort(): Promise<void> {
        await invoke('reorder_list');
    },

    // Update backend name cache (for sorting/display)
    async updateNameCache(cache: Record<string, string>): Promise<void> {
        await invoke('update_name_cache', { cacheUpdate: cache });
    },

    // Inject Decryption Keys into config.vdf
    async injectVdfKeys(keys: Record<string, string>): Promise<void> {
        await invoke('inject_vdf_keys', { keys });
    },

    // Remove a game by ID and reorder
    async removeGame(id: string): Promise<void> {
        await invoke('remove_game_from_applist', { id });
    },

    // Trigger async update scan
    async scanForUpdates(): Promise<void> {
        await invoke('scan_for_updates_async');
    },

    // Download missing manifests for a game
    async updateGameManifests(appId: string): Promise<string> {
        return await invoke('update_game_manifests', { appId });
    }
};
