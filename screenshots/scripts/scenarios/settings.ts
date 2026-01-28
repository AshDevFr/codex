import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Settings pages scenario
 * Captures all settings pages for documentation
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  ⚙️  Capturing settings pages...");

  // Define all settings pages to capture
  const settingsPages = [
    { path: "/settings/server", name: "30-settings-server", label: "Server Settings" },
    { path: "/settings/tasks", name: "31-settings-tasks", label: "Tasks" },
    { path: "/settings/metrics", name: "32-settings-metrics", label: "Metrics" },
    { path: "/settings/users", name: "33-settings-users", label: "User Management" },
    { path: "/settings/sharing-tags", name: "34-settings-sharing-tags", label: "Sharing Tags" },
    { path: "/settings/duplicates", name: "35-settings-duplicates", label: "Duplicates" },
    { path: "/settings/book-errors", name: "36-settings-book-errors", label: "Book Errors" },
    { path: "/settings/cleanup", name: "37-settings-cleanup", label: "Thumbnail Cleanup" },
    { path: "/settings/pdf-cache", name: "38-settings-pdf-cache", label: "PDF Cache" },
    { path: "/settings/profile", name: "39-settings-profile", label: "Profile" },
  ];

  for (const { path, name, label } of settingsPages) {
    console.log(`    📷 ${label}`);

    try {
      await page.goto(path);
      await waitForPageReady(page);

      // Wait a bit for any charts/graphs to render
      await page.waitForTimeout(500);

      await captureScreenshot(page, name);

      // Capture tabs for Server Settings page (Custom Metadata tab)
      if (path === "/settings/server") {
        const customMetadataTab = await page.$('[role="tab"]:has-text("Custom Metadata"), button:has-text("Custom Metadata")');
        if (customMetadataTab) {
          await customMetadataTab.click();
          await waitForPageReady(page);
          await page.waitForTimeout(500);

          // Select an example template to show in the screenshot
          const chooseTemplateButton = await page.$('button:has-text("Choose Example Template")');
          if (chooseTemplateButton) {
            await chooseTemplateButton.click();
            await page.waitForTimeout(500);

            // Wait for modal to open with "Example Templates" title
            await page.waitForSelector('.mantine-Modal-content, [role="dialog"]', { state: "visible", timeout: 5000 });

            // Capture the template selection modal
            await captureScreenshot(page, "30-settings-server-custom-metadata-templates");

            // Click on a template card to select it (first Card element in the modal Grid)
            // The cards are rendered inside a Grid within the Modal
            const templateCard = await page.$('.mantine-Modal-body .mantine-Card-root, .mantine-Modal-content .mantine-Card-root');
            if (templateCard) {
              await templateCard.click();
              await page.waitForTimeout(300);
            } else {
              console.log("      ⚠️  No template card found in modal");
            }

            // Click "Use Template" button to apply the selected template
            const useTemplateButton = await page.$('.mantine-Modal-content button:has-text("Use Template"), [role="dialog"] button:has-text("Use Template")');
            if (useTemplateButton) {
              await useTemplateButton.click();
              await waitForPageReady(page);
              await page.waitForTimeout(500);
            } else {
              // Close modal if button not found
              console.log("      ⚠️  Use Template button not found, closing modal");
              await page.keyboard.press("Escape");
              await page.waitForTimeout(300);
            }
          } else {
            console.log("      ⚠️  Choose Example Template button not found");
          }

          // Wait for any modal to close
          await page.waitForTimeout(300);

          // Capture with template applied (shows editor with template code and preview)
          await captureScreenshot(page, "30-settings-server-custom-metadata");
        } else {
          console.log("      ⚠️  Custom Metadata tab not found");
        }
      }

      // Capture tabs for Metrics page
      if (path === "/settings/metrics") {
        // The default tab is "Inventory" which was just captured as 32-settings-metrics
        // Now switch to Task Performance tab
        const tasksTab = await page.$('[role="tab"]:has-text("Task Performance"), button:has-text("Task Performance")');
        if (tasksTab) {
          await tasksTab.click();
          await waitForPageReady(page);
          await page.waitForTimeout(500);
          await captureScreenshot(page, "32-settings-metrics-tasks");
        } else {
          console.log("      ⚠️  Task Performance tab not found");
        }
      }
    } catch (error) {
      console.log(`    ⚠️  Failed to capture ${label}: ${error}`);
    }
  }

  // Capture profile page with different tabs if available
  await page.goto("/settings/profile");
  await waitForPageReady(page);

  // Try to capture API Keys tab
  const apiKeysTab = await page.$('button:has-text("API Keys"), [role="tab"]:has-text("API Keys")');
  if (apiKeysTab) {
    await apiKeysTab.click();
    await waitForPageReady(page);
    await page.waitForTimeout(300);
    await captureScreenshot(page, "40-settings-profile-api-keys");
  }

  // Try to capture Preferences tab
  const preferencesTab = await page.$('button:has-text("Preferences"), [role="tab"]:has-text("Preferences")');
  if (preferencesTab) {
    await preferencesTab.click();
    await waitForPageReady(page);
    await page.waitForTimeout(300);
    await captureScreenshot(page, "41-settings-profile-preferences");
  }
}
