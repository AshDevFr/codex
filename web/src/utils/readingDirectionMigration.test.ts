import { beforeEach, describe, expect, it, vi } from "vitest";
import { userSeriesReaderSettingsApi } from "@/api/userSeriesReaderSettings";
import {
  migratedFlagKey,
  migrateSeriesReadingDirections,
} from "./readingDirectionMigration";

vi.mock("@/api/userSeriesReaderSettings", () => ({
  userSeriesReaderSettingsApi: {
    get: vi.fn().mockResolvedValue({}),
    patch: vi.fn().mockResolvedValue({}),
    remove: vi.fn().mockResolvedValue(undefined),
  },
}));

const USER = "user-1";
const SERIES = "11111111-1111-1111-1111-111111111111";

function key(seriesId: string, userId = USER) {
  return `codex-reader-${userId}-series-${seriesId}`;
}

/** A blob as the previous version wrote it: device settings plus a direction. */
function legacyBlob(direction: string) {
  return JSON.stringify({
    fitMode: "width",
    webtoonFitMode: "width",
    pageLayout: "single",
    readingDirection: direction,
    backgroundColor: "black",
    doublePageShowWideAlone: true,
    doublePageStartOnOdd: true,
    createdAt: 1,
    version: 1,
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({});
  vi.mocked(userSeriesReaderSettingsApi.patch).mockResolvedValue({});
});

describe("migrateSeriesReadingDirections", () => {
  it("uploads a direction and strips it from the local blob", async () => {
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));

    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(1);
    expect(userSeriesReaderSettingsApi.patch).toHaveBeenCalledWith(SERIES, {
      readingDirection: "rtl",
    });

    // The six device settings stay exactly where they were.
    const blob = JSON.parse(localStorage.getItem(key(SERIES)) as string);
    expect(blob.readingDirection).toBeUndefined();
    expect(blob.fitMode).toBe("width");
    expect(blob.backgroundColor).toBe("black");
    expect(blob.doublePageStartOnOdd).toBe(true);
    expect(blob.version).toBe(1);
  });

  it("runs once", async () => {
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));
    await migrateSeriesReadingDirections(USER);

    // A second load must not re-upload, nor undo a change made since.
    localStorage.setItem(key(SERIES), legacyBlob("ltr"));
    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(0);
    expect(userSeriesReaderSettingsApi.patch).toHaveBeenCalledTimes(1);
  });

  it("does not overwrite a direction already saved on the account", async () => {
    // Set through the current UI, so it is the more recent intent. Migrating a
    // stale local value over it would undo a correction the user just made.
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "ltr",
    });
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));

    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(0);
    expect(userSeriesReaderSettingsApi.patch).not.toHaveBeenCalled();
    // Still stripped: the server is authoritative from here on.
    const blob = JSON.parse(localStorage.getItem(key(SERIES)) as string);
    expect(blob.readingDirection).toBeUndefined();
  });

  it("retries on the next load when an upload fails", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockRejectedValueOnce(
      new Error("network down"),
    );
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));

    await migrateSeriesReadingDirections(USER);

    // The local value survives and the flag is not set, so nothing is lost.
    const blob = JSON.parse(localStorage.getItem(key(SERIES)) as string);
    expect(blob.readingDirection).toBe("rtl");
    expect(localStorage.getItem(migratedFlagKey(USER))).toBeNull();

    const migrated = await migrateSeriesReadingDirections(USER);
    expect(migrated).toBe(1);
  });

  it("gives up on a series that no longer exists", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockRejectedValue({
      response: { status: 404 },
    });
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));

    await migrateSeriesReadingDirections(USER);

    // Retrying a deleted series every load would never succeed.
    const blob = JSON.parse(localStorage.getItem(key(SERIES)) as string);
    expect(blob.readingDirection).toBeUndefined();
    expect(localStorage.getItem(migratedFlagKey(USER))).toBe("1");
  });

  it("ignores another user's blobs", async () => {
    localStorage.setItem(key(SERIES, "someone-else"), legacyBlob("rtl"));

    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(0);
    expect(userSeriesReaderSettingsApi.patch).not.toHaveBeenCalled();
  });

  it("ignores blobs with nothing to migrate", async () => {
    localStorage.setItem(key(SERIES), '{"fitMode":"width"}');
    localStorage.setItem(key("22222222-2222-2222-2222-222222222222"), "{oops");
    localStorage.setItem(
      key("33333333-3333-3333-3333-333333333333"),
      legacyBlob("sideways"),
    );

    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(0);
    expect(userSeriesReaderSettingsApi.patch).not.toHaveBeenCalled();
    // An unparseable blob is left alone rather than destroyed.
    expect(
      localStorage.getItem(key("22222222-2222-2222-2222-222222222222")),
    ).toBe("{oops");
  });

  it("migrates every series the user had customised", async () => {
    const second = "44444444-4444-4444-4444-444444444444";
    localStorage.setItem(key(SERIES), legacyBlob("rtl"));
    localStorage.setItem(key(second), legacyBlob("webtoon"));

    const migrated = await migrateSeriesReadingDirections(USER);

    expect(migrated).toBe(2);
    expect(userSeriesReaderSettingsApi.patch).toHaveBeenCalledWith(SERIES, {
      readingDirection: "rtl",
    });
    expect(userSeriesReaderSettingsApi.patch).toHaveBeenCalledWith(second, {
      readingDirection: "webtoon",
    });
  });
});
