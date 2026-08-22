import { describe, expect, it } from "vitest";
import { handlers } from "./index";

/**
 * The mock handlers back `make frontend-mock`, the dev workflow that runs the
 * app against MSW with no backend. They are wired through `setupWorker`, which
 * only runs in a browser, so the vitest suite never loads them — nothing else in
 * this repo can catch a route the app calls and the mocks do not serve.
 *
 * That gap is not hypothetical. `/books/{id}/full` and `/series/{id}/full` were
 * added to the API and adopted by BookDetail and SeriesDetail while the mocks
 * still only knew the deprecated `?full=true` form, and the suite stayed green
 * because neither page has a test.
 */
describe("mock handler coverage", () => {
  const paths = new Set(
    handlers.map((handler) => String(handler.info.path)).filter(Boolean),
  );

  it.each([
    ["/api/v1/books/:id/full"],
    ["/api/v1/series/:id/full"],
    ["/api/v1/series/full"],
  ])("serves %s", (path) => {
    expect(paths).toContain(path);
  });

  /**
   * MSW resolves handlers in array order, so a `:id` pattern declared first
   * would swallow `/series/full` and read "full" as a series id. The real router
   * has the same constraint and solves it the same way.
   */
  it("declares the literal /series/full before the /series/:id pattern", () => {
    // Method matters: MSW matches on method *and* path, so the PATCH handler
    // for /series/:id cannot shadow a GET however early it is declared.
    const ordered = handlers
      .filter((handler) => String(handler.info.method) === "GET")
      .map((handler) => String(handler.info.path));
    const literal = ordered.indexOf("/api/v1/series/full");
    const pattern = ordered.indexOf("/api/v1/series/:id");

    expect(literal).toBeGreaterThanOrEqual(0);
    expect(pattern).toBeGreaterThanOrEqual(0);
    expect(literal).toBeLessThan(pattern);
  });
});
