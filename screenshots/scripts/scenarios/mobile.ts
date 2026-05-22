import { BrowserContext, Page, devices } from "playwright";
import { config } from "../../playwright.config.js";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForImages } from "../utils/wait.js";

/**
 * Mobile scenario
 *
 * Captures a handful of screens at an iPhone viewport so the xs-breakpoint
 * layout (bottom navigation, full-screen search sheet, stacked toolbars,
 * card-collapsed admin tables) is visible in the docs.
 *
 * Runs after Libraries so there is real content to render.
 *
 * Reuses the parent browser by opening a new mobile context. The existing
 * desktop context's auth state is reapplied so we don't have to log in
 * again.
 */
export async function run(parentPage: Page, parentContext: BrowserContext): Promise<void> {
  console.log("  📱 Capturing mobile (iPhone 14) views...");

  const storageState = await parentContext.storageState();
  const browser = parentContext.browser();
  if (!browser) {
    console.log("    ⚠️  No browser available, skipping mobile scenario");
    return;
  }

  const mobileContext = await browser.newContext({
    ...devices["iPhone 14"],
    colorScheme: "dark",
    baseURL: config.baseUrl,
    storageState,
  });

  const page = await mobileContext.newPage();

  try {
    // Home dashboard at xs: stacked layout, bottom navigation bar, library
    // chips wrapping below the header.
    console.log("    📷 Mobile home dashboard");
    await page.goto("/");
    await waitForPageReady(page);
    await waitForImages(page).catch(() => {});
    await page.waitForTimeout(500);
    await captureScreenshot(page, "mobile/home-dashboard");

    // Series list at xs: cards reflow, toolbar collapses, alphabet jump
    // picker drops below the header.
    console.log("    📷 Mobile series list");
    await page.goto("/libraries/all/series");
    await waitForPageReady(page);
    await waitForImages(page).catch(() => {});
    await page.waitForTimeout(500);
    await captureScreenshot(page, "mobile/series-list");

    // Open the full-screen search sheet that replaces the inline search box
    // below the xs breakpoint.
    console.log("    📷 Mobile search sheet");
    const searchButton = await page.$(
      'button[aria-label*="Search" i], [data-testid="open-search-sheet"]'
    );
    if (searchButton) {
      await searchButton.click();
      await page.waitForTimeout(400);
      await captureScreenshot(page, "mobile/search-sheet");
      await page.keyboard.press("Escape");
      await page.waitForTimeout(200);
    } else {
      console.log("    ⚠️  Mobile search trigger not found");
    }

    // Settings → Users at xs: the admin table collapses into mobile-friendly
    // cards. Pick a settings page that we know has tabular data.
    console.log("    📷 Mobile admin users (card layout)");
    await page.goto("/settings/users");
    await waitForPageReady(page);
    await page.waitForTimeout(500);
    await captureScreenshot(page, "mobile/settings-users-cards");

    // Sidebar drawer open: navigation appears as an overlay drawer below xs.
    console.log("    📷 Mobile sidebar drawer");
    await page.goto("/");
    await waitForPageReady(page);
    const burger = await page.$('button[aria-label*="navigation" i], button[aria-label*="menu" i], .mantine-Burger-root');
    if (burger) {
      await burger.click();
      await page.waitForTimeout(400);
      await captureScreenshot(page, "mobile/sidebar-drawer");
    } else {
      console.log("    ⚠️  Mobile burger button not found");
    }
  } catch (error) {
    console.error("    ✗ Mobile scenario error:", error);
  } finally {
    await mobileContext.close();
  }
}
