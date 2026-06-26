import { describe, expect, it } from "vitest";
import { bookCoverUrl } from "./coverUrl";

describe("bookCoverUrl", () => {
  it("builds the thumbnail URL for a book id", () => {
    expect(bookCoverUrl("abc-123")).toBe("/api/v1/books/abc-123/thumbnail");
  });
});
