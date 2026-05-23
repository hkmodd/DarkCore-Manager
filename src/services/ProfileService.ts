import { invoke } from "@tauri-apps/api/core";

export interface Profile {
    name: string;
    app_ids: string[];
}

export const ProfileService = {
    async listProfiles(): Promise<string[]> {
        return await invoke("get_profiles");
    },

    async saveProfile(profile: Profile): Promise<void> {
        await invoke("save_profile", { profile });
    },

    async loadProfile(name: string): Promise<Profile> {
        return await invoke("load_profile", { name });
    },

    async deleteProfile(name: string): Promise<void> {
        await invoke("delete_profile", { name });
    }
};
