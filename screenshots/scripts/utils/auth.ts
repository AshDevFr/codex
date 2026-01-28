import { Page, BrowserContext } from "playwright";
import { config } from "../../playwright.config.js";
import { waitForPageReady } from "./wait.js";

const AUTH_STATE_PATH = "./auth-state.json";

/**
 * Perform login with provided credentials
 * @param page - Playwright page instance
 * @param username - Username or email
 * @param password - Password
 */
export async function login(
  page: Page,
  username: string = config.admin.username,
  password: string = config.admin.password
): Promise<void> {
  console.log(`  🔐 Logging in as ${username}...`);

  await page.goto("/login");
  await waitForPageReady(page);

  // Fill login form
  await page.fill('input[name="username"], input[type="text"]', username);
  await page.fill('input[name="password"], input[type="password"]', password);

  // Submit form
  await page.click('button[type="submit"]');

  // Wait for redirect to dashboard
  await page.waitForURL((url) => !url.pathname.includes("/login"), {
    timeout: 10000,
  });
  await waitForPageReady(page);

  console.log(`  ✓ Logged in successfully`);
}

/**
 * Save authentication state (cookies, localStorage) for reuse
 * @param context - Browser context
 */
export async function saveAuthState(context: BrowserContext): Promise<void> {
  await context.storageState({ path: AUTH_STATE_PATH });
  console.log(`  💾 Auth state saved to ${AUTH_STATE_PATH}`);
}

/**
 * Check if user is authenticated by checking for auth indicators
 * @param page - Playwright page instance
 */
export async function isAuthenticated(page: Page): Promise<boolean> {
  // Check if we're on a protected page or if there's a logout button
  const logoutButton = await page.$('button:has-text("Logout"), a:has-text("Logout")');
  return logoutButton !== null;
}

/**
 * Logout the current user
 * @param page - Playwright page instance
 */
export async function logout(page: Page): Promise<void> {
  console.log("  🔓 Logging out...");

  // First, go to home to ensure we're on a page with the sidebar
  await page.goto("/");
  await waitForPageReady(page);

  // Try to find and click the logout button, scrolling if needed
  const logoutButton = await page.$('button:has-text("Logout"), a:has-text("Logout"), [data-testid="logout"]');
  if (logoutButton) {
    // Scroll the element into view before clicking
    await logoutButton.scrollIntoViewIfNeeded();
    await page.waitForTimeout(200);
    await logoutButton.click();
    await page.waitForURL("**/login", { timeout: 5000 });
    await waitForPageReady(page);
    console.log("  ✓ Logged out successfully");
  } else {
    // Try using keyboard shortcut or direct navigation
    // Clear cookies/storage to force logout
    await page.context().clearCookies();
    await page.goto("/login");
    await waitForPageReady(page);
    console.log("  ✓ Logged out via cookie clear");
  }
}
