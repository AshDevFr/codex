import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady, waitForImages, waitForThumbnails, waitForPdfPages } from "../utils/wait.js";

// File format types to capture
type FileFormat = "cbz" | "epub" | "pdf";

interface ReaderConfig {
  format: FileFormat;
  searchPattern: string; // Pattern to find books of this type
  prefix: string; // Screenshot prefix
  label: string;
}

const READER_CONFIGS: ReaderConfig[] = [
  {
    format: "cbz",
    searchPattern: ".cbz",
    prefix: "reader/comic",
    label: "Comic (CBZ)",
  },
  {
    format: "epub",
    searchPattern: ".epub",
    prefix: "reader/epub",
    label: "EPUB",
  },
  {
    format: "pdf",
    searchPattern: ".pdf",
    prefix: "reader/pdf",
    label: "PDF",
  },
];

/**
 * Reader scenario
 * Opens books of each type in the reader and captures various views
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  📖 Capturing reader screenshots for each file type...");

  for (const readerConfig of READER_CONFIGS) {
    await captureReaderForFormat(page, readerConfig);
  }

  // Go back to the app
  await page.goto("/");
  await waitForPageReady(page);
}

/**
 * Switch the reader to double page mode via settings
 */
async function switchToDoublePage(page: Page, format: string): Promise<void> {
  console.log(`      Switching to double page mode for ${format.toUpperCase()}...`);

  // Ensure toolbar is visible by moving mouse to top
  await page.mouse.move(960, 30);
  await page.waitForTimeout(300);

  // Find and click the settings button
  let settingsButton = await page.$('button[aria-label="Settings"]');
  if (!settingsButton) {
    settingsButton = await page.$('button:has(svg.tabler-icon-settings)');
  }
  if (!settingsButton) {
    // Find last visible ActionIcon (settings is typically last in toolbar)
    const allActionIcons = await page.$$('button.mantine-ActionIcon-root');
    const visibleIcons = [];
    for (const icon of allActionIcons) {
      if (await icon.isVisible()) {
        visibleIcons.push(icon);
      }
    }
    if (visibleIcons.length > 0) {
      settingsButton = visibleIcons[visibleIcons.length - 1];
    }
  }

  if (!settingsButton) {
    console.log(`      ⚠️  Could not find settings button to switch to double page mode`);
    return;
  }

  await settingsButton.click();
  await page.waitForTimeout(800);

  // Verify settings panel opened
  const settingsPanel = await page.$('.mantine-Modal-body, .mantine-Drawer-body, .mantine-Drawer-content');
  if (!settingsPanel || !(await settingsPanel.isVisible())) {
    console.log(`      ⚠️  Settings panel did not open`);
    return;
  }

  // Find the "Page layout" section and click the appropriate double option
  // For PDF: select "Double (Odd)" which starts spreads on odd pages
  // For CBZ/CBR: select "Double"
  const targetLabel = format === "pdf" ? "Double (Odd)" : "Double";

  // Try clicking by looking for the control that contains the layout options
  const segmentedControls = await page.$$('.mantine-SegmentedControl-root');
  let found = false;
  for (const control of segmentedControls) {
    const labels = await control.$$eval('.mantine-SegmentedControl-label', (els: Element[]) => els.map((el: Element) => el.textContent));
    // Check if this is a page layout control (has Single and some Double variant)
    if (labels.includes('Single') && labels.some(l => l?.includes('Double'))) {
      // Found the page layout control, click the appropriate Double option
      const doubleLabel = await control.$( `.mantine-SegmentedControl-label:has-text("${targetLabel}")`);
      if (doubleLabel) {
        await doubleLabel.click();
        await page.waitForTimeout(500);
        console.log(`      ✓ Switched to ${targetLabel} mode`);
        found = true;
        break;
      }
    }
  }

  if (!found) {
    console.log(`      ⚠️  Could not find ${targetLabel} option in settings`);
  }

  // Close settings modal
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);

  // Wait for the reader to re-render with new layout
  // PDF rendering needs extra time to recalculate dimensions
  await page.waitForTimeout(2000);
}

/**
 * Capture reader screenshots for a specific file format
 */
async function captureReaderForFormat(page: Page, readerConfig: ReaderConfig): Promise<void> {
  const { format, prefix, label } = readerConfig;
  console.log(`    📷 ${label} reader...`);

  // Navigate to all books
  await page.goto("/libraries/all/books");
  await waitForPageReady(page);
  await waitForThumbnails(page);

  // Wait a bit for books to load
  await page.waitForTimeout(2000);

  // Find a book of this format by looking at the book cards
  let bookFound = false;

  // Get all book links - collect hrefs first to avoid stale element handles
  const bookCards = await page.$$('a[href*="/books/"]');
  const bookHrefs: string[] = [];
  for (const card of bookCards) {
    const href = await card.getAttribute("href");
    if (href) {
      bookHrefs.push(href);
    }
  }
  console.log(`      Found ${bookHrefs.length} book cards`);

  // Now iterate through the hrefs
  for (const href of bookHrefs) {
    // Navigate directly to the book detail page
    await page.goto(href);
    await waitForPageReady(page);
    await page.waitForTimeout(1000);

    // Check if this book has the right format by looking for format badge/text
    // The format is usually displayed in a badge or info section
    const pageText = await page.textContent("body");
    const hasFormat = pageText?.toUpperCase().includes(format.toUpperCase());

    if (hasFormat) {
      bookFound = true;
      console.log(`      Found ${format.toUpperCase()} book`);

      // Find and click the "Read" button
      const readButton = await page.$('button:has-text("Read"), a:has-text("Read")');
      if (readButton) {
        await readButton.click();

        // Wait for reader to load
        try {
          await page.waitForURL("**/reader/**", { timeout: 30000 });
          await waitForPageReady(page);
          // Wait for content to render
          if (format === "pdf") {
            // PDF uses canvas rendering via react-pdf
            await waitForPdfPages(page);
          } else {
            await page.waitForTimeout(3000); // Let content load
            await waitForImages(page).catch(() => {});
          }

          // First, open settings and switch to double page mode (for non-EPUB formats)
          if (format !== "epub") {
            await switchToDoublePage(page, format);
            // Wait for content to re-render after layout change
            if (format === "pdf") {
              // PDF uses canvas rendering via react-pdf
              await waitForPdfPages(page);
            } else {
              await page.waitForTimeout(2000);
              await waitForImages(page).catch(() => {});
            }
          }

          // Capture main reader view (now in double page mode for comics/PDFs)
          await captureScreenshot(page, `${prefix}-view`);

          // Show toolbar by pressing Space (toggles toolbar visibility)
          // The toolbar has autoHide which hides it after a delay
          await page.keyboard.press("Space");
          await page.waitForTimeout(500);

          // Also move mouse to top to trigger toolbar visibility
          await page.mouse.move(960, 50);
          await page.waitForTimeout(500);

          // For PDF, wait for pages to render again after toolbar visibility change
          // The layout shift can cause PDF.js to need to re-render
          if (format === "pdf") {
            await waitForPdfPages(page);
          }

          // Capture with toolbar visible
          await captureScreenshot(page, `${prefix}-toolbar`);

          // Try to open settings panel - capture for ALL formats (settings already in double mode)
          // The Settings button is a Mantine ActionIcon inside a Tooltip with label="Settings"
          // It's rendered by ReaderToolbar component when onOpenSettings is provided
          // The button contains an IconSettings SVG from @tabler/icons-react

          // First, ensure toolbar is visible by keeping mouse at top
          await page.mouse.move(960, 30);
          await page.waitForTimeout(300);

          // Find the settings button using multiple strategies
          // Strategy 1: Look for button with Settings tooltip (Mantine tooltips use data-floating-ui)
          let settingsButton = await page.$('button[aria-label="Settings"]');

          // Strategy 2: Find by SVG class - tabler icons have specific class pattern
          if (!settingsButton) {
            settingsButton = await page.$('button:has(svg.tabler-icon-settings)');
          }

          // Strategy 3: Find buttons in the toolbar area and identify by SVG content
          if (!settingsButton) {
            // The toolbar is positioned absolute at top with high z-index
            // Look for ActionIcon buttons that contain a gear/settings icon
            const buttons = await page.$$('button.mantine-ActionIcon-root');
            for (const btn of buttons) {
              // Check if button is visible
              if (!(await btn.isVisible())) continue;

              // Get the SVG inside and check its class or content
              const svg = await btn.$('svg');
              if (!svg) continue;

              const svgClass = await svg.getAttribute('class') || '';
              const svgHtml = await svg.innerHTML();

              // Tabler settings icon has class containing "settings" or specific path patterns
              // The gear icon has a distinctive circle in the center and cog teeth around it
              const pathMatches = svgHtml.match(/d="[^"]*[Mm]/g);
              if (svgClass.includes('settings') ||
                  svgClass.includes('Settings') ||
                  // Check for gear icon pattern: has both circle and path elements typical of gear
                  (svgHtml.includes('<circle') && svgHtml.includes('<path') && pathMatches && pathMatches.length > 5)) {
                settingsButton = btn;
                console.log(`      Found settings button via SVG analysis`);
                break;
              }
            }
          }

          // Strategy 4: Last ActionIcon in the visible area (settings is typically last)
          if (!settingsButton) {
            const allActionIcons = await page.$$('button.mantine-ActionIcon-root');
            const visibleIcons = [];
            for (const icon of allActionIcons) {
              if (await icon.isVisible()) {
                visibleIcons.push(icon);
              }
            }
            // Settings button is the last one in the toolbar
            if (visibleIcons.length > 0) {
              settingsButton = visibleIcons[visibleIcons.length - 1];
              console.log(`      Using last visible ActionIcon as settings button`);
            }
          }

          if (settingsButton) {
            // Click the settings button
            await settingsButton.click();
            await page.waitForTimeout(1000);

            // Verify settings panel opened - it's a Drawer or Modal
            const settingsPanel = await page.$('.mantine-Drawer-body, .mantine-Modal-body, .mantine-Drawer-content');
            if (settingsPanel && await settingsPanel.isVisible()) {
              await captureScreenshot(page, `${prefix}-settings`);
              // Close settings
              await page.keyboard.press("Escape");
              await page.waitForTimeout(500);
            } else {
              // Maybe panel takes longer to animate in
              await page.waitForTimeout(500);
              const retryPanel = await page.$('.mantine-Drawer-body, .mantine-Modal-body, .mantine-Drawer-content');
              if (retryPanel && await retryPanel.isVisible()) {
                await captureScreenshot(page, `${prefix}-settings`);
                await page.keyboard.press("Escape");
                await page.waitForTimeout(500);
              } else {
                console.log(`      ⚠️  Settings panel did not open for ${label}`);
              }
            }
          } else {
            console.log(`      ⚠️  Settings button not found for ${label}`);
          }

          // For EPUB: try to open table of contents
          if (format === "epub") {
            // Try finding TOC button with various selectors
            let tocButton = await page.$('button[aria-label="Table of Contents"]');
            if (!tocButton) {
              tocButton = await page.$('button:has(svg[class*="icon-list"])');
            }
            if (!tocButton) {
              // Look for ActionIcons containing list-related content
              const actionIcons = await page.$$('.mantine-ActionIcon-root');
              for (const icon of actionIcons) {
                const html = await icon.innerHTML();
                if (html.includes('list') || html.includes('List') || html.includes('toc') || html.includes('contents')) {
                  tocButton = icon;
                  break;
                }
              }
            }
            if (tocButton) {
              await tocButton.scrollIntoViewIfNeeded();
              await page.waitForTimeout(200);
              await tocButton.click();
              await page.waitForTimeout(800);
              await captureScreenshot(page, `${prefix}-toc`);
              await page.keyboard.press("Escape");
              await page.waitForTimeout(500);
            }
          }

        } catch (error) {
          console.log(`      ⚠️  Failed to capture ${label} reader: ${error}`);
        }
      } else {
        console.log(`      ⚠️  Read button not found for ${label}`);
      }

      // Done with this format
      break;
    }
  }

  if (!bookFound) {
    console.log(`      ⚠️  No ${label} books found, skipping`);
  }
}
