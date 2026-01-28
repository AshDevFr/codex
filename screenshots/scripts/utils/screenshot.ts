import { Page } from "playwright";
import { mkdir } from "fs/promises";
import { existsSync } from "fs";
import path from "path";
import { config } from "../../playwright.config.js";
import { waitForToastsToDisappear } from "./wait.js";

export interface ScreenshotOptions {
  fullPage?: boolean;
  clip?: { x: number; y: number; width: number; height: number };
  timeout?: number;
}

const capturedScreenshots: string[] = [];

/**
 * Ensure the output directory exists
 */
async function ensureOutputDir(): Promise<void> {
  const outputDir = path.resolve(config.outputDir);
  if (!existsSync(outputDir)) {
    await mkdir(outputDir, { recursive: true });
  }
}

/**
 * Capture a screenshot with consistent naming
 * @param page - Playwright page instance
 * @param name - Screenshot name (without extension)
 * @param options - Screenshot options
 * @returns Path to the saved screenshot
 */
export async function captureScreenshot(
  page: Page,
  name: string,
  options: ScreenshotOptions = {}
): Promise<string> {
  await ensureOutputDir();

  // Wait for any toast notifications to disappear before capturing
  await waitForToastsToDisappear(page);

  const filename = `${name}.png`;
  const filepath = path.join(config.outputDir, filename);

  await page.screenshot({
    path: filepath,
    fullPage: options.fullPage ?? false,
    clip: options.clip,
    timeout: options.timeout ?? 30000,
  });

  capturedScreenshots.push(filename);
  console.log(`  📸 Captured: ${filename}`);

  return filepath;
}

/**
 * Get list of all captured screenshots
 */
export function getCapturedScreenshots(): string[] {
  return [...capturedScreenshots];
}

/**
 * Print summary of captured screenshots
 */
export function printScreenshotSummary(): void {
  console.log("\n" + "=".repeat(50));
  console.log("Screenshot Summary");
  console.log("=".repeat(50));
  console.log(`Total: ${capturedScreenshots.length} screenshots captured\n`);

  for (const screenshot of capturedScreenshots) {
    console.log(`  ✓ ${screenshot}`);
  }

  console.log("\n" + "=".repeat(50));
  console.log(`Output directory: ${path.resolve(config.outputDir)}`);
  console.log("=".repeat(50) + "\n");
}
