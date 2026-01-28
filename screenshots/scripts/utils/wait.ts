import { Page } from "playwright";

const DEFAULT_TIMEOUT = 30000;

/**
 * Wait for page to be fully loaded and ready for screenshot
 * Waits for network idle and no loading indicators
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms
 */
export async function waitForPageReady(
  page: Page,
  _timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  // Wait for DOM to be ready first (with short timeout - this should be fast)
  try {
    await page.waitForLoadState("domcontentloaded", { timeout: 10000 });
  } catch {
    // DOM didn't load in time, but page might still be usable
    console.log("    (DOM load timeout, continuing...)");
  }

  // Try to wait for network idle, but don't fail if it times out
  try {
    await page.waitForLoadState("networkidle", { timeout: 5000 });
  } catch {
    // Network didn't become idle, but that's OK - continue anyway
  }

  // Wait for no loading indicators (shorter timeout)
  await waitForNoLoadingIndicators(page, 5000);

  // Small delay for any final renders
  await page.waitForTimeout(500);
}

/**
 * Wait for all loading indicators to disappear
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms
 */
export async function waitForNoLoadingIndicators(
  page: Page,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  const loadingSelectors = [
    // Mantine loading indicators
    '[data-loading="true"]',
    ".mantine-Loader-root",
    ".mantine-LoadingOverlay-root",
    // Common loading patterns
    '[aria-busy="true"]',
    ".loading",
    ".spinner",
    // Skeleton loaders
    ".mantine-Skeleton-root:not([data-visible='false'])",
  ];

  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    let hasLoadingIndicator = false;

    for (const selector of loadingSelectors) {
      const element = await page.$(selector);
      if (element) {
        const isVisible = await element.isVisible();
        if (isVisible) {
          hasLoadingIndicator = true;
          break;
        }
      }
    }

    if (!hasLoadingIndicator) {
      return;
    }

    await page.waitForTimeout(100);
  }
}

/**
 * Wait for a specific element to appear
 * @param page - Playwright page instance
 * @param selector - CSS selector
 * @param timeout - Maximum wait time in ms
 */
export async function waitForElement(
  page: Page,
  selector: string,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  await page.waitForSelector(selector, {
    state: "visible",
    timeout,
  });
}

/**
 * Wait for library scan to complete by polling the tasks API
 * @param page - Playwright page instance
 * @param maxWaitTime - Maximum wait time in ms (default 5 minutes)
 * @param pollInterval - How often to check in ms (default 3 seconds)
 */
export async function waitForLibraryScan(
  page: Page,
  maxWaitTime: number = 300000,
  pollInterval: number = 3000
): Promise<void> {
  console.log("  ⏳ Waiting for library scan to complete...");

  const startTime = Date.now();
  const currentUrl = page.url();

  // Initial delay to let tasks get queued
  await page.waitForTimeout(2000);

  while (Date.now() - startTime < maxWaitTime) {
    // Navigate to tasks page to check status
    await page.goto("/settings/tasks");

    // Wait for page load with longer timeout and catch errors
    try {
      await page.waitForLoadState("domcontentloaded", { timeout: 10000 });
      await page.waitForTimeout(1000); // Let React render
    } catch {
      console.log("  ... waiting for tasks page to load");
      await page.waitForTimeout(pollInterval);
      continue;
    }

    // Check the stats cards for pending/processing counts
    // The stats show "Pending" and "Processing" with their counts
    const statsText = await page.textContent("body");

    // Look for the Active Tasks section which only appears when tasks are running
    const hasActiveTasks = await page.$('text="Active Tasks"');

    // Check if there are pending or processing tasks by looking at the stat cards
    // The page shows cards with titles like "Pending" and "Processing" followed by counts
    const hasPendingOrProcessing =
      hasActiveTasks !== null ||
      (statsText?.includes("Processing") && statsText?.match(/Processing[\s\S]*?[1-9]/)) ||
      (statsText?.includes("Pending") && statsText?.match(/Pending[\s\S]*?[1-9]/));

    if (!hasPendingOrProcessing) {
      console.log("  ✓ Library scan complete");
      // Return to original page
      await page.goto(currentUrl);
      try {
        await page.waitForLoadState("domcontentloaded", { timeout: 10000 });
        await page.waitForTimeout(500);
      } catch {
        // Ignore timeout on return navigation
      }
      return;
    }

    console.log(`  ... scan in progress (${Math.round((Date.now() - startTime) / 1000)}s)`);
    await page.waitForTimeout(pollInterval);
  }

  console.log("  ⚠️ Scan wait timeout reached, continuing anyway");
  await page.goto(currentUrl);
  try {
    await page.waitForLoadState("domcontentloaded", { timeout: 10000 });
  } catch {
    // Ignore timeout
  }
}

/**
 * Wait for images to load within the page
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms
 */
export async function waitForImages(
  page: Page,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  await page.waitForFunction(
    () => {
      const images = document.querySelectorAll("img");
      return Array.from(images).every((img) => img.complete && img.naturalHeight > 0);
    },
    { timeout }
  );
}

/**
 * Wait for a specific URL pattern
 * @param page - Playwright page instance
 * @param pattern - URL pattern (string or regex)
 * @param timeout - Maximum wait time in ms
 */
export async function waitForUrl(
  page: Page,
  pattern: string | RegExp,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  await page.waitForURL(pattern, { timeout });
}

/**
 * Wait for thumbnails to be generated
 * Thumbnails can take 10+ seconds to generate for series/books
 * The backend generates thumbnails on-demand, and the frontend shows:
 * 1. A Skeleton component while loading
 * 2. A fallback "No Cover" SVG if generation fails
 * 3. The actual thumbnail once ready
 *
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms (default 60 seconds for slow thumbnail generation)
 */
export async function waitForThumbnails(
  page: Page,
  timeout: number = 60000
): Promise<void> {
  console.log("    ⏳ Waiting for thumbnails to generate...");

  const startTime = Date.now();

  // First, wait for any visible skeleton loaders to disappear
  // These indicate images are still being loaded
  while (Date.now() - startTime < timeout) {
    const skeletons = await page.$$('.mantine-Skeleton-root');
    let hasVisibleSkeleton = false;

    for (const skeleton of skeletons) {
      if (await skeleton.isVisible()) {
        hasVisibleSkeleton = true;
        break;
      }
    }

    if (!hasVisibleSkeleton) {
      break;
    }

    await page.waitForTimeout(1000);
  }

  // Wait for all images to load
  await waitForImages(page, timeout).catch(() => {
    console.log("    (Some images may not have loaded)");
  });

  // Additional wait time for thumbnail generation - thumbnails are generated on-demand
  // and may take 10+ seconds per image. Wait a bit to allow more to load.
  await page.waitForTimeout(10000);

  // Refresh the page to get the newly generated thumbnails
  // This is because the frontend may have cached the placeholder/No Cover response
  await page.reload();
  await waitForPageReady(page);

  // Wait for skeletons again after reload
  const reloadStartTime = Date.now();
  while (Date.now() - reloadStartTime < 30000) {
    const skeletons = await page.$$('.mantine-Skeleton-root');
    let hasVisibleSkeleton = false;

    for (const skeleton of skeletons) {
      if (await skeleton.isVisible()) {
        hasVisibleSkeleton = true;
        break;
      }
    }

    if (!hasVisibleSkeleton) {
      break;
    }

    await page.waitForTimeout(1000);
  }

  // Final wait for images after reload
  await waitForImages(page, 30000).catch(() => {});

  // Small extra buffer for any final renders
  await page.waitForTimeout(2000);

  console.log("    ✓ Thumbnails ready");
}

/**
 * Wait for PDF pages to render in react-pdf
 * PDF pages are rendered to canvas elements, not img tags.
 * The Page component shows a Loader while rendering, then displays the canvas.
 * PDF.js renders asynchronously, so we need to wait for actual content to be painted.
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms
 */
export async function waitForPdfPages(
  page: Page,
  timeout: number = DEFAULT_TIMEOUT
): Promise<void> {
  const startTime = Date.now();

  // Wait for loaders to disappear (react-pdf shows Loader while rendering)
  while (Date.now() - startTime < timeout) {
    const loaders = await page.$$('.mantine-Loader-root');
    let hasVisibleLoader = false;
    for (const loader of loaders) {
      if (await loader.isVisible()) {
        hasVisibleLoader = true;
        break;
      }
    }
    if (!hasVisibleLoader) {
      break;
    }
    await page.waitForTimeout(200);
  }

  // Wait for canvas elements to appear with non-zero dimensions
  // react-pdf renders PDF pages to canvas elements
  await page.waitForFunction(
    () => {
      const canvases = document.querySelectorAll('canvas');
      if (canvases.length === 0) return false;
      // Check that at least one canvas has been rendered (has dimensions)
      return Array.from(canvases).some((canvas) => canvas.width > 0 && canvas.height > 0);
    },
    { timeout: Math.max(1000, timeout - (Date.now() - startTime)) }
  ).catch(() => {
    // Canvas may not appear if PDF load fails - don't throw
  });

  // Wait for PDF.js to actually paint content to the canvas
  // PDF.js renders asynchronously after the canvas is created
  // We check if the canvas has non-white pixels (actual content)
  // This may timeout for documents with mostly white pages, which is fine
  await page.waitForFunction(
    () => {
      const canvases = document.querySelectorAll('canvas');
      for (const canvas of canvases) {
        if (canvas.width === 0 || canvas.height === 0) continue;
        try {
          const ctx = canvas.getContext('2d');
          if (!ctx) continue;
          // Sample pixels along a diagonal to check for any rendered content
          // This catches text, images, borders, etc.
          const samples = 20;
          for (let i = 0; i < samples; i++) {
            const x = Math.floor((canvas.width * (i + 1)) / (samples + 1));
            const y = Math.floor((canvas.height * (i + 1)) / (samples + 1));
            const imageData = ctx.getImageData(x, y, 1, 1).data;
            // Check if pixel is not pure white (255,255,255) or transparent (0,0,0,0)
            const isWhite = imageData[0] === 255 && imageData[1] === 255 && imageData[2] === 255;
            const isTransparent = imageData[3] === 0;
            if (!isWhite && !isTransparent) {
              return true; // Found rendered content
            }
          }
        } catch {
          // Canvas might be tainted or inaccessible, skip
        }
      }
      return false;
    },
    { timeout: Math.max(5000, timeout - (Date.now() - startTime)) }
  ).catch(() => {
    // Content detection may timeout for white pages - that's okay
  });

  // Extra delay to ensure rendering is fully complete
  // PDF.js rendering can take time especially for complex pages
  await page.waitForTimeout(2000);
}

/**
 * Wait for toast notifications to disappear
 * Mantine notifications appear in a container and auto-dismiss
 * @param page - Playwright page instance
 * @param timeout - Maximum wait time in ms
 */
export async function waitForToastsToDisappear(
  page: Page,
  timeout: number = 10000
): Promise<void> {
  const toastSelectors = [
    // Mantine notification selectors
    ".mantine-Notifications-root .mantine-Notification-root",
    '[data-mantine-notification="true"]',
    ".mantine-Notification-root",
  ];

  const startTime = Date.now();

  // First, give a small initial delay for any new toasts to appear
  await page.waitForTimeout(300);

  while (Date.now() - startTime < timeout) {
    let hasVisibleToast = false;

    for (const selector of toastSelectors) {
      const elements = await page.$$(selector);
      for (const element of elements) {
        const isVisible = await element.isVisible();
        if (isVisible) {
          hasVisibleToast = true;
          break;
        }
      }
      if (hasVisibleToast) break;
    }

    if (!hasVisibleToast) {
      return;
    }

    await page.waitForTimeout(200);
  }
}
