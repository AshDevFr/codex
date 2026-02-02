/**
 * Development-only logging utility.
 *
 * Creates a prefixed console.debug logger that only runs in development mode.
 * In production, the function is a no-op for zero runtime overhead.
 *
 * @example
 * ```ts
 * const log = createDevLog("[SSE]");
 * log("Connection state:", state);  // Outputs: [SSE] Connection state: connected
 * log("Event received:", event);    // Outputs: [SSE] Event received: {...}
 * ```
 */
export function createDevLog(prefix: string) {
  if (import.meta.env.DEV) {
    return (message: string, ...args: unknown[]) =>
      console.debug(`${prefix} ${message}`, ...args);
  }
  return () => {};
}
