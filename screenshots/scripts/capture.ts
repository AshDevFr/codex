import { chromium, Browser, BrowserContext, Page } from "playwright";
import { config } from "../playwright.config.js";
import { printScreenshotSummary } from "./utils/screenshot.js";

// Import scenarios (will be implemented in Phase 3)
// import { runSetupScenario } from "./scenarios/setup.js";
// import { runLibrariesScenario } from "./scenarios/libraries.js";
// import { runSettingsScenario } from "./scenarios/settings.js";
// import { runReaderScenario } from "./scenarios/reader.js";
// import { runNavigationScenario } from "./scenarios/navigation.js";

interface ScenarioModule {
  run: (page: Page, context: BrowserContext) => Promise<void>;
  name: string;
}

/**
 * Main screenshot capture orchestration
 */
async function main(): Promise<void> {
  console.log("\n" + "=".repeat(50));
  console.log("Codex Screenshot Automation");
  console.log("=".repeat(50));
  console.log(`Base URL: ${config.baseUrl}`);
  console.log(`Viewport: ${config.viewport.width}x${config.viewport.height}`);
  console.log(`Output: ${config.outputDir}`);
  console.log("=".repeat(50) + "\n");

  let browser: Browser | null = null;
  let context: BrowserContext | null = null;

  try {
    // Launch browser
    console.log("🚀 Launching browser...");
    browser = await chromium.launch({
      headless: true,
    });

    // Create context with dark mode and viewport settings
    context = await browser.newContext({
      viewport: config.viewport,
      colorScheme: "dark",
      baseURL: config.baseUrl,
    });

    const page = await context.newPage();

    // Wait for frontend to be available
    console.log("⏳ Waiting for frontend to be ready...");
    await waitForFrontend(page);
    console.log("✓ Frontend is ready\n");

    // Run scenarios in sequence
    const scenarios: ScenarioModule[] = [];

    // Dynamically import scenarios if they exist
    try {
      const setup = await import("./scenarios/setup.js");
      scenarios.push({ name: "Setup Wizard", run: setup.run });
    } catch {
      console.log("⚠️  Setup scenario not found, skipping");
    }

    try {
      const libraries = await import("./scenarios/libraries.js");
      scenarios.push({ name: "Libraries", run: libraries.run });
    } catch {
      console.log("⚠️  Libraries scenario not found, skipping");
    }

    // Settings is now captured during Libraries scenario (while scans run)
    // to ensure Tasks/Metrics pages have data

    try {
      const reader = await import("./scenarios/reader.js");
      scenarios.push({ name: "Reader", run: reader.run });
    } catch {
      console.log("⚠️  Reader scenario not found, skipping");
    }

    try {
      const navigation = await import("./scenarios/navigation.js");
      scenarios.push({ name: "Navigation", run: navigation.run });
    } catch {
      console.log("⚠️  Navigation scenario not found, skipping");
    }

    if (scenarios.length === 0) {
      console.log("\n⚠️  No scenarios found. Please implement scenario modules in ./scenarios/");
      console.log("Expected files:");
      console.log("  - ./scenarios/setup.ts");
      console.log("  - ./scenarios/libraries.ts");
      console.log("  - ./scenarios/settings.ts");
      console.log("  - ./scenarios/reader.ts");
      console.log("  - ./scenarios/navigation.ts");
    }

    // Execute each scenario
    for (const scenario of scenarios) {
      console.log(`\n📷 Running scenario: ${scenario.name}`);
      console.log("-".repeat(40));

      try {
        await scenario.run(page, context);
        console.log(`✓ ${scenario.name} completed`);
      } catch (error) {
        console.error(`✗ ${scenario.name} failed:`, error);
        // Continue with other scenarios even if one fails
      }
    }

    // Print summary
    printScreenshotSummary();

    console.log("✅ Screenshot capture complete!\n");
  } catch (error) {
    console.error("❌ Screenshot capture failed:", error);
    process.exit(1);
  } finally {
    // Cleanup
    if (context) {
      await context.close();
    }
    if (browser) {
      await browser.close();
    }
  }
}

/**
 * Wait for the frontend to be available
 * Retries with exponential backoff
 */
async function waitForFrontend(
  page: Page,
  maxRetries: number = 30,
  initialDelay: number = 1000
): Promise<void> {
  let delay = initialDelay;

  for (let i = 0; i < maxRetries; i++) {
    try {
      const response = await page.goto("/", {
        timeout: 10000,
        waitUntil: "domcontentloaded",
      });

      if (response && response.ok()) {
        return;
      }
    } catch {
      // Retry on error
    }

    console.log(`  Waiting for frontend... (attempt ${i + 1}/${maxRetries})`);
    await page.waitForTimeout(delay);
    delay = Math.min(delay * 1.2, 5000); // Cap at 5 seconds
  }

  throw new Error("Frontend did not become available");
}

// Run main
main();
