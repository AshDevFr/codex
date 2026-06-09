/**
 * Shared plugin logger.
 *
 * A single instance imported by every module so the host-supplied log level
 * (applied via `logger.setLevel(...)` in `onInitialize`, sourced from the Codex
 * `plugins.log_level` config) governs debug output across the whole plugin,
 * not just `index.ts`.
 */

import { createLogger } from "@ashdev/codex-plugin-sdk";

export const logger = createLogger({ name: "recommendations-anilist", level: "info" });
