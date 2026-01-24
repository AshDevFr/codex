import { beforeEach, describe, expect, it } from "vitest";
import { useCoverUpdatesStore } from "./coverUpdatesStore";

describe("coverUpdatesStore", () => {
	beforeEach(() => {
		// Reset store state between tests
		useCoverUpdatesStore.setState({ updates: {} });
	});

	it("should start with empty updates", () => {
		const state = useCoverUpdatesStore.getState();
		expect(state.updates).toEqual({});
	});

	it("should record a cover update with timestamp", () => {
		const entityId = "series-123";
		const beforeTime = Date.now();

		useCoverUpdatesStore.getState().recordCoverUpdate(entityId);

		const afterTime = Date.now();
		const timestamp = useCoverUpdatesStore.getState().updates[entityId];

		expect(timestamp).toBeDefined();
		expect(timestamp).toBeGreaterThanOrEqual(beforeTime);
		expect(timestamp).toBeLessThanOrEqual(afterTime);
	});

	it("should update timestamp on subsequent cover updates", async () => {
		const entityId = "book-456";

		useCoverUpdatesStore.getState().recordCoverUpdate(entityId);
		const firstTimestamp = useCoverUpdatesStore.getState().updates[entityId];

		// Wait a bit to ensure different timestamp
		await new Promise((resolve) => setTimeout(resolve, 10));

		useCoverUpdatesStore.getState().recordCoverUpdate(entityId);
		const secondTimestamp = useCoverUpdatesStore.getState().updates[entityId];

		expect(secondTimestamp).toBeGreaterThan(firstTimestamp);
	});

	it("should track multiple entities independently", () => {
		const seriesId = "series-123";
		const bookId = "book-456";

		useCoverUpdatesStore.getState().recordCoverUpdate(seriesId);

		// Wait a tiny bit
		const seriesTimestamp = useCoverUpdatesStore.getState().updates[seriesId];

		useCoverUpdatesStore.getState().recordCoverUpdate(bookId);
		const bookTimestamp = useCoverUpdatesStore.getState().updates[bookId];

		// Both should have timestamps
		expect(seriesTimestamp).toBeDefined();
		expect(bookTimestamp).toBeDefined();

		// Series timestamp should be preserved
		expect(useCoverUpdatesStore.getState().updates[seriesId]).toBe(
			seriesTimestamp,
		);
	});

	it("should return undefined for untracked entities via getCoverTimestamp", () => {
		const timestamp = useCoverUpdatesStore
			.getState()
			.getCoverTimestamp("unknown-id");
		expect(timestamp).toBeUndefined();
	});

	it("should return timestamp for tracked entities via getCoverTimestamp", () => {
		const entityId = "series-789";
		useCoverUpdatesStore.getState().recordCoverUpdate(entityId);

		const timestamp = useCoverUpdatesStore
			.getState()
			.getCoverTimestamp(entityId);
		expect(timestamp).toBeDefined();
		expect(typeof timestamp).toBe("number");
	});
});
