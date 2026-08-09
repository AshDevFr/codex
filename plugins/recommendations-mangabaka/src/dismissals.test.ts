import type { PluginStorage } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it, vi } from "vitest";
import { DISMISSED_STORAGE_KEY, DismissalStore } from "./dismissals.js";

/** A stub storage backed by a plain object. */
function fakeStorage(initial: unknown = null) {
  const state = { value: initial };
  const storage = {
    get: vi.fn(async () => ({ data: state.value })),
    set: vi.fn(async (_key: string, value: unknown) => {
      state.value = value;
      return { success: true };
    }),
  } as unknown as PluginStorage;
  return { storage, state };
}

/** A storage stub whose every operation rejects. */
function brokenStorage() {
  return {
    get: vi.fn(async () => {
      throw new Error("storage unavailable");
    }),
    set: vi.fn(async () => {
      throw new Error("storage unavailable");
    }),
  } as unknown as PluginStorage;
}

describe("DismissalStore", () => {
  it("starts empty", () => {
    const store = new DismissalStore();

    expect(store.size).toBe(0);
    expect(store.has("1")).toBe(false);
  });

  it("hydrates previously persisted IDs", async () => {
    const { storage } = fakeStorage(["10", "20"]);
    const store = new DismissalStore();

    await store.hydrate(storage);

    expect(store.has("10")).toBe(true);
    expect(store.has("20")).toBe(true);
    expect(store.size).toBe(2);
  });

  it("reads from the documented storage key", async () => {
    const { storage } = fakeStorage([]);
    const store = new DismissalStore();

    await store.hydrate(storage);

    expect(storage.get).toHaveBeenCalledWith(DISMISSED_STORAGE_KEY);
  });

  it("persists a dismissal", async () => {
    const { storage, state } = fakeStorage([]);
    const store = new DismissalStore();
    await store.hydrate(storage);

    await store.add("42");

    expect(store.has("42")).toBe(true);
    expect(state.value).toEqual(["42"]);
  });

  it("clears every dismissal and reports how many went", async () => {
    const { storage, state } = fakeStorage(["1", "2", "3"]);
    const store = new DismissalStore();
    await store.hydrate(storage);

    const cleared = await store.clear();

    expect(cleared).toBe(3);
    expect(store.size).toBe(0);
    expect(state.value).toEqual([]);
  });

  it("ignores non-string entries in stored data", async () => {
    // Guards against a corrupted or hand-edited storage row.
    const { storage } = fakeStorage(["ok", 42, null, { id: 1 }]);
    const store = new DismissalStore();

    await store.hydrate(storage);

    expect(store.size).toBe(1);
    expect(store.has("ok")).toBe(true);
  });

  it("ignores stored data that is not an array", async () => {
    const { storage } = fakeStorage({ unexpected: "shape" });
    const store = new DismissalStore();

    await store.hydrate(storage);

    expect(store.size).toBe(0);
  });

  it("treats a missing storage row as no dismissals", async () => {
    const { storage } = fakeStorage(null);
    const store = new DismissalStore();

    await store.hydrate(storage);

    expect(store.size).toBe(0);
  });

  it("survives a failing read without throwing", async () => {
    // Losing dismissals degrades the result set; failing the whole
    // recommendation run over it would be worse.
    const store = new DismissalStore();

    await expect(store.hydrate(brokenStorage())).resolves.toBeUndefined();
    expect(store.size).toBe(0);
  });

  it("survives a failing write and keeps the in-memory state", async () => {
    const store = new DismissalStore();
    await store.hydrate(brokenStorage());

    await expect(store.add("7")).resolves.toBeUndefined();
    // The dismissal still applies for the rest of this process, even though it
    // will not survive a restart.
    expect(store.has("7")).toBe(true);
  });

  it("works with no storage attached at all", async () => {
    const store = new DismissalStore();

    await store.add("5");

    expect(store.has("5")).toBe(true);
    expect(await store.clear()).toBe(1);
  });

  it("re-hydrating replaces rather than merges", async () => {
    const store = new DismissalStore();
    await store.hydrate(fakeStorage(["1"]).storage);
    await store.hydrate(fakeStorage(["2"]).storage);

    expect(store.has("1")).toBe(false);
    expect(store.has("2")).toBe(true);
  });
});
