import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  _resetForTests,
  clearDownloads,
  clearOutbox,
  type DownloadRecord,
  deleteDownload,
  deleteOutboxEntry,
  drainOutbox,
  enqueueOutbox,
  getAllDownloads,
  getDownload,
  getOutbox,
  putDownload,
  setDbContext,
} from "./db";

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
});

function makeDownload(
  id: string,
  overrides: Partial<DownloadRecord> = {},
): DownloadRecord {
  return {
    id,
    format: "epub",
    status: "complete",
    bytes: 1024,
    pageCount: 1,
    downloadedAt: 1_700_000_000_000,
    ...overrides,
  };
}

describe("offline db — downloads store", () => {
  it("round-trips put/get for a single record", async () => {
    const rec = makeDownload("book-1");
    await putDownload(rec);
    const back = await getDownload("book-1");
    expect(back).toEqual(rec);
  });

  it("getAllDownloads returns every stored record", async () => {
    await putDownload(makeDownload("a"));
    await putDownload(makeDownload("b", { format: "pdf" }));
    await putDownload(makeDownload("c", { format: "comic", pageCount: 24 }));
    const all = await getAllDownloads();
    expect(all.map((r) => r.id).sort()).toEqual(["a", "b", "c"]);
  });

  it("put overwrites the existing record for the same id", async () => {
    await putDownload(
      makeDownload("book-1", { status: "downloading", bytes: 100 }),
    );
    await putDownload(
      makeDownload("book-1", { status: "complete", bytes: 500 }),
    );
    const back = await getDownload("book-1");
    expect(back?.status).toBe("complete");
    expect(back?.bytes).toBe(500);
  });

  it("deleteDownload removes a record without touching siblings", async () => {
    await putDownload(makeDownload("a"));
    await putDownload(makeDownload("b"));
    await deleteDownload("a");
    expect(await getDownload("a")).toBeUndefined();
    expect(await getDownload("b")).toBeDefined();
  });

  it("clearDownloads empties the store", async () => {
    await putDownload(makeDownload("a"));
    await putDownload(makeDownload("b"));
    await clearDownloads();
    expect(await getAllDownloads()).toEqual([]);
  });

  it("returns undefined for an unknown id", async () => {
    expect(await getDownload("nope")).toBeUndefined();
  });
});

describe("offline db — outbox store", () => {
  it("enqueue returns an auto-incremented key and getOutbox returns the record", async () => {
    const key = await enqueueOutbox({
      url: "/api/v1/books/abc/progress",
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ page: 5 }),
    });
    expect(typeof key).toBe("number");
    const all = await getOutbox();
    expect(all).toHaveLength(1);
    expect(all[0]?.id).toBe(key);
    expect(all[0]?.request.url).toBe("/api/v1/books/abc/progress");
    expect(all[0]?.retryCount).toBe(0);
  });

  it("preserves insertion order in the auto-incremented keys", async () => {
    const k1 = await enqueueOutbox({ url: "/a", method: "PUT", headers: {} });
    const k2 = await enqueueOutbox({ url: "/b", method: "PUT", headers: {} });
    const k3 = await enqueueOutbox({ url: "/c", method: "PUT", headers: {} });
    expect(k1).toBeLessThan(k2);
    expect(k2).toBeLessThan(k3);
  });

  it("deleteOutboxEntry removes a single entry", async () => {
    const k1 = await enqueueOutbox({ url: "/a", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/b", method: "PUT", headers: {} });
    await deleteOutboxEntry(k1);
    const remaining = await getOutbox();
    expect(remaining.map((r) => r.request.url)).toEqual(["/b"]);
  });

  it("clearOutbox empties the store", async () => {
    await enqueueOutbox({ url: "/a", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/b", method: "PUT", headers: {} });
    await clearOutbox();
    expect(await getOutbox()).toEqual([]);
  });
});

describe("offline db — drainOutbox", () => {
  it("sends each record in order and removes it on success", async () => {
    await enqueueOutbox({ url: "/1", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/2", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/3", method: "PUT", headers: {} });

    const sentUrls: string[] = [];
    const sent = await drainOutbox(async (record) => {
      sentUrls.push(record.request.url);
    });

    expect(sent).toBe(3);
    expect(sentUrls).toEqual(["/1", "/2", "/3"]);
    expect(await getOutbox()).toEqual([]);
  });

  it("stops draining on first failure and bumps retryCount on the failed record", async () => {
    await enqueueOutbox({ url: "/1", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/2", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/3", method: "PUT", headers: {} });

    const sent = await drainOutbox(async (record) => {
      if (record.request.url === "/2") {
        throw new Error("boom");
      }
    });

    expect(sent).toBe(1);
    const remaining = await getOutbox();
    expect(remaining.map((r) => r.request.url)).toEqual(["/2", "/3"]);
    const failed = remaining.find((r) => r.request.url === "/2");
    expect(failed?.retryCount).toBe(1);
  });

  it("returns 0 and does not error on an empty outbox", async () => {
    const sent = await drainOutbox(async () => {
      throw new Error("should not be called");
    });
    expect(sent).toBe(0);
  });

  it("a second drain attempt picks up where the previous one stopped", async () => {
    await enqueueOutbox({ url: "/1", method: "PUT", headers: {} });
    await enqueueOutbox({ url: "/2", method: "PUT", headers: {} });

    let failOnce = true;
    await drainOutbox(async (record) => {
      if (record.request.url === "/1" && failOnce) {
        failOnce = false;
        throw new Error("transient");
      }
    });

    // /1 is still queued with retryCount=1, /2 not yet attempted.
    let remaining = await getOutbox();
    expect(remaining.map((r) => r.request.url)).toEqual(["/1", "/2"]);

    const sent = await drainOutbox(async () => {
      // succeed this time
    });
    expect(sent).toBe(2);
    remaining = await getOutbox();
    expect(remaining).toEqual([]);
  });
});
