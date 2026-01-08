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
	| {
			SeriesUpdated: {
				series_id: string;
				library_id: string;
				fields: string[];
			};
	  }
	| { SeriesDeleted: { series_id: string; library_id: string } }
	| {
			SeriesBulkPurged: {
				series_id: string;
				library_id: string;
				count: number;
			};
	  }
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
export function isBookEvent(
	event: EntityEvent,
): event is
	| { BookCreated: { book_id: string; library_id: string } }
	| { BookUpdated: { book_id: string; library_id: string; fields: string[] } }
	| { BookDeleted: { book_id: string; library_id: string } } {
	return (
		"BookCreated" in event || "BookUpdated" in event || "BookDeleted" in event
	);
}

export function isSeriesEvent(
	event: EntityEvent,
): event is
	| { SeriesCreated: { series_id: string; library_id: string } }
	| {
			SeriesUpdated: {
				series_id: string;
				library_id: string;
				fields: string[];
			};
	  }
	| { SeriesDeleted: { series_id: string; library_id: string } }
	| {
			SeriesBulkPurged: {
				series_id: string;
				library_id: string;
				count: number;
			};
	  } {
	return (
		"SeriesCreated" in event ||
		"SeriesUpdated" in event ||
		"SeriesDeleted" in event ||
		"SeriesBulkPurged" in event
	);
}

export function isCoverEvent(
	event: EntityEvent,
): event is { CoverUpdated: { entity_type: EntityType; entity_id: string } } {
	return "CoverUpdated" in event;
}

export function isLibraryEvent(
	event: EntityEvent,
): event is { LibraryUpdated: { library_id: string } } {
	return "LibraryUpdated" in event;
}

/**
 * Task status for progress tracking
 */
export type TaskStatus = "pending" | "running" | "completed" | "failed";

/**
 * Progress information for a running task
 */
export interface TaskProgress {
	current: number;
	total: number;
	message?: string;
}

/**
 * Task progress event for background operations
 */
export interface TaskProgressEvent {
	task_id: string;
	task_type: string;
	status: TaskStatus;
	progress?: TaskProgress;
	error?: string;
	started_at: string;
	completed_at?: string;
	library_id?: string;
	series_id?: string;
	book_id?: string;
}
