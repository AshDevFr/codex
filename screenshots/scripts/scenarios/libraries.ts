import { Page, BrowserContext } from "playwright";
import { config, type LibraryConfig } from "../../playwright.config.js";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForLibraryScan, waitForThumbnails } from "../utils/wait.js";
import { run as runSettings } from "./settings.js";

/**
 * Library creation and browsing scenario
 * Creates libraries, waits for scans, and captures various views
 */
export async function run(page: Page, context: BrowserContext): Promise<void> {
  // Ensure we're on the home page and logged in
  await page.goto("/");
  await waitForPageReady(page);

  // Create all libraries with specific configurations from config
  for (const libraryConfig of config.libraries) {
    console.log(`  📚 Creating library: ${libraryConfig.name} (${libraryConfig.type})`);
    await createLibrary(page, libraryConfig);
  }

  // While scans are running, capture Settings pages (so Tasks/Metrics have data)
  console.log("  ⚙️  Capturing settings while scan runs (for metrics data)...");
  await runSettings(page, context);

  // Wait for all library scans to complete
  await waitForLibraryScan(page);

  // Wait additional time for thumbnail generation (thumbnails are generated on-demand)
  console.log("  ⏳ Waiting for initial thumbnail generation...");
  await page.waitForTimeout(15000);

  // Navigate to home and capture library list
  console.log("  📷 Capturing library views...");
  await page.goto("/");
  await waitForPageReady(page);
  await waitForThumbnails(page);
  await captureScreenshot(page, "10-home-with-libraries");

  // Navigate to All Libraries view
  await page.goto("/libraries/all/series");
  await waitForPageReady(page);
  await waitForThumbnails(page);
  await captureScreenshot(page, "11-all-libraries-series");

  // Switch to books view
  await page.goto("/libraries/all/books");
  await waitForPageReady(page);
  await waitForThumbnails(page);
  await captureScreenshot(page, "12-all-libraries-books");

  // Get first library from sidebar and navigate to it
  const libraryLinks = await page.$$('nav a[href^="/libraries/"]');
  if (libraryLinks.length > 0) {
    // Find the first actual library link (not "all")
    for (const link of libraryLinks) {
      const href = await link.getAttribute("href");
      if (href && !href.includes("/all")) {
        await link.click();
        await waitForPageReady(page);
        await waitForThumbnails(page);
        await captureScreenshot(page, "13-library-detail-series");
        break;
      }
    }
  }

  // Try to find and click on a series
  const seriesCards = await page.$$('[data-testid="series-card"], .series-card, a[href*="/series/"]');
  if (seriesCards.length > 0) {
    await seriesCards[0].click();
    await waitForPageReady(page);
    await waitForThumbnails(page);
    await captureScreenshot(page, "14-series-detail");

    // Try to find and click on a book in the series
    const bookCards = await page.$$('[data-testid="book-card"], .book-card, a[href*="/books/"]');
    if (bookCards.length > 0) {
      await bookCards[0].click();
      await waitForPageReady(page);
      await waitForThumbnails(page);
      await captureScreenshot(page, "15-book-detail");
    }
  }
}

/**
 * Create a library with the given configuration
 */
async function createLibrary(page: Page, libraryConfig: LibraryConfig): Promise<void> {
  const { name, path, readingDirection, formats, seriesStrategy, excludedPatterns, scanImmediately, cronSchedule } = libraryConfig;
  const nameLower = name.toLowerCase();

  // Click the "Add Library" button in sidebar
  const addButton = await page.$('button[aria-label="Add library"], button:has(svg.tabler-icon-plus)');

  if (!addButton) {
    // Try clicking the sidebar libraries section first to reveal the button
    const librariesSection = await page.$('text=Libraries');
    if (librariesSection) {
      await librariesSection.click();
      await page.waitForTimeout(300);
    }
  }

  // Try multiple selectors for the add button
  const addButtonSelector = 'button[aria-label="Add library"], [data-testid="add-library"], nav button:has(svg)';
  await page.click(addButtonSelector);

  // Wait for modal to open
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(500);

  // === GENERAL TAB ===
  // Fill in library name
  await page.fill('input[placeholder="Enter library name"]', name);

  // Fill in path directly (since we're in Docker with known paths)
  await page.fill('input[placeholder="Select a path..."]', path);

  // Set reading direction using Mantine Select
  // Find the select input by looking for the label, then clicking the input inside the wrapper
  const readingDirectionWrapper = await page.$('label:has-text("Default Reading Direction")');
  if (readingDirectionWrapper) {
    // Click on the parent wrapper's input element
    const selectInput = await page.$('.mantine-Select-input');
    if (selectInput) {
      await selectInput.click();
      await page.waitForTimeout(300);

      // Select the appropriate option from the dropdown
      if (readingDirection === "rtl") {
        await page.click('[role="option"]:has-text("Right to Left")');
      } else {
        await page.click('[role="option"]:has-text("Left to Right")');
      }
      await page.waitForTimeout(200);
    }
  }

  // Capture filled General tab
  await captureScreenshot(page, `07-add-library-general-${nameLower}`);

  // === STRATEGY TAB ===
  const strategyTab = await page.$('button[role="tab"]:has-text("Strategy")');
  if (strategyTab) {
    await strategyTab.click();
    await page.waitForTimeout(300);

    // Select series strategy based on library type using Mantine Select
    if (seriesStrategy === "calibre_author") {
      // For books: select Calibre Library strategy
      // Find the Series Detection Strategy select by its label
      const seriesStrategySelect = await page.locator('label:has-text("Series Detection Strategy")').locator('..').locator('.mantine-Select-input').first();
      if (await seriesStrategySelect.count() > 0) {
        await seriesStrategySelect.click();
        await page.waitForTimeout(300);
        await page.click('[role="option"]:has-text("Calibre Library")');
        await page.waitForTimeout(500);

        // After selecting Calibre, a "Series Grouping Mode" dropdown appears (default is "From Metadata")
        // Select "By Author" option
        // Wait for the new dropdown to render
        await page.waitForTimeout(500);

        // Find the Series Grouping Mode select by its label (more robust than index-based selection)
        const groupingModeSelect = await page.locator('label:has-text("Series Grouping Mode")').locator('..').locator('.mantine-Select-input').first();
        if (await groupingModeSelect.count() > 0) {
          // Scroll into view and click
          await groupingModeSelect.scrollIntoViewIfNeeded();
          await page.waitForTimeout(200);
          await groupingModeSelect.click();
          await page.waitForTimeout(500);

          // Wait for dropdown options to be visible, then click "By Author"
          await page.waitForSelector('[role="option"]:has-text("By Author")', { state: "visible", timeout: 5000 });
          await page.click('[role="option"]:has-text("By Author")');
          await page.waitForTimeout(200);
        }
      }
    }
    // Default is series_volume, no change needed for comics/manga

    // Capture Strategy tab
    await captureScreenshot(page, `08-add-library-strategy-${nameLower}`);
  }

  // === FORMATS TAB ===
  const formatsTab = await page.$('button[role="tab"]:has-text("Formats")');
  if (formatsTab) {
    await formatsTab.click();
    await page.waitForTimeout(300);

    // The MultiSelect starts with all formats selected by default: CBZ, CBR, EPUB, PDF
    // We need to REMOVE the formats we don't want (deselect them)
    const allFormats = ["CBZ", "CBR", "EPUB", "PDF"];
    const formatsToRemove = allFormats.filter((f) => !formats.includes(f));

    // Remove unwanted formats by clicking their "x" button in the MultiSelect pills
    for (const format of formatsToRemove) {
      // Find the pill with this format and click its remove button
      const pill = await page.$(`[data-value="${format}"] button, .mantine-MultiSelect-pill:has-text("${format}") button, .mantine-Pill:has-text("${format}") button`);
      if (pill) {
        await pill.click();
        await page.waitForTimeout(100);
      }
    }

    // Set excluded patterns if provided
    if (excludedPatterns) {
      const excludedInput = await page.$('textarea[placeholder*="DS_Store"]');
      if (excludedInput) {
        await excludedInput.fill(excludedPatterns);
        await page.waitForTimeout(200);
      }
    }

    // Capture Formats tab
    await captureScreenshot(page, `09-add-library-formats-${nameLower}`);
  }

  // === SCANNING TAB ===
  const scanningTab = await page.$('button[role="tab"]:has-text("Scanning")');
  if (scanningTab) {
    await scanningTab.click();
    await page.waitForTimeout(300);

    // Set scan strategy (auto if cronSchedule is provided) using Mantine Select
    if (cronSchedule) {
      // Find the Scan Strategy select by its label
      const scanStrategySelect = await page.locator('label:has-text("Scan Strategy")').locator('..').locator('.mantine-Select-input').first();
      if (await scanStrategySelect.count() > 0) {
        await scanStrategySelect.click();
        await page.waitForTimeout(300);
        await page.click('[role="option"]:has-text("Automatic")');
        await page.waitForTimeout(300);

        // Fill in cron schedule - the CronInput component has a text input
        const cronInput = await page.$('input[placeholder="0 0 * * *"]');
        if (cronInput) {
          await cronInput.fill(cronSchedule);
          await page.waitForTimeout(200);
        }
      }
    }

    // Check "Scan immediately after creation" if needed
    if (scanImmediately) {
      // Mantine Checkbox - click on the checkbox input or its label
      const scanImmediatelyCheckbox = await page.$('label:has-text("Scan immediately after creation")');
      if (scanImmediatelyCheckbox) {
        await scanImmediatelyCheckbox.click();
        await page.waitForTimeout(200);
      }
    }

    // Capture Scanning tab
    await captureScreenshot(page, `10-add-library-scanning-${nameLower}`);
  }

  // Click Create Library button
  await page.click('button:has-text("Create Library")');

  // Wait for modal to close
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "hidden", timeout: 10000 });
  await waitForPageReady(page);

  console.log(`    ✓ Library "${name}" created`);
}
