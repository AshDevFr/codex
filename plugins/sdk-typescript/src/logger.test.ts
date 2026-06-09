/**
 * Tests for logger.ts - level filtering and runtime level changes.
 *
 * The host sends a `logLevel` at initialize (from the Codex `plugins.log_level`
 * config); plugins apply it via `logger.setLevel(...)`. These tests pin the
 * filtering behavior that toggle depends on.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createLogger } from "./logger.js";

describe("Logger level filtering", () => {
  let written: string[];
  let spy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    written = [];
    spy = vi.spyOn(process.stderr, "write").mockImplementation((chunk: string | Uint8Array) => {
      written.push(String(chunk));
      return true;
    });
  });

  afterEach(() => {
    spy.mockRestore();
  });

  it("suppresses messages below the configured level", () => {
    const logger = createLogger({ name: "t", level: "info", timestamps: false });
    logger.debug("hidden");
    logger.info("shown");
    expect(written.join("")).not.toContain("hidden");
    expect(written.join("")).toContain("shown");
  });

  it("setLevel('debug') enables previously-suppressed debug output", () => {
    const logger = createLogger({ name: "t", level: "info", timestamps: false });
    logger.debug("before");
    expect(written.join("")).not.toContain("before");

    logger.setLevel("debug");
    logger.debug("after");
    expect(written.join("")).toContain("after");
  });

  it("setLevel('error') silences info/warn", () => {
    const logger = createLogger({ name: "t", level: "debug", timestamps: false });
    logger.setLevel("error");
    logger.info("info-msg");
    logger.warn("warn-msg");
    logger.error("error-msg");
    const out = written.join("");
    expect(out).not.toContain("info-msg");
    expect(out).not.toContain("warn-msg");
    expect(out).toContain("error-msg");
  });
});
