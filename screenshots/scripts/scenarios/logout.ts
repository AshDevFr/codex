import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";
import { logout } from "../utils/auth.js";

/**
 * Logout scenario - runs last to capture login page
 * This scenario logs out and captures the login page screenshot
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  🚪 Capturing logout/login page...");

  // Capture login page (logout first)
  console.log("    📷 Login page");
  await logout(page);
  await waitForPageReady(page);
  await captureScreenshot(page, "navigation/login-page");
}
