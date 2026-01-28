/**
 * Logging utilities for plugins
 *
 * IMPORTANT: Plugins must ONLY write to stderr for logging.
 * stdout is reserved for JSON-RPC communication.
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

const LOG_LEVELS: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

export interface LoggerOptions {
  /** Plugin name to prefix log messages */
  name: string;
  /** Minimum log level (default: "info") */
  level?: LogLevel;
  /** Whether to include timestamps (default: true) */
  timestamps?: boolean;
}

/**
 * Logger that writes to stderr (safe for plugins)
 */
export class Logger {
  private readonly name: string;
  private readonly minLevel: number;
  private readonly timestamps: boolean;

  constructor(options: LoggerOptions) {
    this.name = options.name;
    this.minLevel = LOG_LEVELS[options.level ?? "info"];
    this.timestamps = options.timestamps ?? true;
  }

  private shouldLog(level: LogLevel): boolean {
    return LOG_LEVELS[level] >= this.minLevel;
  }

  private format(level: LogLevel, message: string, data?: unknown): string {
    const parts: string[] = [];

    if (this.timestamps) {
      parts.push(new Date().toISOString());
    }

    parts.push(`[${level.toUpperCase()}]`);
    parts.push(`[${this.name}]`);
    parts.push(message);

    if (data !== undefined) {
      if (data instanceof Error) {
        parts.push(`- ${data.message}`);
        if (data.stack) {
          parts.push(`\n${data.stack}`);
        }
      } else if (typeof data === "object") {
        parts.push(`- ${JSON.stringify(data)}`);
      } else {
        parts.push(`- ${String(data)}`);
      }
    }

    return parts.join(" ");
  }

  private log(level: LogLevel, message: string, data?: unknown): void {
    if (this.shouldLog(level)) {
      // Write to stderr (not stdout!) - stdout is for JSON-RPC only
      process.stderr.write(`${this.format(level, message, data)}\n`);
    }
  }

  debug(message: string, data?: unknown): void {
    this.log("debug", message, data);
  }

  info(message: string, data?: unknown): void {
    this.log("info", message, data);
  }

  warn(message: string, data?: unknown): void {
    this.log("warn", message, data);
  }

  error(message: string, data?: unknown): void {
    this.log("error", message, data);
  }
}

/**
 * Create a logger for a plugin
 */
export function createLogger(options: LoggerOptions): Logger {
  return new Logger(options);
}
