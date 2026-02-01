import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Plugins scenario
 * Captures plugin creation, series detail plugin actions, and library auto-match
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  🔌 Capturing plugins screenshots...");

  // Part 1: Create a plugin in settings
  await createPluginScreenshots(page);

  // Part 2: Series detail page - plugin dropdown and metadata flow
  await seriesDetailPluginScreenshots(page);

  // Part 3: Library sidebar - auto-match
  await libraryAutoMatchScreenshots(page);

  // Part 4: Plugin Metrics
  await pluginMetricsScreenshots(page);
}

/**
 * Create a plugin through the settings UI
 */
async function createPluginScreenshots(page: Page): Promise<void> {
  console.log("    📷 Plugin Settings - Create Plugin Flow");

  // Navigate to plugins settings
  await page.goto("/settings/plugins");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Capture initial plugins page (may be empty or have existing plugins)
  await captureScreenshot(page, "plugins/settings-plugins");

  // Click "Add Plugin" button - Mantine Button component
  const addPluginButton = page.locator('button:has-text("Add Plugin")').first();
  if ((await addPluginButton.count()) === 0) {
    console.log("      ⚠️  Add Plugin button not found");
    return;
  }

  await addPluginButton.click();
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(500);

  // === GENERAL TAB (First Tab) ===
  // Fill in plugin details
  await page.fill('input[placeholder="mangabaka"]', "echo");
  await page.waitForTimeout(100);

  await page.fill('input[placeholder="MangaBaka"]', "Echo");
  await page.waitForTimeout(100);

  // Fill description
  const descriptionTextarea = await page.$('textarea[placeholder*="MangaBaka"]');
  if (descriptionTextarea) {
    await descriptionTextarea.fill("Echo plugin");
    await page.waitForTimeout(100);
  }

  // Enable the plugin immediately - click the Switch label/track, not the hidden input
  const enableSwitch = page.locator('label:has-text("Enable immediately")').first();
  if ((await enableSwitch.count()) > 0) {
    await enableSwitch.click();
    await page.waitForTimeout(100);
  }

  // Capture General tab
  await captureScreenshot(page, "plugins/create-general");

  // === EXECUTION TAB (Second Tab) ===
  const executionTab = await page.$('button[role="tab"]:has-text("Execution")');
  if (executionTab) {
    await executionTab.click();
    await page.waitForTimeout(500);

    // Fill command - find the command input by its placeholder
    const commandInput = page.locator('input[placeholder="node"]').first();
    await commandInput.fill("npx");
    await page.waitForTimeout(100);

    // Fill arguments - find by placeholder that contains "mangabaka" (the args textarea)
    const argsTextarea = page.locator('textarea[placeholder*="mangabaka"]').first();
    await argsTextarea.fill("-y\n@ashdev/codex-plugin-metadata-echo@1.0.0");
    await page.waitForTimeout(100);

    // Capture Execution tab with npx command
    await captureScreenshot(page, "plugins/create-execution");

    // Now change command to node and arguments to local path (per instructions)
    await commandInput.fill("node");
    await page.waitForTimeout(100);

    await argsTextarea.fill("/opt/codex/plugins/metadata-echo/dist/index.js");
    await page.waitForTimeout(100);
  } else {
    console.log("      ⚠️  Execution tab not found");
  }

  // === PERMISSIONS TAB (Third Tab) ===
  const permissionsTab = await page.$('button[role="tab"]:has-text("Permissions")');
  if (permissionsTab) {
    await permissionsTab.click();
    await page.waitForTimeout(300);

    // Select permissions using MultiSelect
    // Click on Permissions MultiSelect
    const permissionsSelect = await page.locator('label:has-text("Permissions")').locator('..').locator('.mantine-MultiSelect-input').first();
    if (await permissionsSelect.count() > 0) {
      await permissionsSelect.click();
      await page.waitForTimeout(300);

      // Select "Read metadata"
      const readOption = await page.$('[role="option"]:has-text("Read")');
      if (readOption) {
        await readOption.click();
        await page.waitForTimeout(200);
      }

      // Click again to select more
      await permissionsSelect.click();
      await page.waitForTimeout(300);

      // Select "Write All metadata"
      const writeAllOption = await page.$('[role="option"]:has-text("Write All")');
      if (writeAllOption) {
        await writeAllOption.click();
        await page.waitForTimeout(200);
      }

      // Click outside to close dropdown
      await page.keyboard.press("Escape");
      await page.waitForTimeout(200);
    }

    // Select scopes using MultiSelect
    const scopesSelect = await page.locator('label:has-text("Scopes")').locator('..').locator('.mantine-MultiSelect-input').first();
    if (await scopesSelect.count() > 0) {
      await scopesSelect.click();
      await page.waitForTimeout(300);

      // Select "series:detail"
      const seriesDetailOption = await page.$('[role="option"]:has-text("Series Detail")');
      if (seriesDetailOption) {
        await seriesDetailOption.click();
        await page.waitForTimeout(200);
      }

      // Click again for more
      await scopesSelect.click();
      await page.waitForTimeout(300);

      // Select "library:detail"
      const libraryDetailOption = await page.$('[role="option"]:has-text("Library Detail")');
      if (libraryDetailOption) {
        await libraryDetailOption.click();
        await page.waitForTimeout(200);
      }

      // Close dropdown
      await page.keyboard.press("Escape");
      await page.waitForTimeout(200);
    }

    // Capture Permissions tab
    await captureScreenshot(page, "plugins/create-permissions");
  }

  // === CREDENTIALS TAB (Fourth Tab) ===
  const credentialsTab = await page.$('button[role="tab"]:has-text("Credentials")');
  if (credentialsTab) {
    await credentialsTab.click();
    await page.waitForTimeout(300);

    // Fill credentials JSON with fake data
    const credentialsTextarea = await page.$('textarea[placeholder*="api_key"]');
    if (credentialsTextarea) {
      await credentialsTextarea.fill('{\n  "api_key": "demo-key-12345",\n  "secret": "demo-secret"\n}');
      await page.waitForTimeout(100);
    }

    // Capture Credentials tab
    await captureScreenshot(page, "plugins/create-credentials");
  }

  // Create the plugin - use text selector for reliability
  const createButton = page.locator('button:has-text("Create Plugin")').first();
  if ((await createButton.count()) > 0) {
    await createButton.click();
    await page.waitForTimeout(2000); // Give more time for API call

    // Check if modal is still open (indicates validation error)
    const modalStillOpen = await page.$('[role="dialog"], .mantine-Modal-content');
    if (modalStillOpen) {
      console.log("      ⚠️  Modal still open - plugin creation may have failed");
      // Take a screenshot to see the error state
      await captureScreenshot(page, "plugins/create-error");
      // Try to close the modal
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
    }

    // Wait for modal to close (API call may take time)
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "hidden", timeout: 15000 }).catch(() => {});

    // Wait for notification to appear and disappear
    await page.waitForTimeout(3000);
    await waitForPageReady(page);
  }

  // Click the test button to verify plugin connection
  const testButton = page.locator('button:has(svg.tabler-icon-player-play)').first();
  if ((await testButton.count()) > 0) {
    await testButton.click();
    // Wait for test to complete and notification to appear
    await page.waitForTimeout(2000);
  }

  // Capture plugins list with new plugin (after test)
  await captureScreenshot(page, "plugins/settings-plugins-with-echo");

  // === SEARCH CONFIG MODAL (Separate from creation, for metadata providers) ===
  // Click the gear icon to open Search Configuration modal
  const searchConfigButton = page.locator('button:has(svg.tabler-icon-settings)').first();
  if ((await searchConfigButton.count()) > 0) {
    await searchConfigButton.click();
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
    await page.waitForTimeout(500);

    // Capture Template tab (first tab, shown by default)
    await captureScreenshot(page, "plugins/search-config-template");

    // Click Preprocessing tab
    const preprocessingTab = await page.$('button[role="tab"]:has-text("Preprocessing")');
    if (preprocessingTab) {
      await preprocessingTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/search-config-preprocessing");
    }

    // Click Conditions tab
    const conditionsTab = await page.$('button[role="tab"]:has-text("Conditions")');
    if (conditionsTab) {
      await conditionsTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/search-config-conditions");
    }

    // Close the modal
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
  } else {
    console.log("      ⚠️  Search Config button not found (plugin may not be a metadata provider)");
  }

  console.log("      ✓ Plugin creation screenshots captured");
}

/**
 * Series detail page - plugin dropdown and metadata flow
 */
async function seriesDetailPluginScreenshots(page: Page): Promise<void> {
  console.log("    📷 Series Detail - Plugin Actions");

  // Navigate to the manga library's series view
  // First, find the manga library by clicking its link in the sidebar
  const mangaLibraryLink = page.locator('nav a[href*="/libraries/"]:has-text("Manga")').first();
  if ((await mangaLibraryLink.count()) > 0) {
    await mangaLibraryLink.click();
  } else {
    // Fallback to all libraries if manga not found
    await page.goto("/libraries/all/series");
  }
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Click on first series card
  const seriesCard = await page.$('[data-testid="series-card"], .series-card, a[href*="/series/"]');
  if (!seriesCard) {
    console.log("      ⚠️  No series found, skipping series detail screenshots");
    return;
  }

  await seriesCard.click();
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Find and click the actions menu button (IconDotsVertical - three vertical dots)
  // Target the menu button in the series header Grid, not the sidebar
  // Use a more specific selector: find the ActionIcon with size="lg" that contains the dots icon
  // The sidebar uses smaller buttons without the "lg" size variant
  const actionsMenu = page.locator('.mantine-Grid-root button:has(svg.tabler-icon-dots-vertical)').first();
  if ((await actionsMenu.count()) === 0) {
    console.log("      ⚠️  Actions menu not found on series detail");
    return;
  }

  // Retry mechanism: open menu and wait for plugin to appear (handles TTL/cache delays)
  const maxRetries = 10;
  const retryDelay = 5000; // 5 seconds between retries
  let fetchMetadataEcho: Awaited<ReturnType<typeof page.$>> = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    // Reload the page on retry attempts to refresh plugin cache
    if (attempt > 1) {
      console.log(`      🔄 Reloading page (attempt ${attempt}/${maxRetries})...`);
      await page.reload();
      await waitForPageReady(page);
      await page.waitForTimeout(500);
    }

    await actionsMenu.click();
    await page.waitForTimeout(500);

    // Check if Echo plugin is in the menu
    fetchMetadataEcho = await page.$('[role="menuitem"]:has-text("Echo"), .mantine-Menu-item:has-text("Echo")');

    if (fetchMetadataEcho) {
      console.log(`      ✓ Echo plugin found on attempt ${attempt}`);
      break;
    }

    // Plugin not found, close menu and wait before retry
    console.log(`      ⏳ Echo plugin not found (attempt ${attempt}/${maxRetries}), waiting...`);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(retryDelay);
  }

  if (!fetchMetadataEcho) {
    console.log("      ⚠️  Echo plugin not found in menu after all retries");
    return;
  }

  // Capture dropdown showing plugin options
  await captureScreenshot(page, "plugins/series-detail-plugin-dropdown");

  // Click on "Echo" plugin in "Fetch Metadata" section

  await fetchMetadataEcho.click();
  await page.waitForTimeout(500);

  // Wait for search modal to open
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(1000);

  // Capture search results
  await captureScreenshot(page, "plugins/search-results");

  // Click on first search result (div with cursor: pointer inside the results stack)
  const searchResult = await page.$('.mantine-Modal-content .mantine-Stack-root .mantine-Stack-root > div[style*="cursor: pointer"]');
  if (searchResult) {
    await searchResult.click();
    await page.waitForTimeout(500);

    // Wait for preview to load
    await waitForPageReady(page);
    await page.waitForTimeout(500);

    // Capture metadata preview
    await captureScreenshot(page, "plugins/metadata-preview");

    // Click Apply button
    const applyButton = await page.$('button:has-text("Apply")');
    if (applyButton) {
      await applyButton.click();
      await page.waitForTimeout(1000);

      // Capture success state
      await captureScreenshot(page, "plugins/apply-success");

      // Close the success modal (X button in header)
      const closeButton = await page.$('.mantine-Modal-close');
      if (closeButton) {
        await closeButton.click();
        await page.waitForTimeout(500);
      } else {
        await page.keyboard.press("Escape");
        await page.waitForTimeout(300);
      }
    } else {
      console.log("      ⚠️  No apply button found");
    }
  } else {
    console.log("      ⚠️  No search result found");
  }

  // Wait for modal to close
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "hidden", timeout: 5000 }).catch(() => {});
  await waitForPageReady(page);
  await page.waitForTimeout(300);

  // Capture series detail page after metadata applied
  await captureScreenshot(page, "plugins/series-detail-after-plugin");

  console.log("      ✓ Series detail plugin screenshots captured");
}

/**
 * Library sidebar - auto-match feature
 */
async function libraryAutoMatchScreenshots(page: Page): Promise<void> {
  console.log("    📷 Library Sidebar - Auto Match");

  // Navigate to home to access the sidebar
  await page.goto("/");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Find the Manga library's menu button in the sidebar
  // Look for the NavLink containing "Manga" text and find its menu button
  const mangaNavLink = page.locator('nav .mantine-NavLink-root:has-text("Manga")').first();
  if ((await mangaNavLink.count()) === 0) {
    console.log("      ⚠️  Manga library not found in sidebar");
    return;
  }

  // Click the menu button within the Manga library NavLink
  const mangaMenuButton = mangaNavLink.locator('button:has(svg.tabler-icon-dots-vertical)');
  if ((await mangaMenuButton.count()) === 0) {
    console.log("      ⚠️  Manga library menu button not found");
    return;
  }

  await mangaMenuButton.click();
  await page.waitForTimeout(500);

  // Capture library dropdown showing plugin auto-match options
  await captureScreenshot(page, "plugins/library-sidebar-plugin-dropdown");

  // Click on "Echo" plugin under "Auto Match All Series" section
  // The menu item shows the plugin's displayName
  const autoMatchEcho = page.locator('[role="menuitem"]:has-text("Echo")').first();
  if ((await autoMatchEcho.count()) > 0) {
    await autoMatchEcho.click();
    await page.waitForTimeout(2000);

    // Capture success notification
    await captureScreenshot(page, "plugins/library-auto-match-success");
  } else {
    console.log("      ⚠️  Auto Match Echo option not found in menu");
    // Close menu
    await page.keyboard.press("Escape");
  }

  console.log("      ✓ Library auto-match screenshots captured");
}

/**
 * Plugin metrics tab in settings
 * Shows plugin performance statistics after plugin usage
 */
async function pluginMetricsScreenshots(page: Page): Promise<void> {
  console.log("    📷 Plugin Metrics Tab");

  // Navigate to metrics settings
  await page.goto("/settings/metrics");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Click on Plugins tab
  const pluginsTab = page.locator('[role="tab"]:has-text("Plugins")').first();
  if ((await pluginsTab.count()) === 0) {
    console.log("      ⚠️  Plugins tab not found in metrics");
    return;
  }

  await pluginsTab.click();
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Capture the plugin metrics overview
  await captureScreenshot(page, "settings/metrics-plugins-overview");

  // Try to expand a plugin row to show details
  // Target the Plugins tab panel specifically using aria attributes
  const pluginRow = page.locator('[role="tabpanel"][aria-labelledby*="plugins" i] .mantine-Table-tbody .mantine-Table-tr').first();
  if ((await pluginRow.count()) > 0 && (await pluginRow.isVisible())) {
    // Scroll the row into view before clicking
    await pluginRow.scrollIntoViewIfNeeded();
    await page.waitForTimeout(200);

    await pluginRow.click();
    await page.waitForTimeout(300);

    // Capture with expanded details
    await captureScreenshot(page, "settings/metrics-plugins-expanded");
  } else {
    console.log("      ⚠️  No plugin rows found in metrics table (empty state or not visible)");
  }

  console.log("      ✓ Plugin metrics screenshots captured");
}

