import { HostRpcClient } from "@ashdev/codex-plugin-sdk";
import { describe, expect, it } from "vitest";
import { normalizeBaseUrl, registerSources } from "./index.js";

// -----------------------------------------------------------------------------
// Mock host RPC
// -----------------------------------------------------------------------------

interface CapturedCall {
  method: string;
  params: unknown;
}

type Responder = (
  method: string,
  params: unknown,
  attempt: number,
) => unknown | { __error: { code: number; message: string } };

/**
 * Build a `HostRpcClient` whose calls are intercepted in-memory. The custom
 * `writeFn` captures each request and synthesizes a JSON-RPC response (result
 * or error) via the real id-correlation path. `respond` may return a normal
 * result, or `{ __error: { code, message } }` to drive an error response with
 * a specific code (e.g. -32601 for METHOD_NOT_FOUND).
 */
function makeMockRpc(respond: Responder): {
  rpc: HostRpcClient;
  calls: CapturedCall[];
} {
  const calls: CapturedCall[] = [];
  let attemptByMethod: Record<string, number> = {};
  // `rpc` is referenced inside writeFn (a closure) before its assignment runs,
  // so it must be declared with `let` and initialized after writeFn is built.
  let rpc: HostRpcClient;
  const writeFn = (line: string) => {
    const req = JSON.parse(line.trim()) as {
      id: number;
      method: string;
      params: unknown;
    };
    calls.push({ method: req.method, params: req.params });
    attemptByMethod[req.method] = (attemptByMethod[req.method] ?? 0) + 1;
    const outcome = respond(req.method, req.params, attemptByMethod[req.method]);
    setImmediate(() => {
      const isError =
        outcome !== null &&
        typeof outcome === "object" &&
        "__error" in (outcome as Record<string, unknown>);
      const payload = isError
        ? {
            jsonrpc: "2.0",
            id: req.id,
            error: (outcome as { __error: { code: number; message: string } }).__error,
          }
        : { jsonrpc: "2.0", id: req.id, result: outcome };
      rpc.handleResponse(JSON.stringify(payload));
    });
  };
  rpc = new HostRpcClient(writeFn);
  attemptByMethod = {};
  return { rpc, calls };
}

// -----------------------------------------------------------------------------
// normalizeBaseUrl
// -----------------------------------------------------------------------------

describe("normalizeBaseUrl", () => {
  it("strips trailing slashes and trims whitespace", () => {
    expect(normalizeBaseUrl("https://t.example.com/")).toBe("https://t.example.com");
    expect(normalizeBaseUrl("  https://t.example.com///  ")).toBe("https://t.example.com");
    expect(normalizeBaseUrl("https://t.example.com")).toBe("https://t.example.com");
  });
});

// -----------------------------------------------------------------------------
// registerSources
// -----------------------------------------------------------------------------

describe("registerSources", () => {
  it("registers exactly one api-feed source keyed 'default'", async () => {
    const { rpc, calls } = makeMockRpc(() => ({ registered: 1, pruned: 0 }));
    const result = await registerSources(rpc);

    expect(result).toEqual({ registered: 1, pruned: 0 });
    expect(calls).toHaveLength(1);
    expect(calls[0].method).toBe("releases/register_sources");
    const params = calls[0].params as { sources: Array<Record<string, unknown>> };
    expect(params.sources).toHaveLength(1);
    expect(params.sources[0]).toMatchObject({
      sourceKey: "default",
      displayName: "Tsundoku Releases",
      kind: "api-feed",
    });
  });

  it("retries on METHOD_NOT_FOUND then succeeds", async () => {
    const { rpc, calls } = makeMockRpc((_m, _p, attempt) =>
      attempt < 3
        ? { __error: { code: -32601, message: "method not found" } }
        : { registered: 1, pruned: 0 },
    );
    const result = await registerSources(rpc);

    expect(result).toEqual({ registered: 1, pruned: 0 });
    expect(calls.length).toBe(3);
  });

  it("returns null after exhausting retries on METHOD_NOT_FOUND", async () => {
    const { rpc, calls } = makeMockRpc(() => ({
      __error: { code: -32601, message: "method not found" },
    }));
    const result = await registerSources(rpc);

    expect(result).toBeNull();
    expect(calls.length).toBe(5);
  });

  it("does not retry on a non-METHOD_NOT_FOUND error", async () => {
    const { rpc, calls } = makeMockRpc(() => ({
      __error: { code: -32000, message: "db error" },
    }));
    const result = await registerSources(rpc);

    expect(result).toBeNull();
    expect(calls.length).toBe(1);
  });
});
