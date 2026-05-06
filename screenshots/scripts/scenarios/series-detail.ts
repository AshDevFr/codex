import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForThumbnails } from "../utils/wait.js";

/**
 * Series detail extras scenario.
 *
 * Captures the new surfaces on the series detail and library pages:
 *  1. Bulk selection toolbar + Bulk Metadata Edit modal (Library page)
 *  2. Series detail actions menu (renumber, reset, edit metadata)
 *  3. Series Metadata Edit modal
 *  4. Series Info modal (read-only details)
 *  5. External IDs edit modal
 *  6. Reset Metadata confirmation
 *
 * Runs after Libraries (so series exist) and Plugins (so the
 * "Fetch Metadata" submenu has entries).
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  📚 Capturing series detail extras...");

  await captureBulkMetadataFlow(page);
  await captureSeriesDetailExtras(page);
}

/**
 * Library page → enter bulk-select mode by clicking a card's checkbox →
 * select a couple of series → open Edit Metadata modal → tab through.
 */
async function captureBulkMetadataFlow(page: Page): Promise<void> {
  console.log("    📷 Library — bulk metadata edit");

  await page.goto("/libraries/all/series");
  await waitForPageReady(page);
  await waitForThumbnails(page);
  await page.waitForTimeout(500);

  // The selection checkbox is hidden until hovered or until selection
  // mode activates. Hovering then clicking the first card's checkbox
  // is the canonical entry point. We click it via JS to bypass the
  // hover-only CSS.
  const firstCheckbox = page.locator('[data-selection-checkbox] input[type="checkbox"]').first();
  if ((await firstCheckbox.count()) === 0) {
    console.log("      ⚠️  No selection checkboxes found");
    return;
  }
  await firstCheckbox.evaluate((el) => (el as HTMLInputElement).click());
  await page.waitForTimeout(400);

  // Select a second card so the bulk metadata modal has more than one
  // entry to merge.
  const secondCheckbox = page.locator('[data-selection-checkbox] input[type="checkbox"]').nth(1);
  if ((await secondCheckbox.count()) > 0) {
    await secondCheckbox.evaluate((el) => (el as HTMLInputElement).click());
    await page.waitForTimeout(400);
  }

  // Capture the bulk selection toolbar visible above the grid.
  await captureScreenshot(page, "series-detail/bulk-selection-toolbar");

  // Open the Edit Metadata modal.
  const editButton = page.locator('button:has-text("Edit Metadata")').first();
  if ((await editButton.count()) === 0) {
    console.log("      ⚠️  Edit Metadata button not visible (missing permission?)");
    return;
  }
  await editButton.click();
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', {
    state: "visible",
    timeout: 5000,
  });
  await page.waitForTimeout(600);

  // The modal opens on the General tab.
  await captureScreenshot(page, "series-detail/bulk-metadata-general");

  // Tab through the secondary tabs the modal exposes.
  for (const [tabName, screenshot] of [
    ["Authors", "series-detail/bulk-metadata-authors"],
    ["Tags", "series-detail/bulk-metadata-tags"],
    ["Custom", "series-detail/bulk-metadata-custom"],
  ] as const) {
    const tab = page.locator(`button[role="tab"]:has-text("${tabName}")`).first();
    if ((await tab.count()) === 0) continue;
    await tab.click();
    await page.waitForTimeout(400);
    await captureScreenshot(page, screenshot);
  }

  // Close without saving — destructive on the only fixture data.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(400);

  // Clear selection so subsequent scenarios start clean.
  const clearButton = page.locator(
    'button[aria-label="Clear selection"], button:has-text("Cancel")',
  ).first();
  if ((await clearButton.count()) > 0) {
    await clearButton.click().catch(() => {});
  }
  await page.waitForTimeout(300);
}

/**
 * Series detail actions menu + the modals it triggers. We capture the
 * menu open and the Edit Metadata + Series Info + Reset Metadata
 * confirmation modals, but never click through any destructive action.
 */
async function captureSeriesDetailExtras(page: Page): Promise<void> {
  console.log("    📷 Series detail — actions menu + modals");

  await page.goto("/libraries/all/series");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  const seriesCard = await page.$('[data-testid="series-card"], .series-card, a[href*="/series/"]');
  if (!seriesCard) {
    console.log("      ⚠️  No series found");
    return;
  }
  await seriesCard.click();
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  // Open the actions menu (kebab in the header grid).
  const actionsMenu = page.locator(
    '.mantine-Grid-root button:has(svg.tabler-icon-dots-vertical)',
  ).first();
  if ((await actionsMenu.count()) === 0) {
    console.log("      ⚠️  Actions menu not found");
    return;
  }
  await actionsMenu.click();
  await page.waitForTimeout(400);
  await captureScreenshot(page, "series-detail/actions-menu");

  // Capture Reset Metadata confirmation modal — opens directly from
  // the menu, so we click it then capture before dismissing.
  const resetItem = page.locator('[role="menuitem"]:has-text("Reset Metadata")').first();
  if ((await resetItem.count()) > 0) {
    await resetItem.click();
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', {
      state: "visible",
      timeout: 5000,
    });
    await page.waitForTimeout(500);
    await captureScreenshot(page, "series-detail/reset-metadata-confirm");
    // Dismiss without confirming.
    await page.keyboard.press("Escape");
    await page.waitForTimeout(400);
  }

  // Re-open the actions menu and click Edit Metadata to capture the
  // modal.
  await actionsMenu.click();
  await page.waitForTimeout(400);
  const editItem = page.locator('[role="menuitem"]:has-text("Edit Metadata")').first();
  if ((await editItem.count()) > 0) {
    await editItem.click();
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', {
      state: "visible",
      timeout: 5000,
    });
    await page.waitForTimeout(600);
    await captureScreenshot(page, "series-detail/edit-metadata-modal");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(400);
  } else {
    await page.keyboard.press("Escape");
  }

  // Series Info modal — opens via a dedicated info button on the
  // series header. The IconInfoCircle is the only "info" tabler icon
  // in that area.
  const infoButton = page.locator(
    '.mantine-Grid-root button:has(svg.tabler-icon-info-circle)',
  ).first();
  if ((await infoButton.count()) > 0) {
    await infoButton.click();
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', {
      state: "visible",
      timeout: 5000,
    });
    await page.waitForTimeout(500);
    await captureScreenshot(page, "series-detail/info-modal");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(400);
  }
}
