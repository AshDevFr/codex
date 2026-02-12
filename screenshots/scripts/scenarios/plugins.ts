import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Plugins scenario
 * Captures plugin store, plugin installation, series detail plugin actions,
 * library auto-match, user integrations, and plugin metrics.
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  🔌 Capturing plugins screenshots...");

  // Part 1: Install plugins from the Official Plugin Store
  await pluginStoreScreenshots(page);

  // Part 2: Series detail page - plugin dropdown and metadata flow
  await seriesDetailPluginScreenshots(page);

  // Part 3: Library sidebar - auto-match
  await libraryAutoMatchScreenshots(page);

  // Part 4: User Integrations page
  await userIntegrationsScreenshots(page);

  // Part 5: Plugin Metrics
  await pluginMetricsScreenshots(page);
}

/**
 * Install plugins from the Official Plugin Store.
 * Adds echo, sync-anilist, and recommendations-anilist plugins via the store carousel.
 */
async function pluginStoreScreenshots(page: Page): Promise<void> {
  console.log("    📷 Plugin Store - Install Plugins");

  // Navigate to plugins settings
  await page.goto("/settings/plugins");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Capture initial plugins page (empty state)
  await captureScreenshot(page, "plugins/settings-plugins-empty");

  // === OPEN THE OFFICIAL PLUGINS STORE ===
  // The OfficialPlugins component is a collapsible Card with "Official Plugins" header
  const officialPluginsHeader = page.locator('button:has-text("Official Plugins")').first();
  if ((await officialPluginsHeader.count()) === 0) {
    console.log("      ⚠️  Official Plugins section not found");
    return;
  }

  await officialPluginsHeader.click();
  await page.waitForTimeout(500);

  // Capture the plugin store carousel (all 5 cards visible)
  await captureScreenshot(page, "plugins/store-carousel");

  // === ADD ECHO METADATA PLUGIN ===
  await addPluginFromStore(page, "Echo Metadata", "plugins/store-add-echo");

  // === ADD ANILIST SYNC PLUGIN ===
  await addPluginFromStore(page, "AniList Sync", "plugins/store-add-sync");

  // === ADD ANILIST RECOMMENDATIONS PLUGIN ===
  await addPluginFromStore(page, "AniList Recommendations", "plugins/store-add-recommendations");

  // Navigate back to plugins page to see all installed plugins
  await page.goto("/settings/plugins");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Capture plugins list with all installed plugins
  await captureScreenshot(page, "plugins/settings-plugins-installed");

  // === TEST ALL PLUGINS ===
  // Test each plugin (play icon) to populate their manifests/capabilities
  const testButtons = page.locator('button:has(svg.tabler-icon-player-play)');
  const testCount = await testButtons.count();
  for (let i = 0; i < testCount; i++) {
    await testButtons.nth(i).click();
    await page.waitForTimeout(3000); // Wait for test to complete
  }

  // Capture plugins list after tests
  await captureScreenshot(page, "plugins/settings-plugins-after-test");

  // === CONFIGURE ECHO PLUGIN PERMISSIONS & SCOPES ===
  // After testing, the plugin has a manifest. Open the Config Modal (gear icon)
  // to set permissions and scopes so it appears in series detail and library menus.
  await configureEchoPlugin(page);

  // === EXPANDED PLUGIN DETAILS ===
  // Navigate back to plugins page (config modal may have changed state)
  await page.goto("/settings/plugins");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Click the expand chevron on the first plugin row to show details
  const expandButton = page.locator('.mantine-Table-tbody button:has(svg.tabler-icon-chevron-right)').first();
  if ((await expandButton.count()) > 0) {
    await expandButton.click();
    await page.waitForTimeout(500);

    // Capture expanded plugin details
    await captureScreenshot(page, "plugins/settings-plugin-expanded");

    // Collapse it back
    const collapseButton = page.locator('.mantine-Table-tbody button:has(svg.tabler-icon-chevron-down)').first();
    if ((await collapseButton.count()) > 0) {
      await collapseButton.click();
      await page.waitForTimeout(300);
    }
  } else {
    console.log("      ⚠️  Expand button not found on plugin row");
  }

  // === SEARCH CONFIG MODAL ===
  // Click the gear icon to open Plugin Configuration modal (for metadata plugins)
  // This is the same gear icon that opens the config modal with all tabs
  const searchConfigButton = page.locator('button:has(svg.tabler-icon-settings)').first();
  if ((await searchConfigButton.count()) > 0) {
    await searchConfigButton.click();
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
    await page.waitForTimeout(500);

    // For metadata providers, default tab is "General" - capture it
    await captureScreenshot(page, "plugins/config-modal-general");

    // Click Permissions tab
    const permTab = await page.$('button[role="tab"]:has-text("Permissions")');
    if (permTab) {
      await permTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/config-modal-permissions");
    }

    // Click Template tab
    const templateTab = await page.$('button[role="tab"]:has-text("Template")');
    if (templateTab) {
      await templateTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/config-modal-template");
    }

    // Click Preprocessing tab
    const preprocessingTab = await page.$('button[role="tab"]:has-text("Preprocessing")');
    if (preprocessingTab) {
      await preprocessingTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/config-modal-preprocessing");
    }

    // Click Conditions tab
    const conditionsTab = await page.$('button[role="tab"]:has-text("Conditions")');
    if (conditionsTab) {
      await conditionsTab.click();
      await page.waitForTimeout(300);
      await captureScreenshot(page, "plugins/config-modal-conditions");
    }

    // Close the modal
    await page.keyboard.press("Escape");
    await page.waitForTimeout(300);
  } else {
    console.log("      ⚠️  Search Config button not found (plugin may not be a metadata provider)");
  }

  console.log("      ✓ Plugin store screenshots captured");
}

/**
 * Add a plugin from the Official Plugin Store.
 * The store uses 3D flip cards with CSS hover animations. Since the back face
 * (with the "Add" button) uses backface-visibility: hidden and CSS :hover transforms,
 * we use JavaScript to programmatically click the Add button.
 *
 * @param page - Playwright page
 * @param displayName - The plugin's display name shown on the card front (e.g., "Echo Metadata")
 * @param screenshotPrefix - Screenshot name prefix for the pre-filled modal
 */
async function addPluginFromStore(page: Page, displayName: string, screenshotPrefix: string): Promise<void> {
  console.log(`      📦 Adding "${displayName}" from store...`);

  // Navigate to plugins page for clean state
  await page.goto("/settings/plugins");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Expand the Official Plugins carousel
  const officialPluginsHeader = page.locator('button:has-text("Official Plugins")').first();
  if ((await officialPluginsHeader.count()) > 0) {
    await officialPluginsHeader.click();
    await page.waitForTimeout(800); // Wait for Collapse animation
  }

  // Use JavaScript to find and click the "Add" button for this plugin.
  // The flip cards use CSS modules (hashed class names) and CSS :hover for 3D flip,
  // making direct Playwright interaction unreliable. Instead, we find the button via
  // text content matching in the DOM.
  const clicked = await page.evaluate((name: string) => {
    // Find all buttons with "Add" text that are inside the official plugins section
    const buttons = document.querySelectorAll("button");
    for (const btn of buttons) {
      if (btn.textContent?.trim() !== "Add") continue;
      // Walk up to find a container that includes the plugin display name
      const card = btn.closest("div[class*='flipCard'], div[style*='perspective']");
      if (!card) {
        // Try a broader parent search - look for a parent div that contains both
        // the button and the display name text
        let parent: HTMLElement | null = btn.parentElement;
        for (let i = 0; i < 10 && parent; i++) {
          if (parent.textContent?.includes(name)) {
            btn.click();
            return "clicked";
          }
          parent = parent.parentElement;
        }
        continue;
      }
      if (card.textContent?.includes(name)) {
        btn.click();
        return "clicked";
      }
    }
    // Check if already installed
    for (const btn of buttons) {
      if (btn.textContent?.trim() !== "Added") continue;
      let parent: HTMLElement | null = btn.parentElement;
      for (let i = 0; i < 10 && parent; i++) {
        if (parent.textContent?.includes(name)) {
          return "already_installed";
        }
        parent = parent.parentElement;
      }
    }
    return "not_found";
  }, displayName);

  if (clicked === "already_installed") {
    console.log(`      ✓ "${displayName}" is already installed`);
    return;
  }
  if (clicked === "not_found") {
    console.log(`      ⚠️  Add button not found for "${displayName}"`);
    return;
  }

  await page.waitForTimeout(500);

  // Wait for the pre-filled create modal to open
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(500);

  // Capture the pre-filled modal (General tab is shown by default)
  await captureScreenshot(page, `${screenshotPrefix}-general`);

  // Capture the Execution tab (pre-filled with npx command)
  const executionTab = await page.$('button[role="tab"]:has-text("Execution")');
  if (executionTab) {
    await executionTab.click();
    await page.waitForTimeout(300);
    await captureScreenshot(page, `${screenshotPrefix}-execution`);
  }

  // Switch back to General tab before submitting
  const generalTab = await page.$('button[role="tab"]:has-text("General")');
  if (generalTab) {
    await generalTab.click();
    await page.waitForTimeout(200);
  }

  // Submit the form - click "Create Plugin" button
  const createButton = page.locator('button:has-text("Create Plugin")').first();
  if ((await createButton.count()) > 0) {
    await createButton.click();
    await page.waitForTimeout(2000);

    // Check if modal is still open (validation error)
    const modalStillOpen = await page.$('[role="dialog"], .mantine-Modal-content');
    if (modalStillOpen) {
      console.log(`      ⚠️  Modal still open - "${displayName}" creation may have failed`);
      await captureScreenshot(page, `${screenshotPrefix}-error`);
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
    }

    // Wait for modal to close
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "hidden", timeout: 15000 }).catch(() => {});
    await page.waitForTimeout(1000);
    await waitForPageReady(page);
  }

  console.log(`      ✓ "${displayName}" added from store`);
}

/**
 * Configure the Echo Metadata plugin's permissions and scopes via the Config Modal.
 * This must be done after testing (so the manifest is populated) for the plugin
 * to appear in series detail and library context menus.
 */
async function configureEchoPlugin(page: Page): Promise<void> {
  console.log("      ⚙️  Configuring Echo plugin permissions & scopes...");

  // We should be on the plugins settings page already
  // Find the gear icon (Configure Plugin) for the Echo plugin row
  // The table rows contain plugin display names; find the row with "Echo" and click its gear icon
  const echoRow = page.locator('.mantine-Table-tbody .mantine-Table-tr:has-text("Echo")').first();
  if ((await echoRow.count()) === 0) {
    console.log("      ⚠️  Echo plugin row not found");
    return;
  }

  const configButton = echoRow.locator('button:has(svg.tabler-icon-settings)');
  if ((await configButton.count()) === 0) {
    console.log("      ⚠️  Config button not found on Echo plugin row");
    return;
  }

  await configButton.click();
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(500);

  // Navigate to Permissions tab
  const permissionsTab = page.locator('button[role="tab"]:has-text("Permissions")');
  if ((await permissionsTab.count()) > 0) {
    await permissionsTab.click();
    await page.waitForTimeout(300);
  }

  // === SELECT PERMISSIONS ===
  // Click the Permissions MultiSelect input
  const permissionsSelect = page.locator('label:has-text("Permissions")').locator('..').locator('.mantine-MultiSelect-input').first();
  if ((await permissionsSelect.count()) > 0) {
    await permissionsSelect.click();
    await page.waitForTimeout(300);

    // Select "Read metadata"
    const readOption = page.locator('[role="option"]:has-text("Read")').first();
    if ((await readOption.count()) > 0) {
      await readOption.click();
      await page.waitForTimeout(200);
    }

    // Click again to open dropdown for more selections
    await permissionsSelect.click();
    await page.waitForTimeout(300);

    // Select "Write All metadata"
    const writeAllOption = page.locator('[role="option"]:has-text("Write All")').first();
    if ((await writeAllOption.count()) > 0) {
      await writeAllOption.click();
      await page.waitForTimeout(200);
    }

    // Close dropdown
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }

  // === SELECT SCOPES ===
  const scopesSelect = page.locator('label:has-text("Scopes")').locator('..').locator('.mantine-MultiSelect-input').first();
  if ((await scopesSelect.count()) > 0) {
    await scopesSelect.click();
    await page.waitForTimeout(300);

    // Select "Series Detail"
    const seriesDetailOption = page.locator('[role="option"]:has-text("Series Detail")').first();
    if ((await seriesDetailOption.count()) > 0) {
      await seriesDetailOption.click();
      await page.waitForTimeout(200);
    }

    // Click again
    await scopesSelect.click();
    await page.waitForTimeout(300);

    // Select "Library Detail"
    const libraryDetailOption = page.locator('[role="option"]:has-text("Library Detail")').first();
    if ((await libraryDetailOption.count()) > 0) {
      await libraryDetailOption.click();
      await page.waitForTimeout(200);
    }

    // Close dropdown
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }

  // Capture the configured Permissions tab (with selections made)
  await captureScreenshot(page, "plugins/config-modal-permissions-filled");

  // Save changes
  const saveButton = page.locator('button:has-text("Save Changes")').first();
  if ((await saveButton.count()) > 0) {
    await saveButton.click();
    await page.waitForTimeout(2000);

    // Wait for modal to close
    await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "hidden", timeout: 10000 }).catch(() => {});
    await waitForPageReady(page);
  }

  console.log("      ✓ Echo plugin configured with permissions & scopes");
}

/**
 * Series detail page - plugin dropdown and metadata flow
 */
async function seriesDetailPluginScreenshots(page: Page): Promise<void> {
  console.log("    📷 Series Detail - Plugin Actions");

  // Navigate to the manga library's series view
  const mangaLibraryLink = page.locator('nav a[href*="/libraries/"]:has-text("Manga")').first();
  if ((await mangaLibraryLink.count()) > 0) {
    await mangaLibraryLink.click();
  } else {
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

  // Find and click the actions menu button (three vertical dots in the series header)
  const actionsMenu = page.locator('.mantine-Grid-root button:has(svg.tabler-icon-dots-vertical)').first();
  if ((await actionsMenu.count()) === 0) {
    console.log("      ⚠️  Actions menu not found on series detail");
    return;
  }

  // Retry mechanism: open menu and wait for Echo plugin to appear (handles TTL/cache delays)
  const maxRetries = 10;
  const retryDelay = 5000;
  let fetchMetadataEcho: Awaited<ReturnType<typeof page.$>> = null;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    if (attempt > 1) {
      console.log(`      🔄 Reloading page (attempt ${attempt}/${maxRetries})...`);
      await page.reload();
      await waitForPageReady(page);
      await page.waitForTimeout(500);
    }

    await actionsMenu.click();
    await page.waitForTimeout(500);

    fetchMetadataEcho = await page.$('[role="menuitem"]:has-text("Echo"), .mantine-Menu-item:has-text("Echo")');

    if (fetchMetadataEcho) {
      console.log(`      ✓ Echo plugin found on attempt ${attempt}`);
      break;
    }

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

  // Click on "Echo" plugin
  await fetchMetadataEcho.click();
  await page.waitForTimeout(500);

  // Wait for search modal to open
  await page.waitForSelector('[role="dialog"], .mantine-Modal-content', { state: "visible", timeout: 5000 });
  await page.waitForTimeout(1000);

  // Capture search results
  await captureScreenshot(page, "plugins/search-results");

  // Click on first search result
  const searchResult = await page.$('.mantine-Modal-content .mantine-Stack-root .mantine-Stack-root > div[style*="cursor: pointer"]');
  if (searchResult) {
    await searchResult.click();
    await page.waitForTimeout(500);
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

      // Close the success modal
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

  await page.goto("/");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Find the Manga library's menu button in the sidebar
  const mangaNavLink = page.locator('nav .mantine-NavLink-root:has-text("Manga")').first();
  if ((await mangaNavLink.count()) === 0) {
    console.log("      ⚠️  Manga library not found in sidebar");
    return;
  }

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
  const autoMatchEcho = page.locator('[role="menuitem"]:has-text("Echo")').first();
  if ((await autoMatchEcho.count()) > 0) {
    await autoMatchEcho.click();
    await page.waitForTimeout(2000);

    // Capture success notification
    await captureScreenshot(page, "plugins/library-auto-match-success");
  } else {
    console.log("      ⚠️  Auto Match Echo option not found in menu");
    await page.keyboard.press("Escape");
  }

  console.log("      ✓ Library auto-match screenshots captured");
}

/**
 * User Integrations page - shows available and enabled plugin integrations
 * This is the user-facing view at /settings/integrations where users can
 * enable/disable sync and recommendation plugins for their account.
 */
async function userIntegrationsScreenshots(page: Page): Promise<void> {
  console.log("    📷 User Integrations Page");

  // Navigate to the integrations page
  await page.goto("/settings/integrations");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Capture the integrations page showing available plugins
  // Since sync and recommendations plugins were added, they should appear here
  await captureScreenshot(page, "plugins/user-integrations");

  // Try to enable the sync plugin if it appears in "Available" section
  const enableButtons = page.locator('button:has-text("Enable")');
  const enableCount = await enableButtons.count();

  if (enableCount > 0) {
    // Enable the first available integration (likely AniList Sync)
    await enableButtons.first().click();
    await page.waitForTimeout(1500);
    await waitForPageReady(page);

    // Capture after enabling first integration
    await captureScreenshot(page, "plugins/user-integrations-enabled-sync");

    // Enable the second available integration if present (likely AniList Recommendations)
    const remainingEnableButtons = page.locator('button:has-text("Enable")');
    if ((await remainingEnableButtons.count()) > 0) {
      await remainingEnableButtons.first().click();
      await page.waitForTimeout(1500);
      await waitForPageReady(page);

      // Capture after enabling second integration
      await captureScreenshot(page, "plugins/user-integrations-all-enabled");
    }
  } else {
    console.log("      ⚠️  No available integrations found to enable");
  }

  console.log("      ✓ User integrations screenshots captured");
}

/**
 * Plugin metrics tab in settings
 * Shows plugin performance statistics after plugin usage
 */
async function pluginMetricsScreenshots(page: Page): Promise<void> {
  console.log("    📷 Plugin Metrics Tab");

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
  const pluginRow = page.locator('[role="tabpanel"][aria-labelledby*="plugins" i] .mantine-Table-tbody .mantine-Table-tr').first();
  if ((await pluginRow.count()) > 0 && (await pluginRow.isVisible())) {
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
