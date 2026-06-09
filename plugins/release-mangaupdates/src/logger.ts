/**
 * Shared plugin logger.
 *
 * A single instance imported by every module so the host-supplied log level
 * (applied via `logger.setLevel(...)` in `onInitialize`, sourced from the Codex
 * `plugins.log_level` config) governs debug output across the whole plugin,
 * not just `index.ts`.
 */

import { createLogger } from "@ashdev/codex-plugin-sdk";
import { manifest } from "./manifest.js";

export const logger = createLogger({ name: manifest.name, level: "info" });
