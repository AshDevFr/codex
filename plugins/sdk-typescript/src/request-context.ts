/**
 * Async-local context for the currently-handled forward request.
 *
 * When the SDK dispatches a forward call (e.g. `releases/poll`), it stores
 * the call's `id` in this context for the duration of the handler. Any
 * reverse-RPC the plugin makes while servicing that call (e.g.
 * `releases/record` via `HostRpcClient.call`) reads the id and stamps it as
 * `parentRequestId` on the outgoing request.
 *
 * The host uses `parentRequestId` to route the reverse-RPC back to the
 * originating caller's tokio task, so emitted events land in the recording
 * broadcaster scoped to that task and replay correctly in distributed
 * deployments. Without this stamping, plugins that emit events via
 * reverse-RPC would silently lose them on the worker.
 *
 * Plugin authors don't interact with this directly. The SDK's request
 * dispatch (`server.ts`) sets it; `HostRpcClient.call` reads it.
 */

import { AsyncLocalStorage } from "node:async_hooks";

const store = new AsyncLocalStorage<string | number | null>();

/**
 * Run `fn` with `forwardRequestId` as the current parent. Calls to
 * `currentParentRequestId()` made inside `fn` (or anything it awaits) will
 * see this value.
 */
export function runWithParentRequestId<T>(
  forwardRequestId: string | number | null,
  fn: () => Promise<T>,
): Promise<T> {
  return store.run(forwardRequestId, fn);
}

/**
 * Snapshot the current forward request id, or `undefined` if no forward
 * request is on the call stack (e.g. background timers in the plugin that
 * fire reverse-RPCs outside a forward-call context — those won't be replay-
 * eligible, by design, since they don't belong to any task).
 */
export function currentParentRequestId(): string | number | null | undefined {
  return store.getStore();
}
