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

  // Capture settings step
  await captureScreenshot(page, "setup/wizard-step2-settings");

  // Complete setup with defaults
  console.log("  📝 Completing setup...");
  await page.click('button[type="submit"]:has-text("Finish Setup")');

  // Wait for redirect to home
  await page.waitForURL((url) => !url.pathname.includes("/setup"), { timeout: 15000 });
  await waitForPageReady(page);

  // Capture post-setup dashboard
  console.log("  📝 Post-setup: Dashboard");
  await captureScreenshot(page, "setup/complete-dashboard");
}
