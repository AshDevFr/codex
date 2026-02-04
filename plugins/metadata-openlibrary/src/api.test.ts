import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  buildOpenLibraryUrl,
  clearCache,
  extractOlid,
  getCoverUrlById,
  getCoverUrlByIsbn,
  getCoverUrlByOlid,
  isValidIsbn,
  normalizeIsbn,
  parseDescription,
  parseLanguage,
  parseYear,
} from "./api.js";

describe("ISBN utilities", () => {
  describe("normalizeIsbn", () => {
    it("should remove hyphens from ISBN", () => {
      expect(normalizeIsbn("978-0-553-41802-6")).toBe("9780553418026");
    });

    it("should remove spaces from ISBN", () => {
      expect(normalizeIsbn("978 0 553 41802 6")).toBe("9780553418026");
    });

    it("should convert to uppercase", () => {
      expect(normalizeIsbn("030640615x")).toBe("030640615X");
    });

    it("should handle already clean ISBNs", () => {
      expect(normalizeIsbn("9780553418026")).toBe("9780553418026");
    });
  });

  describe("isValidIsbn", () => {
    it("should accept valid ISBN-10", () => {
      expect(isValidIsbn("0306406152")).toBe(true);
    });

    it("should accept valid ISBN-13", () => {
      expect(isValidIsbn("9780553418026")).toBe(true);
    });

    it("should accept ISBN with hyphens", () => {
      expect(isValidIsbn("978-0-553-41802-6")).toBe(true);
    });

    it("should reject too short ISBN", () => {
      expect(isValidIsbn("12345")).toBe(false);
    });

    it("should reject too long ISBN", () => {
      expect(isValidIsbn("12345678901234567")).toBe(false);
    });
  });
});

describe("Date parsing", () => {
  describe("parseYear", () => {
    it("should parse year from simple string", () => {
      expect(parseYear("2020")).toBe(2020);
    });

    it("should parse year from full date", () => {
      expect(parseYear("January 1, 2020")).toBe(2020);
    });

    it("should parse year from ISO date", () => {
      expect(parseYear("2020-01-15")).toBe(2020);
    });

    it("should handle approximate dates", () => {
      expect(parseYear("c1985")).toBe(1985);
    });

    it("should handle uncertain dates", () => {
      expect(parseYear("1985?")).toBe(1985);
    });

    it("should return undefined for invalid dates", () => {
      expect(parseYear("unknown")).toBeUndefined();
    });

    it("should return undefined for undefined input", () => {
      expect(parseYear(undefined)).toBeUndefined();
    });

    it("should handle 19th century dates", () => {
      expect(parseYear("1895")).toBe(1895);
    });

    it("should handle 21st century dates", () => {
      expect(parseYear("2025")).toBe(2025);
    });
  });
});

describe("Description parsing", () => {
  describe("parseDescription", () => {
    it("should return string description as-is", () => {
      expect(parseDescription("This is a description")).toBe("This is a description");
    });

    it("should extract value from object description", () => {
      expect(
        parseDescription({
          type: "/type/text",
          value: "This is a description",
        }),
      ).toBe("This is a description");
    });

    it("should return undefined for undefined input", () => {
      expect(parseDescription(undefined)).toBeUndefined();
    });

    it("should strip HTML tags from descriptions", () => {
      const html =
        '<p><a href="https://example.com">Author\'s</a> pen ranges far and wide.</p><p>Second paragraph.</p>';
      const result = parseDescription(html);
      expect(result).toBe("Author's pen ranges far and wide.\nSecond paragraph.");
    });

    it("should strip HTML from object descriptions", () => {
      const result = parseDescription({
        type: "/type/text",
        value: "<p>Hello <b>world</b></p>",
      });
      expect(result).toBe("Hello world");
    });

    it("should handle br tags", () => {
      expect(parseDescription("Line one<br/>Line two<br>Line three")).toBe(
        "Line one\nLine two\nLine three",
      );
    });

    it("should decode HTML entities", () => {
      expect(parseDescription("Tom &amp; Jerry &lt;3")).toBe("Tom & Jerry <3");
    });

    it("should collapse excessive whitespace", () => {
      expect(parseDescription("<p>First</p>\n\n\n\n<p>Second</p>")).toBe("First\n\nSecond");
    });

    it("should return undefined for empty HTML", () => {
      expect(parseDescription("<p></p>")).toBeUndefined();
    });
  });
});

describe("Language parsing", () => {
  describe("parseLanguage", () => {
    it("should convert English", () => {
      expect(parseLanguage("/languages/eng")).toBe("en");
    });

    it("should convert Spanish", () => {
      expect(parseLanguage("/languages/spa")).toBe("es");
    });

    it("should convert French (fre)", () => {
      expect(parseLanguage("/languages/fre")).toBe("fr");
    });

    it("should convert French (fra)", () => {
      expect(parseLanguage("/languages/fra")).toBe("fr");
    });

    it("should convert Japanese", () => {
      expect(parseLanguage("/languages/jpn")).toBe("ja");
    });

    it("should convert Chinese", () => {
      expect(parseLanguage("/languages/chi")).toBe("zh");
    });

    it("should convert German (ger)", () => {
      expect(parseLanguage("/languages/ger")).toBe("de");
    });

    it("should convert German (deu)", () => {
      expect(parseLanguage("/languages/deu")).toBe("de");
    });

    it("should return unknown codes as-is", () => {
      expect(parseLanguage("/languages/xyz")).toBe("xyz");
    });

    it("should return undefined for undefined input", () => {
      expect(parseLanguage(undefined)).toBeUndefined();
    });

    it("should return undefined for invalid format", () => {
      expect(parseLanguage("eng")).toBeUndefined();
    });
  });
});

describe("OLID utilities", () => {
  describe("extractOlid", () => {
    it("should extract work OLID", () => {
      expect(extractOlid("/works/OL45883W")).toBe("OL45883W");
    });

    it("should extract book OLID", () => {
      expect(extractOlid("/books/OL7353617M")).toBe("OL7353617M");
    });

    it("should extract author OLID", () => {
      expect(extractOlid("/authors/OL34184A")).toBe("OL34184A");
    });

    it("should return already clean OLID as-is", () => {
      expect(extractOlid("OL45883W")).toBe("OL45883W");
    });
  });

  describe("buildOpenLibraryUrl", () => {
    it("should build URL from work key", () => {
      expect(buildOpenLibraryUrl("/works/OL45883W")).toBe("https://openlibrary.org/works/OL45883W");
    });

    it("should build URL from clean OLID", () => {
      expect(buildOpenLibraryUrl("OL45883W")).toBe("https://openlibrary.org/OL45883W");
    });

    it("should handle keys without leading slash", () => {
      expect(buildOpenLibraryUrl("works/OL45883W")).toBe("https://openlibrary.org/works/OL45883W");
    });
  });
});

describe("Cover URL generation", () => {
  describe("getCoverUrlByIsbn", () => {
    it("should generate small cover URL", () => {
      expect(getCoverUrlByIsbn("9780553418026", "S")).toBe(
        "https://covers.openlibrary.org/b/isbn/9780553418026-S.jpg",
      );
    });

    it("should generate medium cover URL", () => {
      expect(getCoverUrlByIsbn("978-0-553-41802-6", "M")).toBe(
        "https://covers.openlibrary.org/b/isbn/9780553418026-M.jpg",
      );
    });

    it("should generate large cover URL", () => {
      expect(getCoverUrlByIsbn("0553418025", "L")).toBe(
        "https://covers.openlibrary.org/b/isbn/0553418025-L.jpg",
      );
    });
  });

  describe("getCoverUrlById", () => {
    it("should generate cover URL by ID", () => {
      expect(getCoverUrlById(8231849, "M")).toBe(
        "https://covers.openlibrary.org/b/id/8231849-M.jpg",
      );
    });
  });

  describe("getCoverUrlByOlid", () => {
    it("should generate cover URL from OLID", () => {
      expect(getCoverUrlByOlid("OL7353617M", "L")).toBe(
        "https://covers.openlibrary.org/b/olid/OL7353617M-L.jpg",
      );
    });

    it("should strip prefix from full key", () => {
      expect(getCoverUrlByOlid("/books/OL7353617M", "M")).toBe(
        "https://covers.openlibrary.org/b/olid/OL7353617M-M.jpg",
      );
    });

    it("should strip work prefix", () => {
      expect(getCoverUrlByOlid("/works/OL45883W", "S")).toBe(
        "https://covers.openlibrary.org/b/olid/OL45883W-S.jpg",
      );
    });
  });
});

describe("Cache", () => {
  beforeEach(() => {
    clearCache();
  });

  afterEach(() => {
    clearCache();
  });

  it("should clear cache", () => {
    // Just verify clearCache doesn't throw
    expect(() => clearCache()).not.toThrow();
  });
});
