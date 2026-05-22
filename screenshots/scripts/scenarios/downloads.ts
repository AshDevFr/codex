import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Offline downloads scenario
 * Captures Settings → Offline downloads.
 *
 * The page renders even with an empty offline cache (it shows the storage
 * quota meter and an empty-state message), so it does not depend on having
 * any books saved offline first.
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  ⬇️  Capturing offline downloads page...");

  await page.goto("/settings/downloads");
  await waitForPageReady(page);

  // The quota estimate is async — give the browser a moment to resolve it
  // and render the StorageManager-derived meter.
  await page.waitForTimeout(800);

  console.log("    📷 Offline downloads (desktop)");
  await captureScreenshot(page, "settings/downloads-offline");
}
