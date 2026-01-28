/**
 * Plugin error classes for structured error handling
 */

import { type JsonRpcError, PLUGIN_ERROR_CODES } from "./types/rpc.js";

/**
 * Base class for plugin errors that map to JSON-RPC errors
 */
export abstract class PluginError extends Error {
  abstract readonly code: number;
  readonly data?: unknown;

  constructor(message: string, data?: unknown) {
    super(message);
    this.name = this.constructor.name;
    this.data = data;
  }

  /**
   * Convert to JSON-RPC error format
   */
  toJsonRpcError(): JsonRpcError {
    return {
      code: this.code,
      message: this.message,
      data: this.data,
    };
  }
}

/**
 * Thrown when rate limited by an external API
 */
export class RateLimitError extends PluginError {
  readonly code = PLUGIN_ERROR_CODES.RATE_LIMITED;
  /** Seconds to wait before retrying */
  readonly retryAfterSeconds: number;

  constructor(retryAfterSeconds: number, message?: string) {
    super(message ?? `Rate limited, retry after ${retryAfterSeconds}s`, {
      retryAfterSeconds,
    });
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

/**
 * Thrown when a requested resource is not found
 */
export class NotFoundError extends PluginError {
  readonly code = PLUGIN_ERROR_CODES.NOT_FOUND;
}

/**
 * Thrown when authentication fails (invalid credentials)
 */
export class AuthError extends PluginError {
  readonly code = PLUGIN_ERROR_CODES.AUTH_FAILED;

  constructor(message?: string) {
    super(message ?? "Authentication failed");
  }
}

/**
 * Thrown when an external API returns an error
 */
export class ApiError extends PluginError {
  readonly code = PLUGIN_ERROR_CODES.API_ERROR;
  readonly statusCode: number | undefined;

  constructor(message: string, statusCode?: number) {
    super(message, statusCode !== undefined ? { statusCode } : undefined);
    this.statusCode = statusCode;
  }
}

/**
 * Thrown when the plugin is misconfigured
 */
export class ConfigError extends PluginError {
  readonly code = PLUGIN_ERROR_CODES.CONFIG_ERROR;
}
