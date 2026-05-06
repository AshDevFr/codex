import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Per-library scheduled jobs scenario.
 *
 * Captures the LibraryJobs page (empty state) and the JobEditor modal
 * mid-creation: the metadata-refresh job UI with provider selection,
 * cron presets, field-group toggles, and matching strategy.
 *
 * Runs after Plugins so the provider dropdown has entries.
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  ⏱  Capturing per-library scheduled jobs...");

  // Find the manga library's id from the sidebar.
  await page.goto("/");
  await waitForPageReady(page);
  await page.waitForTimeout(400);

  const mangaLink = page.locator('nav a[href*="/libraries/"]:has-text("Manga")').first();
  if ((await mangaLink.count()) === 0) {
    console.log("    ⚠️  Manga library not found in sidebar");
    return;
  }
  const mangaHref = await mangaLink.getAttribute("href");
  if (!mangaHref) {
    console.log("    ⚠️  Manga library href missing");
    return;
  }
  // Hrefs look like /libraries/<uuid>/series — strip the trailing tab.
  const libraryId = mangaHref.split("/")[2];
  if (!libraryId) {
    console.log("    ⚠️  Could not parse library id from href");
    return;
  }

  await page.goto(`/libraries/${libraryId}/jobs`);
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  // Empty-state list (no jobs yet).
  await captureScreenshot(page, "library-jobs/empty");

  // Open the editor.
  const addButton = page.locator('button:has-text("Add job")').first();
  if ((await addButton.count()) === 0) {
    console.log("    ⚠️  Add job button not found");
    return;
  }
  await addButton.click();
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', {
    state: "visible",
    timeout: 5000,
  });
  await page.waitForTimeout(800);

  // Capture the editor in its initial state.
  await captureScreenshot(page, "library-jobs/editor-empty");

  // Fill in a representative job so the screenshot looks real.
  const nameInput = page.locator('input[placeholder*="Daily" i], label:has-text("Name") + * input').first();
  if ((await nameInput.count()) > 0) {
    await nameInput.fill("Daily metadata refresh");
    await page.waitForTimeout(200);
  }

  // Pick a provider. The Select label reads "Provider" or "Plugin" —
  // Mantine wraps the input in `.mantine-Select-input`.
  const providerSelect = page.locator(
    'label:has-text("Provider"), label:has-text("Plugin")',
  ).locator('..').locator('.mantine-Select-input').first();
  if ((await providerSelect.count()) > 0) {
    await providerSelect.click();
    await page.waitForTimeout(300);
    const firstOption = page.locator('[role="option"]').first();
    if ((await firstOption.count()) > 0) {
      await firstOption.click();
      await page.waitForTimeout(400);
    } else {
      await page.keyboard.press("Escape");
    }
  }

  await captureScreenshot(page, "library-jobs/editor-filled");

  // Close without saving.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(400);
}
