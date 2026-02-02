import { describe, expect, it } from "vitest";
import {
  ERROR_TYPE_COLORS,
  ERROR_TYPE_DESCRIPTIONS,
  ERROR_TYPE_LABELS,
  ERROR_TYPES_ORDER,
  getErrorTypeColor,
  getErrorTypeDescription,
  getErrorTypeLabel,
  isAnalysisError,
} from "./bookErrors";

describe("bookErrors utility functions", () => {
  describe("getErrorTypeLabel", () => {
    it("should return correct label for format_detection", () => {
      expect(getErrorTypeLabel("format_detection")).toBe("Format Detection");
    });

    it("should return correct label for parser", () => {
      expect(getErrorTypeLabel("parser")).toBe("Parser Error");
    });

    it("should return correct label for metadata", () => {
      expect(getErrorTypeLabel("metadata")).toBe("Metadata Error");
    });

    it("should return correct label for thumbnail", () => {
      expect(getErrorTypeLabel("thumbnail")).toBe("Thumbnail Error");
    });

    it("should return correct label for page_extraction", () => {
      expect(getErrorTypeLabel("page_extraction")).toBe("Page Extraction");
    });

    it("should return correct label for pdf_rendering", () => {
      expect(getErrorTypeLabel("pdf_rendering")).toBe("PDF Rendering");
    });

    it("should return correct label for other", () => {
      expect(getErrorTypeLabel("other")).toBe("Other Error");
    });
  });

  describe("getErrorTypeColor", () => {
    it("should return grape for format_detection", () => {
      expect(getErrorTypeColor("format_detection")).toBe("grape");
    });

    it("should return red for parser", () => {
      expect(getErrorTypeColor("parser")).toBe("red");
    });

    it("should return gray for unknown types", () => {
      // @ts-expect-error Testing unknown type
      expect(getErrorTypeColor("unknown_type")).toBe("gray");
    });
  });

  describe("getErrorTypeDescription", () => {
    it("should return description for format_detection", () => {
      expect(getErrorTypeDescription("format_detection")).toContain(
        "file format could not be identified",
      );
    });

    it("should return description for thumbnail", () => {
      expect(getErrorTypeDescription("thumbnail")).toContain(
        "Thumbnail generation failed",
      );
    });

    it("should return fallback for unknown types", () => {
      // @ts-expect-error Testing unknown type
      expect(getErrorTypeDescription("unknown_type")).toBe(
        "Unknown error type.",
      );
    });
  });

  describe("isAnalysisError", () => {
    it("should return true for parser errors", () => {
      expect(isAnalysisError("parser")).toBe(true);
    });

    it("should return true for format_detection errors", () => {
      expect(isAnalysisError("format_detection")).toBe(true);
    });

    it("should return true for metadata errors", () => {
      expect(isAnalysisError("metadata")).toBe(true);
    });

    it("should return true for page_extraction errors", () => {
      expect(isAnalysisError("page_extraction")).toBe(true);
    });

    it("should return true for pdf_rendering errors", () => {
      expect(isAnalysisError("pdf_rendering")).toBe(true);
    });

    it("should return true for other errors", () => {
      expect(isAnalysisError("other")).toBe(true);
    });

    it("should return false for thumbnail errors", () => {
      expect(isAnalysisError("thumbnail")).toBe(false);
    });
  });

  describe("ERROR_TYPES_ORDER", () => {
    it("should include all error types", () => {
      expect(ERROR_TYPES_ORDER).toContain("parser");
      expect(ERROR_TYPES_ORDER).toContain("format_detection");
      expect(ERROR_TYPES_ORDER).toContain("page_extraction");
      expect(ERROR_TYPES_ORDER).toContain("thumbnail");
      expect(ERROR_TYPES_ORDER).toContain("pdf_rendering");
      expect(ERROR_TYPES_ORDER).toContain("metadata");
      expect(ERROR_TYPES_ORDER).toContain("other");
    });

    it("should have parser first", () => {
      expect(ERROR_TYPES_ORDER[0]).toBe("parser");
    });

    it("should have other last", () => {
      expect(ERROR_TYPES_ORDER[ERROR_TYPES_ORDER.length - 1]).toBe("other");
    });
  });

  describe("ERROR_TYPE_LABELS constant", () => {
    it("should have labels for all error types", () => {
      expect(Object.keys(ERROR_TYPE_LABELS)).toHaveLength(8);
    });
  });

  describe("ERROR_TYPE_COLORS constant", () => {
    it("should have colors for all error types", () => {
      expect(Object.keys(ERROR_TYPE_COLORS)).toHaveLength(8);
    });
  });

  describe("ERROR_TYPE_DESCRIPTIONS constant", () => {
    it("should have descriptions for all error types", () => {
      expect(Object.keys(ERROR_TYPE_DESCRIPTIONS)).toHaveLength(8);
    });
  });
});
