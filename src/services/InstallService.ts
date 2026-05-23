import { invoke } from "@tauri-apps/api/core";

export const InstallService = {
    // Phase 4: Godmode Installer
    async installGodmode(appid: string, includeDlcs: boolean): Promise<void> {
        return invoke("install_godmode", { appid, includeDlcs });
    },

    // Trigger Steam Protocol Install
    async triggerSteamInstall(appid: string): Promise<void> {
        return invoke("trigger_steam_install", { appid });
    },

    // Get Library Folders
    async getLibraryFolders(): Promise<string[]> {
        return invoke("get_library_folders");
    },

    // Detect Default Install Path
    async detectInstallPath(appid: string, name: string, library: string): Promise<string> {
        return invoke("detect_install_path", { appid, name, library });
    },

    // Phase 4: Resolve Installation IDs (Legacy Logic)
    async resolveInstallIds(appid: string, selectedDlcs: string[]): Promise<string[]> {
        return invoke("resolve_install_ids", { appid, selectedDlcs });
    }
};
