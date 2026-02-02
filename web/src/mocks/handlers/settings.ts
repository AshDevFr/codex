/**
 * MSW handlers for settings API endpoints
 */

import { delay, HttpResponse, http } from "msw";
import {
  createList,
  createSetting,
  createSettingHistory,
} from "../data/factories";

// Generate mock settings data matching the actual database seed
const mockSettings = [
  // Scanner settings
  createSetting({
    key: "scanner.scan_timeout_minutes",
    value: "120",
    valueType: "integer",
    category: "Scanner",
    description: "Maximum time (in minutes) for a single scan before timeout",
    defaultValue: "120",
  }),
  createSetting({
    key: "scanner.retry_failed_files",
    value: "false",
    valueType: "boolean",
    category: "Scanner",
    description: "Automatically retry files that failed to scan",
    defaultValue: "false",
  }),
  createSetting({
    key: "scanner.batch_size",
    value: "100",
    valueType: "integer",
    category: "Scanner",
    description:
      "Number of files to process in each batch during library scanning",
    defaultValue: "100",
  }),
  createSetting({
    key: "scanner.parallel_hashing",
    value: "8",
    valueType: "integer",
    category: "Scanner",
    description: "Number of files to hash concurrently during scanning",
    defaultValue: "8",
  }),
  createSetting({
    key: "scanner.parallel_series",
    value: "4",
    valueType: "integer",
    category: "Scanner",
    description: "Number of series to process concurrently during scanning",
    defaultValue: "4",
  }),
  // Application settings
  createSetting({
    key: "application.name",
    value: "Codex - Mock",
    valueType: "string",
    category: "Application",
    description: "Application display name (for branding/white-labeling)",
    defaultValue: "Codex - Mock",
  }),
  // Authentication settings
  createSetting({
    key: "auth.registration_enabled",
    value: "false",
    valueType: "boolean",
    category: "Authentication",
    description: "Allow new users to register accounts",
    defaultValue: "false",
  }),
  // Task settings
  createSetting({
    key: "task.poll_interval_seconds",
    value: "5",
    valueType: "integer",
    category: "Task",
    description: "Interval (in seconds) for polling task queue",
    defaultValue: "5",
  }),
  createSetting({
    key: "task.cleanup_interval_seconds",
    value: "30",
    valueType: "integer",
    category: "Task",
    description: "Interval (in seconds) for cleaning up completed tasks",
    defaultValue: "30",
  }),
  createSetting({
    key: "task.prioritize_scans_over_analysis",
    value: "true",
    valueType: "boolean",
    category: "Task",
    description: "Prioritize scan tasks over analysis tasks in the queue",
    defaultValue: "true",
  }),
  // Deduplication settings
  createSetting({
    key: "deduplication.enabled",
    value: "true",
    valueType: "boolean",
    category: "Deduplication",
    description: "Enable automatic duplicate detection scanning",
    defaultValue: "true",
  }),
  createSetting({
    key: "deduplication.cron_schedule",
    value: "",
    valueType: "string",
    category: "Deduplication",
    description: "Cron schedule for automatic duplicate detection",
    defaultValue: "",
  }),
  // Purge settings
  createSetting({
    key: "purge.purge_empty_series",
    value: "true",
    valueType: "boolean",
    category: "Purge",
    description: "When purging deleted books, also delete empty series",
    defaultValue: "true",
  }),
  // Thumbnail settings
  createSetting({
    key: "thumbnail.max_dimension",
    value: "400",
    valueType: "integer",
    category: "Thumbnail",
    description: "Maximum width or height for generated thumbnails",
    defaultValue: "400",
  }),
  createSetting({
    key: "thumbnail.jpeg_quality",
    value: "85",
    valueType: "integer",
    category: "Thumbnail",
    description: "JPEG quality for thumbnail images (1-100)",
    defaultValue: "85",
  }),
  // Display settings
  createSetting({
    key: "display.custom_metadata_template",
    value: `{{#if metadata.genres}}
**Genres:** {{join metadata.genres " • "}}
{{/if}}

{{#if custom_metadata}}
## Additional Information

{{#each custom_metadata}}
- **{{@key}}**: {{this}}
{{/each}}
{{/if}}`,
    valueType: "string",
    category: "Display",
    description:
      "Handlebars-style Markdown template for displaying custom metadata on series detail pages.",
    defaultValue: "",
  }),
];

export const settingsHandlers = [
  // ============================================
  // Branding Settings (unauthenticated)
  // ============================================

  // Get branding settings (unauthenticated - used on login page)
  http.get("/api/v1/settings/branding", async () => {
    await delay(50);
    const appNameSetting = mockSettings.find(
      (s) => s.key === "application.name",
    );
    return HttpResponse.json({
      applicationName: appNameSetting?.value || "Codex",
    });
  }),

  // ============================================
  // Admin Settings
  // ============================================

  // List all settings
  http.get("/api/v1/admin/settings", async ({ request }) => {
    await delay(100);
    const url = new URL(request.url);
    const category = url.searchParams.get("category");

    let filteredSettings = mockSettings;
    if (category) {
      filteredSettings = mockSettings.filter((s) => s.category === category);
    }

    return HttpResponse.json(filteredSettings);
  }),

  // Get single setting
  http.get("/api/v1/admin/settings/:settingKey", async ({ params }) => {
    await delay(50);
    const { settingKey } = params;
    const setting = mockSettings.find((s) => s.key === settingKey);

    if (!setting) {
      return new HttpResponse(null, { status: 404 });
    }

    return HttpResponse.json(setting);
  }),

  // Update setting
  http.put(
    "/api/v1/admin/settings/:settingKey",
    async ({ params, request }) => {
      await delay(100);
      const { settingKey } = params;
      const body = (await request.json()) as { value: string };
      const settingIndex = mockSettings.findIndex((s) => s.key === settingKey);

      if (settingIndex === -1) {
        return new HttpResponse(null, { status: 404 });
      }

      mockSettings[settingIndex] = {
        ...mockSettings[settingIndex],
        value: body.value,
        updatedAt: new Date().toISOString(),
        version: mockSettings[settingIndex].version + 1,
      };

      return HttpResponse.json(mockSettings[settingIndex]);
    },
  ),

  // Reset setting to default
  http.post("/api/v1/admin/settings/:settingKey/reset", async ({ params }) => {
    await delay(100);
    const { settingKey } = params;
    const settingIndex = mockSettings.findIndex((s) => s.key === settingKey);

    if (settingIndex === -1) {
      return new HttpResponse(null, { status: 404 });
    }

    mockSettings[settingIndex] = {
      ...mockSettings[settingIndex],
      value: mockSettings[settingIndex].defaultValue,
      updatedAt: new Date().toISOString(),
      version: mockSettings[settingIndex].version + 1,
    };

    return HttpResponse.json(mockSettings[settingIndex]);
  }),

  // Bulk update settings
  http.post("/api/v1/admin/settings/bulk", async ({ request }) => {
    await delay(150);
    const body = (await request.json()) as {
      settings: Array<{ key: string; value: string }>;
    };

    const updatedSettings = body.settings
      .map((update) => {
        const settingIndex = mockSettings.findIndex(
          (s) => s.key === update.key,
        );
        if (settingIndex !== -1) {
          mockSettings[settingIndex] = {
            ...mockSettings[settingIndex],
            value: update.value,
            updatedAt: new Date().toISOString(),
            version: mockSettings[settingIndex].version + 1,
          };
          return mockSettings[settingIndex];
        }
        return null;
      })
      .filter(Boolean);

    return HttpResponse.json(updatedSettings);
  }),

  // Get setting history
  http.get("/api/v1/admin/settings/:settingKey/history", async ({ params }) => {
    await delay(100);
    const { settingKey } = params;

    const history = createList(
      () => createSettingHistory({ key: settingKey as string }),
      5,
    );

    return HttpResponse.json(history);
  }),

  // ============================================
  // Public Settings (non-admin)
  // ============================================

  // Get public settings (accessible to all authenticated users)
  http.get("/api/v1/settings/public", async () => {
    await delay(50);
    // Return a subset of settings that are safe for non-admin users
    const publicSettings = {
      applicationName:
        mockSettings.find((s) => s.key === "application.name")?.value ||
        "Codex",
      registrationEnabled:
        mockSettings.find((s) => s.key === "auth.registration_enabled")
          ?.value === "true",
      version: "1.0.0",
    };
    return HttpResponse.json(publicSettings);
  }),
];
