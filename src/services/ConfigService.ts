import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
    steam_path: string;
    gl_path: string;
    steamless_path: string;
    api_key: string;
    enable_stealth_mode: boolean;
    active_profile: string;
    target_language: string;
}

export const ConfigService = {
    async getConfig(): Promise<AppConfig> {
        return await invoke("get_config");
    },

    async saveConfig(config: AppConfig): Promise<void> {
        await invoke("save_config", { config });
    }
};
