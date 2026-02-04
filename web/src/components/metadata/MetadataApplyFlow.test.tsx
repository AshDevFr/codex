import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PluginActionDto } from "@/api/plugins";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { MetadataApplyFlow } from "./MetadataApplyFlow";

// Mock the child components and API
vi.mock("@/api/plugins", () => ({
  pluginsApi: {
    searchMetadata: vi.fn(),
  },
  pluginActionsApi: {
    previewSeriesMetadata: vi.fn(),
    applySeriesMetadata: vi.fn(),
  },
}));

import { pluginActionsApi, pluginsApi } from "@/api/plugins";

const mockPlugin: PluginActionDto = {
  pluginId: "test-plugin-id",
  pluginName: "test-plugin",
  pluginDisplayName: "Test Plugin",
  actionType: "metadata_search",
  label: "Search Test Plugin",
};

const mockSearchResults = {
  success: true,
  result: {
    results: [
      {
        externalId: "ext-1",
        title: "Test Series",
        alternateTitles: [],
        year: 2024,
        coverUrl: null,
        relevanceScore: 0.95,
        preview: null,
      },
    ],
  },
  latencyMs: 150,
};

const mockPreviewResponse = {
  fields: [
    {
      field: "title",
      currentValue: "Old Title",
      proposedValue: "New Title",
      status: "will_apply",
    },
  ],
  summary: {
    willApply: 1,
    locked: 0,
    noPermission: 0,
    unchanged: 0,
    notProvided: 0,
  },
  pluginId: "test-plugin-id",
  pluginName: "Test Plugin",
  externalId: "ext-1",
};

describe("MetadataApplyFlow", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    (pluginsApi.searchMetadata as ReturnType<typeof vi.fn>).mockResolvedValue(
      mockSearchResults,
    );
    (
      pluginActionsApi.previewSeriesMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue(mockPreviewResponse);
    (
      pluginActionsApi.applySeriesMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue({
      success: true,
      appliedFields: ["title"],
      skippedFields: [],
      message: "Applied 1 field",
    });
  });

  it("starts in search step showing search modal", () => {
    renderWithProviders(
      <MetadataApplyFlow
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        entityId="test-series-id"
        entityTitle="My Series"
      />,
    );

    // Should show search modal with plugin name
    expect(screen.getByText("Search Test Plugin")).toBeInTheDocument();
  });

  it("pre-fills search with entity title", () => {
    renderWithProviders(
      <MetadataApplyFlow
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        entityId="test-series-id"
        entityTitle="My Series"
      />,
    );

    expect(screen.getByDisplayValue("My Series")).toBeInTheDocument();
  });

  it("does not render when closed", () => {
    renderWithProviders(
      <MetadataApplyFlow
        opened={false}
        onClose={onClose}
        plugin={mockPlugin}
        entityId="test-series-id"
        entityTitle="My Series"
      />,
    );

    expect(screen.queryByText("Search Test Plugin")).not.toBeInTheDocument();
  });

  it("uses series content type by default", async () => {
    renderWithProviders(
      <MetadataApplyFlow
        opened={true}
        onClose={onClose}
        plugin={mockPlugin}
        entityId="test-series-id"
        entityTitle="My Series"
      />,
    );

    // Trigger a search by waiting for debounce
    await waitFor(
      () => {
        expect(pluginsApi.searchMetadata).toHaveBeenCalledWith(
          "test-plugin-id",
          "My Series",
          "series",
          undefined,
        );
      },
      { timeout: 1000 },
    );
  });

  // TODO: Add test for "book" content type when it's supported by the API
});
