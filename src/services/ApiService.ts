import { invoke } from "@tauri-apps/api/core";

export interface SearchResult {
    game_id: any;
    game_name?: string;
    app_id?: any;
    name?: string;
    is_free: boolean;
    tiny_image?: string;
    logo?: string;
}

export interface DepotInfo {
    name: string;
    config: string;
    gid?: string;
    size?: string;
    category: string;
}

export interface DlcNode {
    appid: string;
    name: string;
}

export interface GameDetails {
    app_id: string;
    name: string;
    short_description: string;
    developers: string[];
    publishers: string[];
    genres: string[];
    release_date: string;
    metacritic_score?: number;
    recommendations?: number;
    platforms: [boolean, boolean, boolean];
    required_age: number;
    depots: Record<string, DepotInfo>;
    dlcs: DlcNode[];
}

export const ApiService = {
    async searchGames(query: string): Promise<SearchResult[]> {
        return invoke("search_games", { query });
    },

    async getGameDetails(appid: string): Promise<GameDetails> {
        return invoke("get_game_details", { appid });
    },

    getCoverUrl(appid: string): Promise<string> {
        return invoke("get_cover_url", { appid });
    }
};
