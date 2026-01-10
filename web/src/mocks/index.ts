/**
 * Mocks module entry point
 *
 * Re-exports all mock-related utilities.
 */

export { handlers } from "./handlers";
export { worker, startMockServiceWorker } from "./browser";
export * from "./data/factories";
