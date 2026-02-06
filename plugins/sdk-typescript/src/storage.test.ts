/**
 * Tests for storage.ts - Plugin storage client
 *
 * These tests cover:
 * - Storage type interfaces and structure
 * - StorageError class
 * - PluginStorage request building and response handling
 * - Full round-trip request/response flow
 */

import { describe, expect, it } from "vitest";
import {
  PluginStorage,
  type StorageClearResponse,
  type StorageDeleteResponse,
  StorageError,
  type StorageGetResponse,
  type StorageKeyEntry,
  type StorageListResponse,
  type StorageSetResponse,
} from "./storage.js";

// =============================================================================
// StorageError Tests
// =============================================================================

describe("StorageError", () => {
  it("should create error with message and code", () => {
    const err = new StorageError("Not found", -32002);
    expect(err.message).toBe("Not found");
    expect(err.code).toBe(-32002);
    expect(err.data).toBeUndefined();
    expect(err.name).toBe("StorageError");
  });

  it("should create error with message, code, and data", () => {
    const err = new StorageError("Invalid params", -32602, { field: "key" });
    expect(err.message).toBe("Invalid params");
    expect(err.code).toBe(-32602);
    expect(err.data).toEqual({ field: "key" });
  });

  it("should be instanceof Error", () => {
    const err = new StorageError("test", -1);
    expect(err).toBeInstanceOf(Error);
    expect(err).toBeInstanceOf(StorageError);
  });
});

// =============================================================================
// Storage Type Tests (compile-time + runtime shape validation)
// =============================================================================

describe("Storage Types", () => {
  it("should accept valid StorageGetResponse shape", () => {
    const response: StorageGetResponse = {
      data: { genres: ["action", "drama"] },
      expiresAt: "2026-12-31T23:59:59Z",
    };
    expect(response.data).toEqual({ genres: ["action", "drama"] });
    expect(response.expiresAt).toBe("2026-12-31T23:59:59Z");
  });

  it("should accept StorageGetResponse with null data", () => {
    const response: StorageGetResponse = { data: null };
    expect(response.data).toBeNull();
  });

  it("should accept valid StorageSetResponse shape", () => {
    const response: StorageSetResponse = { success: true };
    expect(response.success).toBe(true);
  });

  it("should accept valid StorageDeleteResponse shape", () => {
    const response: StorageDeleteResponse = { deleted: true };
    expect(response.deleted).toBe(true);

    const notDeleted: StorageDeleteResponse = { deleted: false };
    expect(notDeleted.deleted).toBe(false);
  });

  it("should accept valid StorageKeyEntry shape", () => {
    const entry: StorageKeyEntry = {
      key: "taste_profile",
      updatedAt: "2026-02-06T10:00:00Z",
    };
    expect(entry.key).toBe("taste_profile");
    expect(entry.updatedAt).toBe("2026-02-06T10:00:00Z");
  });

  it("should accept StorageKeyEntry with expiresAt", () => {
    const entry: StorageKeyEntry = {
      key: "cache",
      expiresAt: "2026-02-07T00:00:00Z",
      updatedAt: "2026-02-06T11:00:00Z",
    };
    expect(entry.expiresAt).toBe("2026-02-07T00:00:00Z");
  });

  it("should accept valid StorageListResponse shape", () => {
    const response: StorageListResponse = {
      keys: [
        { key: "profile", updatedAt: "2026-02-06T10:00:00Z" },
        { key: "cache", expiresAt: "2026-02-07T00:00:00Z", updatedAt: "2026-02-06T11:00:00Z" },
      ],
    };
    expect(response.keys).toHaveLength(2);
    expect(response.keys[0].key).toBe("profile");
    expect(response.keys[1].expiresAt).toBe("2026-02-07T00:00:00Z");
  });

  it("should accept valid StorageClearResponse shape", () => {
    const response: StorageClearResponse = { deletedCount: 5 };
    expect(response.deletedCount).toBe(5);
  });
});

// =============================================================================
// Helper: Create a testable storage client with captured writes
// =============================================================================

function createTestStorage() {
  const written: string[] = [];
  const writeFn = (line: string) => {
    written.push(line);
  };
  const storage = new PluginStorage(writeFn);
  return { storage, written };
}

/** Simulate a successful JSON-RPC response for a given request ID */
function successResponse(id: number, result: unknown): string {
  return JSON.stringify({ jsonrpc: "2.0", id, result });
}

/** Simulate an error JSON-RPC response */
function errorResponse(id: number, code: number, message: string): string {
  return JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } });
}

// =============================================================================
// PluginStorage Request Building Tests
// =============================================================================

describe("PluginStorage - Request Building", () => {
  it("should build correct JSON-RPC request for storage/get", () => {
    const { storage, written } = createTestStorage();

    // Start the request (won't resolve yet - no response delivered)
    const promise = storage.get("taste_profile");

    expect(written).toHaveLength(1);
    const request = JSON.parse(written[0].trim());
    expect(request.jsonrpc).toBe("2.0");
    expect(request.method).toBe("storage/get");
    expect(request.params).toEqual({ key: "taste_profile" });
    expect(request.id).toBe(1);

    // Deliver response to resolve the promise
    storage.handleResponse(successResponse(1, { data: null }));
    return promise; // Let vitest verify it resolves
  });

  it("should build correct JSON-RPC request for storage/set", () => {
    const { storage, written } = createTestStorage();

    const promise = storage.set("profile", { version: 1 });

    const request = JSON.parse(written[0].trim());
    expect(request.method).toBe("storage/set");
    expect(request.params).toEqual({ key: "profile", data: { version: 1 } });

    storage.handleResponse(successResponse(1, { success: true }));
    return promise;
  });

  it("should build correct JSON-RPC request for storage/set with TTL", () => {
    const { storage, written } = createTestStorage();

    const expiresAt = "2026-02-07T00:00:00Z";
    const promise = storage.set("cache", [1, 2, 3], expiresAt);

    const request = JSON.parse(written[0].trim());
    expect(request.method).toBe("storage/set");
    expect(request.params).toEqual({
      key: "cache",
      data: [1, 2, 3],
      expiresAt: "2026-02-07T00:00:00Z",
    });

    storage.handleResponse(successResponse(1, { success: true }));
    return promise;
  });

  it("should not include expiresAt when not provided", () => {
    const { storage, written } = createTestStorage();

    const promise = storage.set("key", "value");

    const request = JSON.parse(written[0].trim());
    expect(request.params).toEqual({ key: "key", data: "value" });
    expect("expiresAt" in request.params).toBe(false);

    storage.handleResponse(successResponse(1, { success: true }));
    return promise;
  });

  it("should build correct JSON-RPC request for storage/delete", () => {
    const { storage, written } = createTestStorage();

    const promise = storage.delete("old_cache");

    const request = JSON.parse(written[0].trim());
    expect(request.method).toBe("storage/delete");
    expect(request.params).toEqual({ key: "old_cache" });

    storage.handleResponse(successResponse(1, { deleted: true }));
    return promise;
  });

  it("should build correct JSON-RPC request for storage/list", () => {
    const { storage, written } = createTestStorage();

    const promise = storage.list();

    const request = JSON.parse(written[0].trim());
    expect(request.method).toBe("storage/list");
    expect(request.params).toEqual({});

    storage.handleResponse(successResponse(1, { keys: [] }));
    return promise;
  });

  it("should build correct JSON-RPC request for storage/clear", () => {
    const { storage, written } = createTestStorage();

    const promise = storage.clear();

    const request = JSON.parse(written[0].trim());
    expect(request.method).toBe("storage/clear");
    expect(request.params).toEqual({});

    storage.handleResponse(successResponse(1, { deletedCount: 0 }));
    return promise;
  });

  it("should increment request IDs", () => {
    const { storage, written } = createTestStorage();

    const p1 = storage.get("key1");
    const p2 = storage.get("key2");

    const req1 = JSON.parse(written[0].trim());
    const req2 = JSON.parse(written[1].trim());

    expect(req1.id).toBe(1);
    expect(req2.id).toBe(2);

    storage.handleResponse(successResponse(1, { data: null }));
    storage.handleResponse(successResponse(2, { data: null }));
    return Promise.all([p1, p2]);
  });
});

// =============================================================================
// PluginStorage Response Handling Tests
// =============================================================================

describe("PluginStorage - Response Handling", () => {
  it("should resolve get request with data", async () => {
    const { storage } = createTestStorage();

    const promise = storage.get("taste_profile");
    storage.handleResponse(
      successResponse(1, { data: { genres: ["action"] }, expiresAt: "2026-12-31T23:59:59Z" }),
    );

    const result = await promise;
    expect(result.data).toEqual({ genres: ["action"] });
    expect(result.expiresAt).toBe("2026-12-31T23:59:59Z");
  });

  it("should resolve get request with null data", async () => {
    const { storage } = createTestStorage();

    const promise = storage.get("nonexistent");
    storage.handleResponse(successResponse(1, { data: null }));

    const result = await promise;
    expect(result.data).toBeNull();
  });

  it("should resolve set request", async () => {
    const { storage } = createTestStorage();

    const promise = storage.set("key", { value: 42 });
    storage.handleResponse(successResponse(1, { success: true }));

    const result = await promise;
    expect(result.success).toBe(true);
  });

  it("should resolve delete request", async () => {
    const { storage } = createTestStorage();

    const promise = storage.delete("key");
    storage.handleResponse(successResponse(1, { deleted: true }));

    const result = await promise;
    expect(result.deleted).toBe(true);
  });

  it("should resolve list request", async () => {
    const { storage } = createTestStorage();

    const promise = storage.list();
    storage.handleResponse(
      successResponse(1, {
        keys: [
          { key: "profile", updatedAt: "2026-02-06T10:00:00Z" },
          { key: "cache", expiresAt: "2026-02-07T00:00:00Z", updatedAt: "2026-02-06T11:00:00Z" },
        ],
      }),
    );

    const result = await promise;
    expect(result.keys).toHaveLength(2);
    expect(result.keys[0].key).toBe("profile");
    expect(result.keys[1].expiresAt).toBe("2026-02-07T00:00:00Z");
  });

  it("should resolve clear request", async () => {
    const { storage } = createTestStorage();

    const promise = storage.clear();
    storage.handleResponse(successResponse(1, { deletedCount: 3 }));

    const result = await promise;
    expect(result.deletedCount).toBe(3);
  });

  it("should reject on error response", async () => {
    const { storage } = createTestStorage();

    const promise = storage.get("key");
    storage.handleResponse(errorResponse(1, -32603, "Internal error"));

    await expect(promise).rejects.toThrow(StorageError);
    await expect(promise).rejects.toThrow("Internal error");
    try {
      await promise;
    } catch (e) {
      expect((e as StorageError).code).toBe(-32603);
    }
  });

  it("should handle out-of-order responses", async () => {
    const { storage } = createTestStorage();

    const p1 = storage.get("key1");
    const p2 = storage.get("key2");

    // Respond to second request first
    storage.handleResponse(successResponse(2, { data: "second" }));
    storage.handleResponse(successResponse(1, { data: "first" }));

    const r1 = await p1;
    const r2 = await p2;

    expect(r1.data).toBe("first");
    expect(r2.data).toBe("second");
  });

  it("should ignore non-JSON lines", () => {
    const { storage } = createTestStorage();
    // Should not throw
    storage.handleResponse("not json at all");
    storage.handleResponse("");
    storage.handleResponse("   ");
  });

  it("should ignore host-to-plugin requests (lines with method field)", () => {
    const { storage } = createTestStorage();
    const promise = storage.get("key");

    // This is a host-to-plugin request, not a response - should be ignored
    storage.handleResponse(
      JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {} }),
    );

    // Promise should still be pending (not resolved)
    storage.cancelAll();
    return expect(promise).rejects.toThrow("Storage client stopped");
  });

  it("should ignore responses with no matching pending request", () => {
    const { storage } = createTestStorage();
    // Should not throw even though there's no pending request with id=999
    storage.handleResponse(successResponse(999, { data: "orphan" }));
  });
});

// =============================================================================
// PluginStorage cancelAll Tests
// =============================================================================

describe("PluginStorage - cancelAll", () => {
  it("should reject all pending requests", async () => {
    const { storage } = createTestStorage();

    const p1 = storage.get("key1");
    const p2 = storage.set("key2", "value");
    const p3 = storage.delete("key3");

    storage.cancelAll();

    await expect(p1).rejects.toThrow("Storage client stopped");
    await expect(p2).rejects.toThrow("Storage client stopped");
    await expect(p3).rejects.toThrow("Storage client stopped");
  });

  it("should be safe to call with no pending requests", () => {
    const { storage } = createTestStorage();
    expect(() => storage.cancelAll()).not.toThrow();
  });
});

// =============================================================================
// PluginStorage Write Error Tests
// =============================================================================

describe("PluginStorage - Write Errors", () => {
  it("should reject with StorageError when write throws", async () => {
    const storage = new PluginStorage(() => {
      throw new Error("pipe broken");
    });

    const promise = storage.get("key");
    await expect(promise).rejects.toThrow(StorageError);
    await expect(promise).rejects.toThrow("pipe broken");
  });
});
