import { describe, expect, it } from "vitest";
import { ApiError, AuthError, ConfigError, NotFoundError, RateLimitError } from "./errors.js";
import { PLUGIN_ERROR_CODES } from "./types/rpc.js";

describe("PluginError classes", () => {
  describe("RateLimitError", () => {
    it("should have correct code and retry time", () => {
      const error = new RateLimitError(60);

      expect(error.code).toBe(PLUGIN_ERROR_CODES.RATE_LIMITED);
      expect(error.retryAfterSeconds).toBe(60);
      expect(error.message).toBe("Rate limited, retry after 60s");
    });

    it("should accept custom message", () => {
      const error = new RateLimitError(30, "Too many requests");

      expect(error.message).toBe("Too many requests");
      expect(error.retryAfterSeconds).toBe(30);
    });

    it("should convert to JSON-RPC error", () => {
      const error = new RateLimitError(60);
      const rpcError = error.toJsonRpcError();

      expect(rpcError).toEqual({
        code: PLUGIN_ERROR_CODES.RATE_LIMITED,
        message: "Rate limited, retry after 60s",
        data: { retryAfterSeconds: 60 },
      });
    });
  });

  describe("NotFoundError", () => {
    it("should have correct code", () => {
      const error = new NotFoundError("Series 123 not found");

      expect(error.code).toBe(PLUGIN_ERROR_CODES.NOT_FOUND);
      expect(error.message).toBe("Series 123 not found");
    });

    it("should convert to JSON-RPC error", () => {
      const error = new NotFoundError("Not found");
      const rpcError = error.toJsonRpcError();

      expect(rpcError).toEqual({
        code: PLUGIN_ERROR_CODES.NOT_FOUND,
        message: "Not found",
        data: undefined,
      });
    });
  });

  describe("AuthError", () => {
    it("should have correct code and default message", () => {
      const error = new AuthError();

      expect(error.code).toBe(PLUGIN_ERROR_CODES.AUTH_FAILED);
      expect(error.message).toBe("Authentication failed");
    });

    it("should accept custom message", () => {
      const error = new AuthError("Invalid API key");

      expect(error.message).toBe("Invalid API key");
    });
  });

  describe("ApiError", () => {
    it("should have correct code and status", () => {
      const error = new ApiError("Server error", 500);

      expect(error.code).toBe(PLUGIN_ERROR_CODES.API_ERROR);
      expect(error.statusCode).toBe(500);
      expect(error.message).toBe("Server error");
    });

    it("should convert to JSON-RPC error with status", () => {
      const error = new ApiError("Bad gateway", 502);
      const rpcError = error.toJsonRpcError();

      expect(rpcError).toEqual({
        code: PLUGIN_ERROR_CODES.API_ERROR,
        message: "Bad gateway",
        data: { statusCode: 502 },
      });
    });
  });

  describe("ConfigError", () => {
    it("should have correct code", () => {
      const error = new ConfigError("Missing API key");

      expect(error.code).toBe(PLUGIN_ERROR_CODES.CONFIG_ERROR);
      expect(error.message).toBe("Missing API key");
    });
  });
});
