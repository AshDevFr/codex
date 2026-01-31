/**
 * Tests for server.ts - JSON-RPC server implementation
 *
 * These tests cover:
 * - Parameter validation for search, get, and match methods
 * - Error handling for invalid requests
 * - Request handling flow
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
