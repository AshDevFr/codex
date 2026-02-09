/**
 * Tests for server.ts - JSON-RPC server implementation
 *
 * These tests cover:
 * - Parameter validation for search, get, and match methods
 * - Error handling for invalid requests
 * - Request handling flow
 * - JSON-RPC response detection (isJsonRpcResponse)
 * - Storage response routing in handleLine
 */

import { describe, expect, it } from "vitest";
import { JSON_RPC_ERROR_CODES } from "./types/rpc.js";

// =============================================================================
// Test Helpers - Re-implement validation functions for testing
// (These mirror the internal functions in server.ts)
// =============================================================================

interface ValidationError {
  field: string;
  message: string;
}

function validateStringFields(params: unknown, fields: string[]): ValidationError | null {
  if (params === null || params === undefined) {
    return { field: "params", message: "params is required" };
  }
  if (typeof params !== "object") {
    return { field: "params", message: "params must be an object" };
  }

  const obj = params as Record<string, unknown>;
  for (const field of fields) {
    const value = obj[field];
    if (value === undefined || value === null) {
      return { field, message: `${field} is required` };
    }
    if (typeof value !== "string") {
      return { field, message: `${field} must be a string` };
    }
    if (value.trim() === "") {
      return { field, message: `${field} cannot be empty` };
    }
  }

  return null;
}

function validateSearchParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["query"]);
}

function validateGetParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["externalId"]);
}

function validateMatchParams(params: unknown): ValidationError | null {
  return validateStringFields(params, ["title"]);
}

// =============================================================================
// Tests
// =============================================================================

describe("Parameter Validation", () => {
  describe("validateStringFields", () => {
    it("should return error when params is null", () => {
      const result = validateStringFields(null, ["query"]);
      expect(result).toEqual({ field: "params", message: "params is required" });
    });

    it("should return error when params is undefined", () => {
      const result = validateStringFields(undefined, ["query"]);
      expect(result).toEqual({ field: "params", message: "params is required" });
    });

    it("should return error when params is not an object", () => {
      const result = validateStringFields("string", ["query"]);
      expect(result).toEqual({ field: "params", message: "params must be an object" });
    });

    it("should return error when params is a number", () => {
      const result = validateStringFields(123, ["query"]);
      expect(result).toEqual({ field: "params", message: "params must be an object" });
    });

    it("should return error when required field is missing", () => {
      const result = validateStringFields({ other: "value" }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query is required" });
    });

    it("should return error when required field is null", () => {
      const result = validateStringFields({ query: null }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query is required" });
    });

    it("should return error when field is not a string", () => {
      const result = validateStringFields({ query: 123 }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query must be a string" });
    });

    it("should return error when field is an object", () => {
      const result = validateStringFields({ query: {} }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query must be a string" });
    });

    it("should return error when field is an empty string", () => {
      const result = validateStringFields({ query: "" }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query cannot be empty" });
    });

    it("should return error when field is whitespace only", () => {
      const result = validateStringFields({ query: "   " }, ["query"]);
      expect(result).toEqual({ field: "query", message: "query cannot be empty" });
    });

    it("should return null when validation passes", () => {
      const result = validateStringFields({ query: "test" }, ["query"]);
      expect(result).toBeNull();
    });

    it("should validate multiple fields", () => {
      const result = validateStringFields({ a: "x", b: "y" }, ["a", "b"]);
      expect(result).toBeNull();
    });

    it("should return error for first missing field in multi-field validation", () => {
      const result = validateStringFields({ a: "x" }, ["a", "b"]);
      expect(result).toEqual({ field: "b", message: "b is required" });
    });

    it("should accept objects with extra fields", () => {
      const result = validateStringFields({ query: "test", extra: "ignored" }, ["query"]);
      expect(result).toBeNull();
    });
  });

  describe("validateSearchParams", () => {
    it("should require query field", () => {
      const result = validateSearchParams({});
      expect(result).toEqual({ field: "query", message: "query is required" });
    });

    it("should accept valid search params", () => {
      const result = validateSearchParams({ query: "one piece", limit: 10 });
      expect(result).toBeNull();
    });

    it("should reject empty query", () => {
      const result = validateSearchParams({ query: "" });
      expect(result).toEqual({ field: "query", message: "query cannot be empty" });
    });
  });

  describe("validateGetParams", () => {
    it("should require externalId field", () => {
      const result = validateGetParams({});
      expect(result).toEqual({ field: "externalId", message: "externalId is required" });
    });

    it("should accept valid get params", () => {
      const result = validateGetParams({ externalId: "12345" });
      expect(result).toBeNull();
    });

    it("should reject empty externalId", () => {
      const result = validateGetParams({ externalId: "" });
      expect(result).toEqual({ field: "externalId", message: "externalId cannot be empty" });
    });

    it("should reject non-string externalId", () => {
      const result = validateGetParams({ externalId: 12345 });
      expect(result).toEqual({ field: "externalId", message: "externalId must be a string" });
    });
  });

  describe("validateMatchParams", () => {
    it("should require title field", () => {
      const result = validateMatchParams({});
      expect(result).toEqual({ field: "title", message: "title is required" });
    });

    it("should accept valid match params", () => {
      const result = validateMatchParams({ title: "Naruto", year: 2002 });
      expect(result).toBeNull();
    });

    it("should reject empty title", () => {
      const result = validateMatchParams({ title: "" });
      expect(result).toEqual({ field: "title", message: "title cannot be empty" });
    });
  });
});

describe("JSON-RPC Error Codes", () => {
  it("should have correct INVALID_PARAMS code", () => {
    expect(JSON_RPC_ERROR_CODES.INVALID_PARAMS).toBe(-32602);
  });

  it("should have correct PARSE_ERROR code", () => {
    expect(JSON_RPC_ERROR_CODES.PARSE_ERROR).toBe(-32700);
  });

  it("should have correct INVALID_REQUEST code", () => {
    expect(JSON_RPC_ERROR_CODES.INVALID_REQUEST).toBe(-32600);
  });

  it("should have correct METHOD_NOT_FOUND code", () => {
    expect(JSON_RPC_ERROR_CODES.METHOD_NOT_FOUND).toBe(-32601);
  });

  it("should have correct INTERNAL_ERROR code", () => {
    expect(JSON_RPC_ERROR_CODES.INTERNAL_ERROR).toBe(-32603);
  });
});

// =============================================================================
// isJsonRpcResponse Tests
// (Mirrors the internal isJsonRpcResponse function in server.ts)
// =============================================================================

/**
 * Detect whether a parsed JSON object is a JSON-RPC response (not a request).
 * This mirrors the internal function in server.ts for testing.
 */
function isJsonRpcResponse(obj: Record<string, unknown>): boolean {
  if (obj.method !== undefined) return false;
  if (obj.id === undefined || obj.id === null) return false;
  return "result" in obj || "error" in obj;
}

describe("isJsonRpcResponse", () => {
  it("should detect a success response", () => {
    const obj = { jsonrpc: "2.0", id: 1, result: { data: "hello" } };
    expect(isJsonRpcResponse(obj)).toBe(true);
  });

  it("should detect an error response", () => {
    const obj = { jsonrpc: "2.0", id: 1, error: { code: -32603, message: "Internal error" } };
    expect(isJsonRpcResponse(obj)).toBe(true);
  });

  it("should detect a response with null result", () => {
    const obj = { jsonrpc: "2.0", id: 1, result: null };
    expect(isJsonRpcResponse(obj)).toBe(true);
  });

  it("should detect a response with string id", () => {
    const obj = { jsonrpc: "2.0", id: "abc-123", result: "pong" };
    expect(isJsonRpcResponse(obj)).toBe(true);
  });

  it("should detect a response with numeric id 0", () => {
    const obj = { jsonrpc: "2.0", id: 0, result: {} };
    expect(isJsonRpcResponse(obj)).toBe(true);
  });

  it("should reject a request (has method field)", () => {
    const obj = { jsonrpc: "2.0", id: 1, method: "initialize", params: {} };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject a notification (has method, no id)", () => {
    const obj = { jsonrpc: "2.0", method: "shutdown" };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject when id is null", () => {
    const obj = { jsonrpc: "2.0", id: null, result: {} };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject when id is undefined (missing)", () => {
    const obj = { jsonrpc: "2.0", result: {} };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject an empty object", () => {
    const obj = {};
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject when object has id but neither result nor error", () => {
    const obj = { jsonrpc: "2.0", id: 1 };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should reject when both method and result are present (treat as request)", () => {
    // This should be treated as a request because it has method
    const obj = { jsonrpc: "2.0", id: 1, method: "storage/get", result: {} };
    expect(isJsonRpcResponse(obj)).toBe(false);
  });

  it("should accept a response with result: undefined (key present)", () => {
    // "result" key is present but value is undefined — still a response shape
    const obj: Record<string, unknown> = { jsonrpc: "2.0", id: 1 };
    // Explicitly set result key
    Object.defineProperty(obj, "result", { value: undefined, enumerable: true });
    expect(isJsonRpcResponse(obj)).toBe(true);
  });
});

// =============================================================================
// Storage Routing in handleLine
// =============================================================================

describe("Storage Response Routing", () => {
  it("should distinguish a storage response from a host request", () => {
    // A storage response: has id + result, no method
    const storageResponse = { jsonrpc: "2.0", id: 42, result: { data: "cached_value" } };
    expect(isJsonRpcResponse(storageResponse)).toBe(true);

    // A host request: has method field
    const hostRequest = { jsonrpc: "2.0", id: 1, method: "recommendations/get", params: {} };
    expect(isJsonRpcResponse(hostRequest)).toBe(false);
  });

  it("should distinguish a storage error response from a host request", () => {
    const storageError = {
      jsonrpc: "2.0",
      id: 5,
      error: { code: -32002, message: "Key not found" },
    };
    expect(isJsonRpcResponse(storageError)).toBe(true);
  });

  it("should not misclassify a parse error response (null id) as a storage response", () => {
    // Parse errors have id: null — these should NOT be routed to storage
    const parseError = {
      jsonrpc: "2.0",
      id: null,
      error: { code: -32700, message: "Parse error" },
    };
    expect(isJsonRpcResponse(parseError)).toBe(false);
  });
});
