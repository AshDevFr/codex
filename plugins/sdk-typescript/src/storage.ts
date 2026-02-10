/**
 * Plugin Storage - key-value storage for per-user plugin data
 *
 * Storage is scoped per user-plugin instance. Plugins only specify a key;
 * the host resolves the user context from the connection.
 *
 * Plugins send storage requests as JSON-RPC calls to the host over stdout
 * and receive responses on stdin. This is the reverse of the normal
 * host-to-plugin request flow.
 *
 * @example
 * ```typescript
 * import { PluginStorage } from "@ashdev/codex-plugin-sdk";
 *
 * const storage = new PluginStorage();
 *
 * // Store data
 * await storage.set("taste_profile", { genres: ["action", "drama"] });
 *
 * // Retrieve data
 * const data = await storage.get("taste_profile");
 *
 * // Store with TTL (expires in 24 hours)
 * const expires = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString();
 * await storage.set("cache", { items: [1, 2, 3] }, expires);
 *
 * // List all keys
 * const keys = await storage.list();
 *
 * // Delete a key
 * await storage.delete("cache");
 *
 * // Clear all data
 * await storage.clear();
 * ```
 */

import type { JsonRpcError, JsonRpcRequest } from "./types/rpc.js";

// =============================================================================
// Storage Types
// =============================================================================

/** Response from storage/get */
export interface StorageGetResponse {
  /** The stored data, or null if key doesn't exist */
  data: unknown | null;
  /** Expiration timestamp (ISO 8601) if TTL was set */
  expiresAt?: string;
}

/** Response from storage/set */
export interface StorageSetResponse {
  /** Always true on success */
  success: boolean;
}

/** Response from storage/delete */
export interface StorageDeleteResponse {
  /** Whether the key existed and was deleted */
  deleted: boolean;
}

/** Individual key entry from storage/list */
export interface StorageKeyEntry {
  /** Storage key name */
  key: string;
  /** Expiration timestamp (ISO 8601) if TTL was set */
  expiresAt?: string;
  /** Last update timestamp (ISO 8601) */
  updatedAt: string;
}

/** Response from storage/list */
export interface StorageListResponse {
  /** All keys for this plugin instance (excluding expired) */
  keys: StorageKeyEntry[];
}

/** Response from storage/clear */
export interface StorageClearResponse {
  /** Number of entries deleted */
  deletedCount: number;
}

// =============================================================================
// Storage Error
// =============================================================================

/** Error from a storage operation */
export class StorageError extends Error {
  constructor(
    message: string,
    public readonly code: number,
    public readonly data?: unknown,
  ) {
    super(message);
    this.name = "StorageError";
  }
}

// =============================================================================
// Plugin Storage Client
// =============================================================================

/** Write function signature for sending JSON-RPC requests */
type WriteFn = (line: string) => void;

/**
 * Client for plugin key-value storage.
 *
 * Sends JSON-RPC requests to the host process over stdout and reads
 * responses on stdin. Each request gets a unique ID so responses can
 * be correlated even if they arrive out of order.
 */
export class PluginStorage {
  private nextId = 1;
  private pendingRequests = new Map<
    string | number,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private writeFn: WriteFn;

  /**
   * Create a new storage client.
   *
   * @param writeFn - Optional custom write function (defaults to process.stdout.write).
   *                  Useful for testing or custom transport layers.
   */
  constructor(writeFn?: WriteFn) {
    this.writeFn =
      writeFn ??
      ((line: string) => {
        process.stdout.write(line);
      });
  }

  /**
   * Get a value by key
   *
   * @param key - Storage key to retrieve
   * @returns The stored data and optional expiration, or null data if key doesn't exist
   */
  async get(key: string): Promise<StorageGetResponse> {
    return (await this.sendRequest("storage/get", { key })) as StorageGetResponse;
  }

  /**
   * Set a value by key (upsert - creates or updates)
   *
   * @param key - Storage key
   * @param data - JSON-serializable data to store
   * @param expiresAt - Optional expiration timestamp (ISO 8601)
   * @returns Success indicator
   */
  async set(key: string, data: unknown, expiresAt?: string): Promise<StorageSetResponse> {
    const params: Record<string, unknown> = { key, data };
    if (expiresAt !== undefined) {
      params.expiresAt = expiresAt;
    }
    return (await this.sendRequest("storage/set", params)) as StorageSetResponse;
  }

  /**
   * Delete a value by key
   *
   * @param key - Storage key to delete
   * @returns Whether the key existed and was deleted
   */
  async delete(key: string): Promise<StorageDeleteResponse> {
    return (await this.sendRequest("storage/delete", { key })) as StorageDeleteResponse;
  }

  /**
   * List all keys for this plugin instance (excluding expired)
   *
   * @returns List of key entries with metadata
   */
  async list(): Promise<StorageListResponse> {
    return (await this.sendRequest("storage/list", {})) as StorageListResponse;
  }

  /**
   * Clear all data for this plugin instance
   *
   * @returns Number of entries deleted
   */
  async clear(): Promise<StorageClearResponse> {
    return (await this.sendRequest("storage/clear", {})) as StorageClearResponse;
  }

  /**
   * Handle an incoming JSON-RPC response line from the host.
   *
   * Call this method from your readline handler to deliver responses
   * back to pending storage requests.
   */
  handleResponse(line: string): void {
    const trimmed = line.trim();
    if (!trimmed) return;

    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      // Not JSON - ignore
      return;
    }

    const obj = parsed as Record<string, unknown>;

    // Only handle responses (have "result" or "error", no "method")
    if (obj.method !== undefined) {
      // This is a host-to-plugin request, not a storage response - ignore
      return;
    }

    const id = obj.id;
    if (id === undefined || id === null) return;

    const pending = this.pendingRequests.get(id as string | number);
    if (!pending) return;

    this.pendingRequests.delete(id as string | number);

    if ("error" in obj && obj.error) {
      const err = obj.error as JsonRpcError;
      pending.reject(new StorageError(err.message, err.code, err.data));
    } else {
      pending.resolve(obj.result);
    }
  }

  /**
   * Cancel all pending requests (e.g. on shutdown).
   */
  cancelAll(): void {
    for (const [, pending] of this.pendingRequests) {
      pending.reject(new StorageError("Storage client stopped", -1));
    }
    this.pendingRequests.clear();
  }

  // ===========================================================================
  // Internal
  // ===========================================================================

  private sendRequest(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;

    const request: JsonRpcRequest = {
      jsonrpc: "2.0",
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });

      try {
        this.writeFn(`${JSON.stringify(request)}\n`);
      } catch (err) {
        this.pendingRequests.delete(id);
        const message = err instanceof Error ? err.message : "Unknown write error";
        reject(new StorageError(`Failed to send request: ${message}`, -1));
      }
    });
  }
}
