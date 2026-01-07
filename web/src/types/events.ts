/**
 * Entity types that can emit change events
 */
export type EntityType = "book" | "series" | "library";

/**
 * Entity change events emitted by the backend
 */
export type EntityEvent =
  | { BookCreated: { book_id: string; library_id: string } }
  | { BookUpdated: { book_id: string; library_id: string; fields: string[] } }
  | { BookDeleted: { book_id: string; library_id: string } }
  | { SeriesCreated: { series_id: string; library_id: string } }
  | { SeriesUpdated: { series_id: string; library_id: string; fields: string[] } }
  | { SeriesDeleted: { series_id: string; library_id: string } }
  | { CoverUpdated: { entity_type: EntityType; entity_id: string } }
  | { LibraryUpdated: { library_id: string } };

/**
 * Complete entity change event with metadata
 */
export interface EntityChangeEvent {
  event: EntityEvent;
  timestamp: string;
  user_id?: string;
}

/**
 * Helper type guards for entity events
 */
export function isBookEvent(event: EntityEvent): event is
  | { BookCreated: { book_id: string; library_id: string } }
  | { BookUpdated: { book_id: string; library_id: string; fields: string[] } }
  | { BookDeleted: { book_id: string; library_id: string } } {
  return "BookCreated" in event || "BookUpdated" in event || "BookDeleted" in event;
}

export function isSeriesEvent(event: EntityEvent): event is
  | { SeriesCreated: { series_id: string; library_id: string } }
  | { SeriesUpdated: { series_id: string; library_id: string; fields: string[] } }
  | { SeriesDeleted: { series_id: string; library_id: string } } {
  return "SeriesCreated" in event || "SeriesUpdated" in event || "SeriesDeleted" in event;
}

export function isCoverEvent(event: EntityEvent): event is
  | { CoverUpdated: { entity_type: EntityType; entity_id: string } } {
  return "CoverUpdated" in event;
}

export function isLibraryEvent(event: EntityEvent): event is
  | { LibraryUpdated: { library_id: string } } {
  return "LibraryUpdated" in event;
}
