import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { metadataRefreshApi } from "./metadataRefresh";

vi.mock("./client", () => ({
  api: {
    get: vi.fn(),
    post: vi.fn(),
    patch: vi.fn(),
  },
}));

const LIBRARY_ID = "11111111-1111-1111-1111-111111111111";

describe("metadataRefreshApi", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("get", () => {
    it("fetches the saved config for a library", async () => {
      const config = {
        enabled: true,
        cronSchedule: "0 4 * * *",
        timezone: null,
        fieldGroups: ["ratings", "status", "counts"],
        extraFields: [],
        providers: ["plugin:mangabaka"],
        existingSourceIdsOnly: true,
        skipRecentlySyncedWithinS: 3600,
        maxConcurrency: 4,
      };
      vi.mocked(api.get).mockResolvedValueOnce({ data: config });

      const result = await metadataRefreshApi.get(LIBRARY_ID);

      expect(api.get).toHaveBeenCalledWith(
        `/libraries/${LIBRARY_ID}/metadata-refresh`,
      );
      expect(result).toEqual(config);
    });
  });

  describe("update", () => {
    it("PATCHes only the provided fields", async () => {
      const updated = {
        enabled: true,
        cronSchedule: "0 0 * * *",
        timezone: null,
        fieldGroups: ["ratings"],
        extraFields: [],
        providers: ["plugin:mangabaka"],
        existingSourceIdsOnly: true,
        skipRecentlySyncedWithinS: 3600,
        maxConcurrency: 4,
      };
      vi.mocked(api.patch).mockResolvedValueOnce({ data: updated });

      const result = await metadataRefreshApi.update(LIBRARY_ID, {
        enabled: true,
        fieldGroups: ["ratings"],
      });

      expect(api.patch).toHaveBeenCalledWith(
        `/libraries/${LIBRARY_ID}/metadata-refresh`,
        { enabled: true, fieldGroups: ["ratings"] },
      );
      expect(result).toEqual(updated);
    });
  });

  describe("runNow", () => {
    it("POSTs run-now and returns the task id", async () => {
      const taskId = "22222222-2222-2222-2222-222222222222";
      vi.mocked(api.post).mockResolvedValueOnce({ data: { taskId } });

      const result = await metadataRefreshApi.runNow(LIBRARY_ID);

      expect(api.post).toHaveBeenCalledWith(
        `/libraries/${LIBRARY_ID}/metadata-refresh/run-now`,
      );
      expect(result).toEqual({ taskId });
    });
  });

  describe("dryRun", () => {
    it("posts default body when no override is supplied", async () => {
      const dryRun = {
        sample: [],
        totalEligible: 0,
        estSkippedNoId: 0,
        estSkippedRecentlySynced: 0,
      };
      vi.mocked(api.post).mockResolvedValueOnce({ data: dryRun });

      const result = await metadataRefreshApi.dryRun(LIBRARY_ID);

      expect(api.post).toHaveBeenCalledWith(
        `/libraries/${LIBRARY_ID}/metadata-refresh/dry-run`,
        { configOverride: null, sampleSize: null },
      );
      expect(result).toEqual(dryRun);
    });

    it("forwards configOverride and sampleSize when provided", async () => {
      const config = {
        enabled: true,
        cronSchedule: "0 4 * * *",
        timezone: null,
        fieldGroups: ["ratings"],
        extraFields: [],
        providers: ["plugin:mangabaka"],
        existingSourceIdsOnly: true,
        skipRecentlySyncedWithinS: 3600,
        maxConcurrency: 4,
      };
      const dryRun = {
        sample: [
          {
            seriesId: "33333333-3333-3333-3333-333333333333",
            seriesTitle: "Test",
            provider: "plugin:mangabaka",
            changes: [{ field: "rating", before: 80, after: 82 }],
            skipped: [],
          },
        ],
        totalEligible: 12,
        estSkippedNoId: 0,
        estSkippedRecentlySynced: 0,
      };
      vi.mocked(api.post).mockResolvedValueOnce({ data: dryRun });

      const result = await metadataRefreshApi.dryRun(LIBRARY_ID, {
        configOverride: config,
        sampleSize: 5,
      });

      expect(api.post).toHaveBeenCalledWith(
        `/libraries/${LIBRARY_ID}/metadata-refresh/dry-run`,
        { configOverride: config, sampleSize: 5 },
      );
      expect(result).toEqual(dryRun);
    });
  });

  describe("listFieldGroups", () => {
    it("returns the field group catalog", async () => {
      const groups = [
        { id: "ratings", label: "Ratings", fields: ["rating"] },
        { id: "status", label: "Status", fields: ["status"] },
      ];
      vi.mocked(api.get).mockResolvedValueOnce({ data: groups });

      const result = await metadataRefreshApi.listFieldGroups();

      expect(api.get).toHaveBeenCalledWith("/metadata-refresh/field-groups");
      expect(result).toEqual(groups);
    });
  });
});
