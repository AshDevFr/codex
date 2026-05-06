/**
 * Generic host reverse-RPC client.
 *
 * Plugins use this to call host methods outside the storage namespace —
 * notably `releases/list_tracked`, `releases/record`,
 * `releases/source_state/get`, and `releases/source_state/set`. The class is
 * intentionally generic so future reverse-RPC namespaces can reuse it
 * without a per-namespace client.
 *
 * Wire-format and lifecycle mirror `PluginStorage`: send a JSON-RPC request
 * over stdout with a unique id, and resolve when the host's response with
 * the matching id arrives on stdin. The plugin server's main loop calls
 * `handleResponse(line)` on every incoming response; whichever client owns
 * the id resolves it (others no-op silently).
 *
 * The id counter starts at a high value (`1_000_000_000`) so it can never
 * collide with `PluginStorage`'s sequence (`1, 2, 3, ...`). This means the
 * dispatch in the server doesn't need to know which client a response
 * belongs to — it can fan out to both, and at most one will match.
 */

import { currentParentRequestId } from "./request-context.js";
import type { JsonRpcError, JsonRpcRequest } from "./types/rpc.js";

/** Write function signature for sending JSON-RPC requests. */
type WriteFn = (line: string) => void;

/**
 * Error thrown when a reverse-RPC call fails (host returned a JSON-RPC error,
 * or the client was canceled).
 */
export class HostRpcError extends Error {
  constructor(
    message: string,
    public readonly code: number,
    public readonly data?: unknown,
  ) {
    super(message);
    this.name = "HostRpcError";
  }
}

/**
 * Generic reverse-RPC client. Construct one per plugin instance and pass it
 * around via `InitializeParams`.
 */
export class HostRpcClient {
  // Start the counter high so it can't collide with PluginStorage's id space.
  // `Number.MAX_SAFE_INTEGER` is far above this, so we have plenty of room
  // before wrapping (and we never expect a single plugin lifetime to issue
  // more than ~9 quintillion calls).
  private nextId = 1_000_000_000;
  private pendingRequests = new Map<
    number,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private writeFn: WriteFn;

  /**
   * @param writeFn - Optional custom write function (defaults to
   *   `process.stdout.write`). Useful for testing.
   */
  constructor(writeFn?: WriteFn) {
    this.writeFn =
      writeFn ??
      ((line: string) => {
        process.stdout.write(line);
      });
  }

  /**
   * Send a JSON-RPC request to the host and resolve with the result.
   *
   * @param method - JSON-RPC method name (e.g. `"releases/list_tracked"`).
   * @param params - Method-specific params. Pass `undefined` when the method
   *   takes no params.
   */
  async call<T = unknown>(method: string, params?: unknown): Promise<T> {
    const id = this.nextId++;
    // Stamp the forward call we're inside so the host can route this
    // reverse-RPC back to the originating caller's task. Lifted from the
    // `request-context` async-local storage that `server.ts` sets around
    // every forward-request handler.
    const parent = currentParentRequestId();
    const request: JsonRpcRequest = {
      jsonrpc: "2.0",
      id,
      method,
      params,
      ...(parent !== undefined ? { parentRequestId: parent } : {}),
    };

    return new Promise<T>((resolve, reject) => {
      this.pendingRequests.set(id, {
        resolve: (v) => resolve(v as T),
        reject,
      });
      try {
        this.writeFn(`${JSON.stringify(request)}\n`);
      } catch (err) {
        this.pendingRequests.delete(id);
        const message = err instanceof Error ? err.message : "Unknown write error";
        reject(new HostRpcError(`Failed to send request: ${message}`, -1));
      }
    });
  }

  /**
   * Process an incoming JSON-RPC response line. Returns `true` if this
   * client owned the response id and resolved it, `false` otherwise (so
   * other clients can try).
   *
   * Called by the plugin server's main loop on every response.
   */
  handleResponse(line: string): boolean {
    const trimmed = line.trim();
    if (!trimmed) return false;

    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      return false;
    }

    const obj = parsed as Record<string, unknown>;
    if (obj.method !== undefined) return false; // not a response
    const rawId = obj.id;
    if (typeof rawId !== "number") return false;
    if (!this.pendingRequests.has(rawId)) return false;

    const pending = this.pendingRequests.get(rawId);
    if (!pending) return false;
    this.pendingRequests.delete(rawId);

    if ("error" in obj && obj.error) {
      const err = obj.error as JsonRpcError;
      pending.reject(new HostRpcError(err.message, err.code, err.data));
    } else {
      pending.resolve(obj.result);
    }
    return true;
  }

  /** Reject all pending requests (e.g. on shutdown). */
  cancelAll(): void {
    for (const [, pending] of this.pendingRequests) {
      pending.reject(new HostRpcError("Host RPC client stopped", -1));
    }
    this.pendingRequests.clear();
  }
}
