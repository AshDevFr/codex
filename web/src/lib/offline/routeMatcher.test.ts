import { describe, expect, it } from "vitest";
import {
  cacheNameForBook,
  matchDownloadedBookRequest,
  shouldServeSpaShell,
} from "./routeMatcher";

function u(path: string): URL {
  return new URL(`https://example.com${path}`);
}

describe("matchDownloadedBookRequest", () => {
  const downloaded = new Set<string>(["abc", "xyz-123"]);

  it("matches a /pages/N request for a downloaded book", () => {
    const result = matchDownloadedBookRequest(
      u("/api/v1/books/abc/pages/7"),
      "GET",
      downloaded,
    );
    expect(result).toEqual({
      bookId: "abc",
      resource: { kind: "page", number: 7 },
    });
  });

  it("matches a /file request for a downloaded book", () => {
    const result = matchDownloadedBookRequest(
      u("/api/v1/books/xyz-123/file"),
      "GET",
      downloaded,
    );
    expect(result).toEqual({
      bookId: "xyz-123",
      resource: { kind: "file" },
    });
  });

  it("returns null for books that are not in the downloaded set", () => {
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/not-downloaded/file"),
        "GET",
        downloaded,
      ),
    ).toBeNull();
  });

  it("returns null for non-GET methods even when the book is downloaded", () => {
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/abc/file"),
        "PUT",
        downloaded,
      ),
    ).toBeNull();
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/abc/pages/1"),
        "DELETE",
        downloaded,
      ),
    ).toBeNull();
  });

  it("does not match unrelated API paths", () => {
    expect(
      matchDownloadedBookRequest(u("/api/v1/books/abc"), "GET", downloaded),
    ).toBeNull();
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/abc/thumbnail"),
        "GET",
        downloaded,
      ),
    ).toBeNull();
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/series/abc/file"),
        "GET",
        downloaded,
      ),
    ).toBeNull();
  });

  it("does not match versioned paths outside /v1/", () => {
    expect(
      matchDownloadedBookRequest(
        u("/api/v2/books/abc/file"),
        "GET",
        downloaded,
      ),
    ).toBeNull();
  });

  it("does not match a /pages path with a non-numeric segment", () => {
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/abc/pages/foo"),
        "GET",
        downloaded,
      ),
    ).toBeNull();
  });

  it("returns null for an empty downloaded set", () => {
    expect(
      matchDownloadedBookRequest(
        u("/api/v1/books/abc/file"),
        "GET",
        new Set<string>(),
      ),
    ).toBeNull();
  });

  it("ignores query strings and hash fragments", () => {
    const result = matchDownloadedBookRequest(
      u("/api/v1/books/abc/pages/3?x=1#y"),
      "GET",
      downloaded,
    );
    expect(result?.bookId).toBe("abc");
    expect(result?.resource).toEqual({ kind: "page", number: 3 });
  });
});

describe("cacheNameForBook", () => {
  it("produces a deterministic per-book cache name", () => {
    expect(cacheNameForBook("abc")).toBe("codex-book-abc");
    expect(cacheNameForBook("xyz-123")).toBe("codex-book-xyz-123");
  });
});

describe("shouldServeSpaShell", () => {
  it("serves the SPA shell for client-side app routes", () => {
    expect(shouldServeSpaShell("/")).toBe(true);
    expect(shouldServeSpaShell("/library/123/series/abc")).toBe(true);
    expect(shouldServeSpaShell("/settings")).toBe(true);
  });

  it("bypasses the SPA shell for the bare /docs Scalar route", () => {
    // Regression: /docs is a leaf route. A denylist anchored to /docs/ would
    // miss the bare path and the active SW would serve the React shell, which
    // has no /docs route (404).
    expect(shouldServeSpaShell("/docs")).toBe(false);
    expect(shouldServeSpaShell("/docs/")).toBe(false);
  });

  it("bypasses the SPA shell for backend API prefixes", () => {
    expect(shouldServeSpaShell("/api/v1/books")).toBe(false);
    expect(shouldServeSpaShell("/opds/v2/catalog")).toBe(false);
    expect(shouldServeSpaShell("/komga/api/v1/series")).toBe(false);
    expect(shouldServeSpaShell("/health")).toBe(false);
  });

  it("does not over-match app routes that merely start with a backend word", () => {
    // /docsomething is a hypothetical app route, not the Scalar docs page.
    expect(shouldServeSpaShell("/docsomething")).toBe(true);
    expect(shouldServeSpaShell("/healthcheck")).toBe(true);
  });
});
