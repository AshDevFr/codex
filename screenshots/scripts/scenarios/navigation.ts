import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForImages } from "../utils/wait.js";
import { logout } from "../utils/auth.js";

/**
 * Navigation and miscellaneous pages scenario
 * Captures login page, search results, and other general views
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  🧭 Capturing navigation pages...");

  // Capture search functionality
  console.log("    📷 Search page");
  await captureSearchPage(page);

  // Capture home dashboard (should already have libraries at this point)
  console.log("    📷 Home dashboard");
  await page.goto("/");
  await waitForPageReady(page);
  await waitForImages(page);
  await captureScreenshot(page, "50-home-dashboard");

  // Capture sidebar expanded with settings
  console.log("    📷 Sidebar with settings");
  // Click on Settings to expand it
  const settingsNavItem = await page.$('nav button:has-text("Settings"), nav a:has-text("Settings")');
  if (settingsNavItem) {
    await settingsNavItem.click();
    await page.waitForTimeout(300);
    await captureScreenshot(page, "51-sidebar-settings-expanded");
  }

  // Capture login page (logout first)
  console.log("    📷 Login page");
  await logout(page);
  await waitForPageReady(page);
  await captureScreenshot(page, "52-login-page");
}

/**
 * Capture search page with results
 */
async function captureSearchPage(page: Page): Promise<void> {
  // Navigate to home first
  await page.goto("/");
  await waitForPageReady(page);

  // Find and click the search input/button
  const searchInput = await page.$('input[type="search"], input[placeholder*="Search"], [data-testid="search-input"]');

  if (searchInput) {
    await searchInput.click();
    await page.waitForTimeout(200);

    // Type a search query (needs at least 2 characters)
    await searchInput.fill("cook");
    await page.keyboard.press("Enter");

    // Wait for search results
    await page.waitForURL("**/search**", { timeout: 5000 }).catch(() => {
      // Search might not navigate, results could appear in-place
    });

    await waitForPageReady(page);
    await waitForImages(page);
    await page.waitForTimeout(500);

    await captureScreenshot(page, "53-search-results");
  } else {
    console.log("    ⚠️  Search input not found");
  }
}
