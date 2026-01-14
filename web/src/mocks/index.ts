/**
 * Mocks module entry point
 *
 * Re-exports all mock-related utilities.
 */

export { startMockServiceWorker, worker } from "./browser";
export * from "./data/factories";
export { handlers } from "./handlers";
