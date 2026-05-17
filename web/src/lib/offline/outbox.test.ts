import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { _resetForTests, clearOutbox, getOutbox, setDbContext } from "./db";
import {
  _resetOutboxLifecycleForTests,
  drainOfflineOutbox,
  enqueueOfflineWrite,
  installOutboxDrainListeners,
  isOfflineError,
  isOfflineQueuedError,
  OfflineQueuedError,
} from "./outbox";

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(async () => {
  _resetOutboxLifecycleForTests();
  await clearOutbox().catch(() => {});
  setDbContext(null);
  _resetForTests();
});

describe("isOfflineError", () => {
  it("recognises the project's ApiError network shape", () => {
    expect(isOfflineError({ error: "Network Error", message: "..." })).toBe(
      true,
    );
  });

  it("recognises raw axios ERR_NETWORK / ECONNABORTED codes", () => {
    expect(isOfflineError({ code: "ERR_NETWORK" })).toBe(true);
    expect(isOfflineError({ code: "ECONNABORTED" })).toBe(true);
  });

  it("recognises errors with no response and a network-flavoured message", () => {
    expect(
      isOfflineError({
        message: "Network request failed",
        response: undefined,
      }),
    ).toBe(true);
    expect(
      isOfflineError({ message: "fetch failed", response: undefined }),
    ).toBe(true);
  });

  it("returns false for server errors (response present)", () => {
    expect(
      isOfflineError({ error: "Internal", response: { status: 500 } }),
    ).toBe(false);
  });

  it("returns true when navigator.onLine is false even on an opaque error", () => {
    const originalDescriptor = Object.getOwnPropertyDescriptor(
      globalThis.navigator,
      "onLine",
    );
    Object.defineProperty(globalThis.navigator, "onLine", {
      configurable: true,
      value: false,
    });
    try {
      expect(isOfflineError(new Error("whatever"))).toBe(true);
    } finally {
      if (originalDescriptor) {
        Object.defineProperty(
          globalThis.navigator,
          "onLine",
          originalDescriptor,
        );
      } else {
        Object.defineProperty(globalThis.navigator, "onLine", {
          configurable: true,
          value: true,
        });
      }
    }
  });

  it("returns false for null / non-objects", () => {
    expect(isOfflineError(null)).toBe(false);
    expect(isOfflineError("string")).toBe(false);
  });
});

describe("OfflineQueuedError + isOfflineQueuedError", () => {
  it("isOfflineQueuedError narrows the type", () => {
    const err = new OfflineQueuedError({ url: "/x", method: "PUT" });
    expect(isOfflineQueuedError(err)).toBe(true);
    expect(isOfflineQueuedError(new Error("nope"))).toBe(false);
    expect(err.request.url).toBe("/x");
  });
});

describe("enqueueOfflineWrite", () => {
  it("normalises method to uppercase and JSON-encodes the body", async () => {
    const key = await enqueueOfflineWrite({
      url: "/api/v1/books/abc/progress",
      method: "put",
      headers: { Authorization: "Bearer t" },
      body: { currentPage: 42, completed: false },
    });
    expect(typeof key).toBe("number");
    const stored = await getOutbox();
    expect(stored).toHaveLength(1);
    expect(stored[0]?.request.method).toBe("PUT");
    expect(stored[0]?.request.body).toBe(
      JSON.stringify({ currentPage: 42, completed: false }),
    );
    expect(stored[0]?.request.headers.Authorization).toBe("Bearer t");
  });

  it("leaves body undefined when none is provided", async () => {
    await enqueueOfflineWrite({
      url: "/x",
      method: "DELETE",
    });
    const stored = await getOutbox();
    expect(stored[0]?.request.body).toBeUndefined();
  });
});

describe("drainOfflineOutbox", () => {
  it("replays each queued request in insertion order and clears the outbox", async () => {
    await enqueueOfflineWrite({ url: "/a", method: "PUT" });
    await enqueueOfflineWrite({ url: "/b", method: "PUT" });
    await enqueueOfflineWrite({ url: "/c", method: "PUT" });

    const sent: string[] = [];
    const result = await drainOfflineOutbox(async (record) => {
      sent.push(record.request.url);
    });
    expect(result).toBe(3);
    expect(sent).toEqual(["/a", "/b", "/c"]);
    expect(await getOutbox()).toEqual([]);
  });

  it("stops at the first failure (record stays at head, retryCount bumps)", async () => {
    await enqueueOfflineWrite({ url: "/ok", method: "PUT" });
    await enqueueOfflineWrite({ url: "/fail", method: "PUT" });
    await enqueueOfflineWrite({ url: "/never", method: "PUT" });

    const sent = await drainOfflineOutbox(async (record) => {
      if (record.request.url === "/fail") throw new Error("boom");
    });
    expect(sent).toBe(1);
    const remaining = await getOutbox();
    expect(remaining.map((r) => r.request.url)).toEqual(["/fail", "/never"]);
    expect(remaining[0]?.retryCount).toBe(1);
  });

  it("deduplicates concurrent in-flight drains", async () => {
    await enqueueOfflineWrite({ url: "/a", method: "PUT" });
    await enqueueOfflineWrite({ url: "/b", method: "PUT" });

    const seen: string[] = [];
    const slowSend = async (record: { request: { url: string } }) => {
      seen.push(record.request.url);
      await new Promise((res) => setTimeout(res, 0));
    };

    const drain1 = drainOfflineOutbox(slowSend as never);
    const drain2 = drainOfflineOutbox(slowSend as never);

    expect(drain1).toBe(drain2);
    const [a, b] = await Promise.all([drain1, drain2]);
    expect(a).toBe(2);
    expect(b).toBe(2);
    expect(seen).toEqual(["/a", "/b"]);
  });
});

describe("installOutboxDrainListeners", () => {
  it("drains the outbox when the window fires `online`", async () => {
    const drainSpy = vi.fn(async (_record: unknown) => undefined);
    // Pre-seed two queued writes so the drain has something to do.
    await enqueueOfflineWrite({ url: "/a", method: "PUT" });
    await enqueueOfflineWrite({ url: "/b", method: "PUT" });

    // Default sender uses fetch; stub it instead of relying on a spy because
    // the listener-installed handler does not accept a sender argument.
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockImplementation(async (input) => {
        drainSpy({ request: { url: String(input) } });
        return new Response(null, { status: 200 });
      });

    installOutboxDrainListeners();
    window.dispatchEvent(new Event("online"));
    // Poll until the drain has flushed both records (sequential IDB
    // round-trips need a few microtask + macrotask ticks beyond a single
    // `setTimeout(0)`).
    await vi.waitFor(async () => {
      expect(await getOutbox()).toEqual([]);
    });

    expect(fetchSpy).toHaveBeenCalledTimes(2);
    fetchSpy.mockRestore();
  });

  it("is idempotent: calling install twice does not register duplicate listeners", async () => {
    const addSpy = vi.spyOn(window, "addEventListener");
    installOutboxDrainListeners();
    installOutboxDrainListeners();
    // 1 for `online` + 1 for `visibilitychange` on the document.
    // The window-level addEventListener spy only sees `online`.
    const onlineCalls = addSpy.mock.calls.filter((c) => c[0] === "online");
    expect(onlineCalls).toHaveLength(1);
    addSpy.mockRestore();
  });

  it("drains on visibilitychange when the tab becomes visible", async () => {
    await enqueueOfflineWrite({ url: "/x", method: "PUT" });

    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(null, { status: 200 }));

    installOutboxDrainListeners();
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.waitFor(async () => {
      expect(await getOutbox()).toEqual([]);
    });

    expect(fetchSpy).toHaveBeenCalledTimes(1);
    fetchSpy.mockRestore();
  });
});
