import { invoke } from "@tauri-apps/api/core";

export interface VaultGame {
    app_id: string;
    name: string;
    size_gb: string;
    timestamp: number;
}

export const VaultService = {
    async listGames(): Promise<VaultGame[]> {
        return await invoke("get_vault_games");
    },

    async backupGame(appId: string): Promise<number> {
        return await invoke("backup_game_cmd", { appid: appId });
    },

    async restoreGame(appId: string): Promise<string> {
        return await invoke("restore_game_cmd", { appid: appId });
    }
};
