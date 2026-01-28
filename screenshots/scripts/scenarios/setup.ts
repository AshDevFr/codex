import { Page, BrowserContext } from "playwright";
import { config } from "../../playwright.config.js";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Setup wizard scenario
 * Captures screenshots of the setup wizard flow
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  // Navigate to the app - should redirect to /setup if not configured
  await page.goto("/");
  await waitForPageReady(page);

  // Check if we're redirected to setup
  const currentUrl = page.url();
  if (!currentUrl.includes("/setup")) {
    console.log("  ⚠️  Setup already complete, skipping setup wizard screenshots");
    return;
  }

  // Step 1: Create Admin User
  console.log("  📝 Setup Step 1: Create Admin User");
  await waitForPageReady(page);

  // Capture empty form
  await captureScreenshot(page, "setup/wizard-step1-empty");

  // Fill in admin credentials
  await page.fill('input[placeholder="admin"]', config.admin.username);
  await page.fill('input[placeholder="admin@example.com"]', config.admin.email);
  await page.fill('input[placeholder="Your password"]', config.admin.password);
  await page.fill('input[placeholder="Confirm your password"]', config.admin.password);

  // Small delay to let validation update
  await page.waitForTimeout(300);

  // Capture filled form
  await captureScreenshot(page, "setup/wizard-step1-filled");

  // Submit the form
  await page.click('button[type="submit"]:has-text("Create Admin User")');

  // Wait for step 2 to load
  await page.waitForSelector('text=Configure Settings', { state: "visible", timeout: 10000 });
  await waitForPageReady(page);

  // Step 2: Configure Settings
  console.log("  📝 Setup Step 2: Configure Settings");

  // The switch starts unchecked (skipSettings = false), so settings are visible by default
  // First, check the "Skip" switch to hide settings, then capture that view
  const skipLabel = await page.$('label:has-text("Skip configuration")');
  if (skipLabel) {
    // Click to check the skip switch (hide settings)
    await skipLabel.click();
    await page.waitForTimeout(500);
  }

  // Capture with skip checked (settings hidden)
  await captureScreenshot(page, "setup/wizard-step2-skip");

  // Now uncheck the skip switch to show settings
  if (skipLabel) {
    await skipLabel.click();
    await page.waitForTimeout(500);
  }

  // Capture with basic settings visible
  await captureScreenshot(page, "setup/wizard-step2-basic-settings");

  // Click "Show Advanced Settings" to expand
  const advancedButton = await page.$('button:has-text("Show Advanced Settings")');
  if (advancedButton) {
    await advancedButton.click();
    await page.waitForTimeout(500); // Wait for collapse animation
    await captureScreenshot(page, "setup/wizard-step2-advanced-settings", { fullPage: true });
  }

  // Check the skip option and complete setup
  console.log("  📝 Completing setup with defaults...");

  // Re-check skip to use defaults
  const skipSwitchAgain = await page.$('label:has-text("Skip configuration") input[type="checkbox"]');
  if (skipSwitchAgain) {
    const isChecked = await skipSwitchAgain.isChecked();
    if (!isChecked) {
      await page.click('label:has-text("Skip configuration")');
      await page.waitForTimeout(300);
    }
  }

  // Submit to complete setup
  await page.click('button[type="submit"]:has-text("Skip and Finish"), button[type="submit"]:has-text("Save Settings and Finish")');

  // Wait for redirect to home
  await page.waitForURL((url) => !url.pathname.includes("/setup"), { timeout: 15000 });
  await waitForPageReady(page);

  // Capture post-setup dashboard
  console.log("  📝 Post-setup: Dashboard");
  await captureScreenshot(page, "setup/complete-dashboard");
}
