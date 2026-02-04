import { describe, expect, it } from "vitest";
import { mapSearchDocToBookPreview, mapSearchDocToSearchResult } from "./mapper.js";
import type { OLSearchDoc } from "./types.js";

describe("mapSearchDocToSearchResult", () => {
  it("should map basic search doc fields", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      first_publish_year: 2011,
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.externalId).toBe("/works/OL45883W");
    expect(result.title).toBe("The Martian");
    expect(result.year).toBe(2011);
  });

  it("should include subtitle in alternateTitles", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      subtitle: "A Novel",
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.alternateTitles).toContain("A Novel");
  });

  it("should generate cover URL from cover_i", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      cover_i: 8231849,
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.coverUrl).toBe("https://covers.openlibrary.org/b/id/8231849-M.jpg");
  });

  it("should calculate relevance score based on data completeness", () => {
    const minimalDoc: OLSearchDoc = {
      key: "/works/OL1",
      title: "Minimal",
    };

    const completeDoc: OLSearchDoc = {
      key: "/works/OL2",
      title: "Complete",
      author_name: ["Author"],
      isbn: ["9780553418026"],
      cover_i: 123456,
      first_publish_year: 2020,
      subject: ["Fiction"],
      ratings_count: 100,
    };

    const minimalResult = mapSearchDocToSearchResult(minimalDoc);
    const completeResult = mapSearchDocToSearchResult(completeDoc);

    expect(completeResult.relevanceScore).toBeGreaterThan(minimalResult.relevanceScore || 0);
  });

  it("should include subjects in preview genres", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      subject: ["Science Fiction", "Space Exploration", "Survival", "Mars"],
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.preview?.genres).toEqual([
      "Science Fiction",
      "Space Exploration",
      "Survival",
      "Mars",
    ]);
  });

  it("should limit preview genres to 5", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      subject: ["A", "B", "C", "D", "E", "F", "G"],
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.preview?.genres).toHaveLength(5);
  });

  it("should include authors in preview", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      author_name: ["Andy Weir"],
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.preview?.authors).toContain("Andy Weir");
  });

  it("should include publisher in preview description", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      author_name: ["Andy Weir"],
      publisher: ["Crown Publishing"],
    };

    const result = mapSearchDocToSearchResult(doc);

    expect(result.preview?.description).toContain("Crown Publishing");
  });

  it("should round ratings to nearest 0.5", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      ratings_average: 4.33, // Open Library uses 1-5 scale
    };

    const result = mapSearchDocToSearchResult(doc);

    // Math.round(4.33 * 2) / 2 = Math.round(8.66) / 2 = 9 / 2 = 4.5
    expect(result.preview?.rating).toBe(4.5);
  });
});

describe("mapSearchDocToBookPreview", () => {
  it("should map basic fields", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      first_publish_year: 2011,
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.externalId).toBe("/works/OL45883W");
    expect(result.title).toBe("The Martian");
    expect(result.year).toBe(2011);
  });

  it("should include ISBNs", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      isbn: ["9780553418026", "0553418025", "9780091956141"],
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.isbn).toBe("9780553418026");
    expect(result.isbns).toEqual(["9780553418026", "0553418025", "9780091956141"]);
  });

  it("should limit ISBNs to 5", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      isbn: ["1", "2", "3", "4", "5", "6", "7"],
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.isbns).toHaveLength(5);
  });

  it("should map authors", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      author_name: ["Andy Weir", "Jane Doe"],
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.authors).toHaveLength(2);
    expect(result.authors[0]).toEqual({ name: "Andy Weir", role: "author" });
    expect(result.authors[1]).toEqual({ name: "Jane Doe", role: "author" });
  });

  it("should map subjects with limit", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      subject: Array.from({ length: 20 }, (_, i) => `Subject ${i}`),
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.subjects).toHaveLength(10);
  });

  it("should map page count", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      number_of_pages_median: 369,
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.pageCount).toBe(369);
  });

  it("should generate cover URLs", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      isbn: ["9780553418026"],
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.coverUrl).toBe("https://covers.openlibrary.org/b/isbn/9780553418026-L.jpg");
    expect(result.covers).toHaveLength(3);
    expect(result.covers[0].size).toBe("small");
    expect(result.covers[1].size).toBe("medium");
    expect(result.covers[2].size).toBe("large");
  });

  it("should use cover_i when no ISBN available", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      cover_i: 8231849,
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.coverUrl).toBe("https://covers.openlibrary.org/b/id/8231849-L.jpg");
    expect(result.covers[0].url).toContain("/b/id/8231849");
  });

  it("should map ratings", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
      ratings_average: 4.2,
      ratings_count: 1000,
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.rating).toBeDefined();
    expect(result.rating?.score).toBe(84); // 4.2 * 20 = 84
    expect(result.rating?.voteCount).toBe(1000);
    expect(result.rating?.source).toBe("openlibrary");
    expect(result.externalRatings).toHaveLength(1);
  });

  it("should include external link to Open Library", () => {
    const doc: OLSearchDoc = {
      key: "/works/OL45883W",
      title: "The Martian",
    };

    const result = mapSearchDocToBookPreview(doc);

    expect(result.externalLinks).toHaveLength(1);
    expect(result.externalLinks[0]).toEqual({
      url: "https://openlibrary.org/works/OL45883W",
      label: "Open Library",
      linkType: "provider",
    });
  });
});
