import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForImages } from "../utils/wait.js";

/**
 * Advanced search scenario
 * Captures the /search page with the nested filter builder, presets menu,
 * and series/books tabs.
 *
 * Runs after Libraries so the catalog has scanned content to match against.
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  🔎 Capturing advanced search...");

  // Empty advanced search page (builder open, no query, no filters)
  console.log("    📷 Advanced search empty state");
  await page.goto("/search");
  await waitForPageReady(page);
  await captureScreenshot(page, "search/advanced-empty");

  // Run a query that should match seeded content. Wait for results to load.
  console.log("    📷 Advanced search with results");
  const queryInput = await page.$('input[aria-label="Search query"]');
  if (queryInput) {
    await queryInput.fill("a");
    await page.keyboard.press("Enter");
    await waitForPageReady(page);
    await waitForImages(page).catch(() => {});
    await page.waitForTimeout(800);
    await captureScreenshot(page, "search/advanced-results-series");

    // Switch to the Books tab so cross-tab filtering is visible.
    const booksTab = await page.$('button[role="tab"]:has-text("Books"), [role="tab"]:has-text("Books")');
    if (booksTab) {
      await booksTab.click();
      await waitForPageReady(page);
      await waitForImages(page).catch(() => {});
      await page.waitForTimeout(500);
      await captureScreenshot(page, "search/advanced-results-books");
    }
  } else {
    console.log("    ⚠️  Search query input not found");
  }

  // Open the Sort selector to show the Relevance option that the fuzzy
  // index exposes when there is a query.
  console.log("    📷 Sort dropdown (relevance)");
  const sortSelect = await page.$('input[placeholder="Relevance"], input[placeholder="Default"]');
  if (sortSelect) {
    await sortSelect.click();
    await page.waitForTimeout(400);
    await captureScreenshot(page, "search/sort-dropdown");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }

  // Open the Presets menu so the saved-filters surface is visible.
  console.log("    📷 Presets menu");
  const presetsButton = await page.$('button:has-text("Presets")');
  if (presetsButton) {
    await presetsButton.click();
    await page.waitForTimeout(400);
    await captureScreenshot(page, "search/presets-menu");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  } else {
    console.log("    ⚠️  Presets button not found");
  }
}
