/**
 * Payload Recorder - debug helper for echo plugins.
 *
 * Writes every request and its matching response to paired JSON files under the
 * plugin's host-provided file-storage directory (`InitializeParams.dataDir`), so
 * the host -> plugin protocol traffic can be inspected without trawling the
 * server logs. This is intended for the echo (test/debug) plugins only.
 *
 * Files for one call share a sortable basename and differ only by suffix:
 *
 *   {yyyy-MM-dd-HH-mm-ss}-{id}-{method}-request.json
 *   {yyyy-MM-dd-HH-mm-ss}-{id}-{method}-response.json
 *
 * Timestamps are UTC + 24-hour + zero-padded, so lexical sort == chronological
 * sort regardless of the server's timezone. `{id}` is a zero-padded monotonic
 * per-process counter that breaks ties within the same second and pairs the two
 * files. Each file is a JSON envelope holding the payload plus a snapshot of the
 * active config (credentials redacted).
 *
 * All filesystem I/O is best-effort: a failure is logged and swallowed so a disk
 * problem never breaks an RPC response.
 *
 * NOTE: this module is intentionally duplicated across the echo plugins to keep
 * the published SDK surface small. Keep the copies in sync.
 */

import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** Minimal logger contract (compatible with the SDK `Logger`). */
export interface RecorderLogger {
  info(message: string): void;
  warn(message: string): void;
  debug(message: string): void;
}

export interface PayloadRecorderOptions {
  /** Plugin name, used for the fallback directory and the file envelope. */
  pluginName: string;
  /** Host-provided file-storage directory (`InitializeParams.dataDir`). */
  dataDir?: string;
  /** Whether recording is on (default: true). */
  enabled?: boolean;
  /** Maximum number of files to keep; oldest are pruned (default: 500). */
  maxFiles?: number;
  /** Active config snapshot to embed in each file (should be pre-redacted). */
  configSnapshot: unknown;
  /** Logger for diagnostics. */
  logger: RecorderLogger;
  /** Clock injection for tests (default: `() => new Date()`). */
  now?: () => Date;
}

/** Keys whose values are dropped from on-disk config snapshots. */
const SECRET_KEY_RE = /token|secret|password|api[-_]?key|credential/i;
const REDACTED = "[REDACTED]";

const DEFAULT_MAX_FILES = 500;

function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

/** Format a date as `yyyy-MM-dd-HH-mm-ss` in UTC. */
function utcStamp(date: Date): string {
  const y = date.getUTCFullYear();
  const mo = pad(date.getUTCMonth() + 1, 2);
  const d = pad(date.getUTCDate(), 2);
  const h = pad(date.getUTCHours(), 2);
  const mi = pad(date.getUTCMinutes(), 2);
  const s = pad(date.getUTCSeconds(), 2);
  return `${y}-${mo}-${d}-${h}-${mi}-${s}`;
}

/** Replace non-alphanumeric runs with `_` so a method is filename-safe. */
function sanitizeMethod(method: string): string {
  return method.replace(/[^a-z0-9]+/gi, "_").replace(/^_+|_+$/g, "");
}

/**
 * Recursively copy a config object, replacing values under secret-like keys
 * with `[REDACTED]`. Arrays and primitives are returned as-is.
 */
function redactObject(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(redactObject);
  }
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
      out[key] = SECRET_KEY_RE.test(key) ? REDACTED : redactObject(val);
    }
    return out;
  }
  return value;
}

/**
 * Build a redacted config snapshot from initialize params. Credentials are
 * never included (they are passed separately, not via config); secret-like
 * config keys are additionally redacted as a defensive measure.
 */
export function redactConfig(input: {
  adminConfig?: Record<string, unknown>;
  userConfig?: Record<string, unknown>;
}): { adminConfig: unknown; userConfig: unknown } {
  return {
    adminConfig: redactObject(input.adminConfig ?? {}),
    userConfig: redactObject(input.userConfig ?? {}),
  };
}

interface Envelope {
  timestamp: string;
  plugin: string;
  method: string;
  direction: "request" | "response";
  id: number;
  config: unknown;
  payload: unknown;
}

export class PayloadRecorder {
  private readonly pluginName: string;
  private readonly enabled: boolean;
  private readonly maxFiles: number;
  private readonly configSnapshot: unknown;
  private readonly logger: RecorderLogger;
  private readonly now: () => Date;
  private readonly dir: string;
  private readonly usingFallback: boolean;
  private seq = 0;
  private ready: Promise<boolean> | null = null;

  constructor(opts: PayloadRecorderOptions) {
    this.pluginName = opts.pluginName;
    this.enabled = opts.enabled ?? true;
    this.maxFiles = opts.maxFiles ?? DEFAULT_MAX_FILES;
    this.configSnapshot = opts.configSnapshot;
    this.logger = opts.logger;
    this.now = opts.now ?? (() => new Date());

    if (opts.dataDir) {
      this.dir = join(opts.dataDir, "payloads");
      this.usingFallback = false;
    } else {
      this.dir = join(tmpdir(), `codex-${opts.pluginName}`, "payloads");
      this.usingFallback = true;
    }
  }

  /** Absolute directory payloads are written to (for logging/tests). */
  get directory(): string {
    return this.dir;
  }

  /** Lazily create the payloads directory once; returns false if unusable. */
  private ensureDir(): Promise<boolean> {
    if (!this.ready) {
      this.ready = mkdir(this.dir, { recursive: true })
        .then(() => {
          if (this.usingFallback) {
            this.logger.warn(`No dataDir provided; recording payloads under temp dir ${this.dir}`);
          } else {
            this.logger.debug(`Recording payloads to ${this.dir}`);
          }
          return true;
        })
        .catch((err: unknown) => {
          const msg = err instanceof Error ? err.message : "unknown error";
          this.logger.warn(`Failed to create payload dir ${this.dir}: ${msg}`);
          return false;
        });
    }
    return this.ready;
  }

  /**
   * Record a request and its response under one shared, sortable basename.
   * Best-effort: never throws.
   */
  async record(method: string, request: unknown, response: unknown): Promise<void> {
    if (!this.enabled) return;
    if (!(await this.ensureDir())) return;

    const id = ++this.seq;
    const date = this.now();
    const base = `${utcStamp(date)}-${pad(id, 4)}-${sanitizeMethod(method)}`;
    const iso = date.toISOString();

    await this.writeFile(`${base}-request.json`, {
      timestamp: iso,
      plugin: this.pluginName,
      method,
      direction: "request",
      id,
      config: this.configSnapshot,
      payload: request,
    });
    await this.writeFile(`${base}-response.json`, {
      timestamp: iso,
      plugin: this.pluginName,
      method,
      direction: "response",
      id,
      config: this.configSnapshot,
      payload: response,
    });

    await this.prune();
  }

  private async writeFile(name: string, envelope: Envelope): Promise<void> {
    try {
      await writeFile(join(this.dir, name), JSON.stringify(envelope, null, 2), "utf8");
    } catch (err) {
      const msg = err instanceof Error ? err.message : "unknown error";
      this.logger.warn(`Failed to write payload file ${name}: ${msg}`);
    }
  }

  /** Keep at most `maxFiles` files, deleting the oldest (lexical == chrono). */
  private async prune(): Promise<void> {
    if (this.maxFiles <= 0) return;
    try {
      const files = (await readdir(this.dir)).filter((f) => f.endsWith(".json")).sort();
      const excess = files.length - this.maxFiles;
      if (excess <= 0) return;
      for (const file of files.slice(0, excess)) {
        await rm(join(this.dir, file), { force: true });
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "unknown error";
      this.logger.warn(`Failed to prune payload dir ${this.dir}: ${msg}`);
    }
  }
}
