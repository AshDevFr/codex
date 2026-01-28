/**
 * MangaBaka API response types
 * Based on: https://api.mangabaka.dev/
 */

/**
 * Standard API response wrapper
 */
export interface MbApiResponse<T> {
  status: number;
  data: T;
  pagination?: MbPagination;
}

export interface MbPagination {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
}

/**
 * Series type enum
 */
export type MbSeriesType = "manga" | "novel" | "manhwa" | "manhua" | "oel" | "other";

/**
 * Publication status enum
 */
export type MbStatus = "cancelled" | "completed" | "hiatus" | "releasing" | "unknown" | "upcoming";

/**
 * Content rating enum
 */
export type MbContentRating = "safe" | "suggestive" | "erotica" | "pornographic";

/**
 * Genre enum
 */
export type MbGenre =
  | "action"
  | "adult"
  | "adventure"
  | "avant_garde"
  | "award_winning"
  | "boys_love"
  | "comedy"
  | "doujinshi"
  | "drama"
  | "ecchi"
  | "erotica"
  | "fantasy"
  | "gender_bender"
  | "girls_love"
  | "gourmet"
  | "harem"
  | "hentai"
  | "historical"
  | "horror"
  | "josei"
  | "lolicon"
  | "mahou_shoujo"
  | "martial_arts"
  | "mature"
  | "mecha"
  | "music"
  | "mystery"
  | "psychological"
  | "romance"
  | "school_life"
  | "sci-fi"
  | "seinen"
  | "shotacon"
  | "shoujo"
  | "shoujo_ai"
  | "shounen"
  | "shounen_ai"
  | "slice_of_life"
  | "smut"
  | "sports"
  | "supernatural"
  | "suspense"
  | "thriller"
  | "tragedy"
  | "yaoi"
  | "yuri";

/**
 * Series state
 */
export type MbSeriesState = "active" | "merged" | "deleted";

/**
 * Publisher information
 */
export interface MbPublisher {
  name: string;
  type: string;
  note?: string | null;
}

/**
 * Cover image structure
 */
export interface MbCover {
  raw: {
    url: string | null;
    size?: number | null;
    height?: number | null;
    width?: number | null;
    blurhash?: string | null;
    thumbhash?: string | null;
    format?: string | null;
  };
  x150: MbScaledImage;
  x250: MbScaledImage;
  x350: MbScaledImage;
}

export interface MbScaledImage {
  x1: string | null;
  x2: string | null;
  x3: string | null;
}

/**
 * Secondary title entry
 */
export interface MbSecondaryTitle {
  type: "alternative" | "native" | "official" | "unofficial";
  title: string;
  note?: string | null;
}

/**
 * Secondary titles by language code
 */
export interface MbSecondaryTitles {
  [languageCode: string]: MbSecondaryTitle[] | null;
}

/**
 * Source information (e.g., anilist, mal, etc.)
 */
export interface MbSourceInfo {
  id: number | string | null;
  rating?: number | null;
  rating_normalized?: number | null;
}

/**
 * Series relationships
 */
export interface MbRelationships {
  main_story?: number[];
  adaptation?: number[];
  prequel?: number[];
  sequel?: number[];
  side_story?: number[];
  spin_off?: number[];
  alternative?: number[];
  other?: number[];
}

/**
 * Series data from search or get endpoints
 */
export interface MbSeries {
  id: number;
  state: MbSeriesState;
  merged_with?: number | null;
  title: string;
  native_title?: string | null;
  romanized_title?: string | null;
  secondary_titles?: MbSecondaryTitles | null;
  cover: MbCover;
  authors?: string[] | null;
  artists?: string[] | null;
  publishers?: MbPublisher[] | null;
  description?: string | null;
  year?: number | null;
  final_volume?: string | null;
  status: MbStatus;
  is_licensed?: boolean;
  has_anime?: boolean;
  type: MbSeriesType;
  country_of_origin?: string | null;
  content_rating?: MbContentRating | null;
  genres?: MbGenre[] | null;
  tags?: string[] | null;
  relationships?: MbRelationships | null;
  source?: {
    anilist?: MbSourceInfo;
    my_anime_list?: MbSourceInfo;
    mangadex?: MbSourceInfo;
    manga_updates?: MbSourceInfo;
    kitsu?: MbSourceInfo;
    anime_planet?: MbSourceInfo;
    anime_news_network?: MbSourceInfo;
    shikimori?: MbSourceInfo;
    [key: string]: MbSourceInfo | undefined;
  };
  rating?:
    | number
    | {
        average?: number | null;
        bayesian?: number | null;
        distribution?: Record<string, number> | null;
      }
    | null;
  last_updated_at?: string | null;
}

/**
 * Search response - array of series
 */
export type MbSearchResponse = MbApiResponse<MbSeries[]>;

/**
 * Get series response - single series
 */
export type MbGetSeriesResponse = MbApiResponse<MbSeries>;
