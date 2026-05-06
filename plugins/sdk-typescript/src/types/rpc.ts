/**
 * JSON-RPC 2.0 types for plugin communication
 */

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id: string | number | null;
  method: string;
  params?: unknown;
  /**
   * Reverse-RPC only: id of the forward call this plugin is currently
   * servicing. Tells the host to route the reverse-RPC back to the
   * originating caller's task so emitted events land in that caller's
   * recording broadcaster (and replay correctly in distributed
   * deployments). The SDK stamps this automatically via
   * `AsyncLocalStorage` — plugin authors don't set it.
   */
  parentRequestId?: string | number | null;
}

export interface JsonRpcSuccessResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  result: unknown;
}

export interface JsonRpcErrorResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  error: JsonRpcError;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export type JsonRpcResponse = JsonRpcSuccessResponse | JsonRpcErrorResponse;

/**
 * Standard JSON-RPC error codes
 */
export const JSON_RPC_ERROR_CODES = {
  /** Invalid JSON was received */
  PARSE_ERROR: -32700,
  /** The JSON sent is not a valid Request object */
  INVALID_REQUEST: -32600,
  /** The method does not exist / is not available */
  METHOD_NOT_FOUND: -32601,
  /** Invalid method parameter(s) */
  INVALID_PARAMS: -32602,
  /** Internal JSON-RPC error */
  INTERNAL_ERROR: -32603,
} as const;

/**
 * Plugin-specific error codes (in the -32000 to -32099 range)
 */
export const PLUGIN_ERROR_CODES = {
  /** Rate limited by external API */
  RATE_LIMITED: -32001,
  /** Resource not found (e.g., series ID doesn't exist) */
  NOT_FOUND: -32002,
  /** Authentication failed (invalid credentials) */
  AUTH_FAILED: -32003,
  /** External API error */
  API_ERROR: -32004,
  /** Plugin configuration error */
  CONFIG_ERROR: -32005,
} as const;
