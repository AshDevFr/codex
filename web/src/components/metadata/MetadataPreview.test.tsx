import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MetadataPreviewResponse } from "@/api/plugins";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { MetadataPreview } from "./MetadataPreview";

// Mock the plugins API
vi.mock("@/api/plugins", () => ({
  pluginActionsApi: {
    previewSeriesMetadata: vi.fn(),
    applySeriesMetadata: vi.fn(),
    previewBookMetadata: vi.fn(),
    applyBookMetadata: vi.fn(),
  },
}));

import { pluginActionsApi } from "@/api/plugins";

const mockPreviewResponse: MetadataPreviewResponse = {
  fields: [
    {
      field: "title",
      currentValue: "Old Title",
      proposedValue: "New Title",
      status: "will_apply",
    },
    {
      field: "summary",
      currentValue: "Old summary",
      proposedValue: "New summary",
      status: "locked",
      reason: "Field is locked by user",
    },
    {
      field: "genres",
      currentValue: ["Action"],
      proposedValue: ["Action", "Adventure"],
      status: "will_apply",
    },
    {
      field: "year",
      currentValue: 2023,
      proposedValue: 2023,
      status: "unchanged",
    },
    {
      field: "publisher",
      currentValue: null,
      proposedValue: null,
      status: "not_provided",
    },
  ],
  summary: {
    willApply: 2,
    locked: 1,
    noPermission: 0,
    unchanged: 1,
    notProvided: 1,
  },
  pluginId: "test-plugin-id",
  pluginName: "Test Plugin",
  externalId: "ext-123",
  externalUrl: "https://example.com/series/ext-123",
};

describe("MetadataPreview", () => {
  const onApplyComplete = vi.fn();
  const onBack = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    (
      pluginActionsApi.previewSeriesMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue(mockPreviewResponse);
    (
      pluginActionsApi.applySeriesMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue({
      success: true,
      appliedFields: ["title", "genres"],
      skippedFields: [],
      message: "Applied 2 fields",
    });
  });

  it("shows loading state initially", async () => {
    // Make the preview take a while to resolve
    (
      pluginActionsApi.previewSeriesMetadata as ReturnType<typeof vi.fn>
    ).mockImplementation(
      () =>
        new Promise((resolve) =>
          setTimeout(() => resolve(mockPreviewResponse), 100),
        ),
    );

    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    // Should show loading state while waiting
    await waitFor(() => {
      expect(
        screen.getByText("Fetching metadata from Test Plugin..."),
      ).toBeInTheDocument();
    });
  });

  it("displays field preview table after loading", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Title")).toBeInTheDocument();
    });

    expect(screen.getByText("Summary")).toBeInTheDocument();
    expect(screen.getByText("Genres")).toBeInTheDocument();
    expect(screen.getByText("Year")).toBeInTheDocument();
  });

  it("shows current and proposed values", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Old Title")).toBeInTheDocument();
    });

    expect(screen.getByText("New Title")).toBeInTheDocument();
  });

  it("displays summary badges", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("2 to apply")).toBeInTheDocument();
    });

    expect(screen.getByText("1 locked")).toBeInTheDocument();
    // Note: unchanged/notProvided fields don't get summary badges
  });

  it("shows apply button with field count", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Apply 2 Fields")).toBeInTheDocument();
    });
  });

  it("shows back button when onBack is provided", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
        onBack={onBack}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Back to Search")).toBeInTheDocument();
    });
  });

  it("shows external link when available", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("View on Test Plugin →")).toBeInTheDocument();
    });
  });

  it("shows error state when preview fails", async () => {
    (
      pluginActionsApi.previewSeriesMetadata as ReturnType<typeof vi.fn>
    ).mockRejectedValue(new Error("Failed to fetch metadata"));

    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Failed to fetch metadata")).toBeInTheDocument();
    });

    expect(screen.getByText("Retry")).toBeInTheDocument();
  });

  it("disables apply button when no fields will be applied", async () => {
    const noChangesResponse: MetadataPreviewResponse = {
      fields: [
        {
          field: "title",
          currentValue: "Same Title",
          proposedValue: "Same Title",
          status: "unchanged",
        },
        {
          field: "summary",
          currentValue: "Old summary",
          proposedValue: "New summary",
          status: "locked",
          reason: "Field is locked by user",
        },
        {
          field: "year",
          currentValue: 2023,
          proposedValue: 2023,
          status: "unchanged",
        },
      ],
      summary: {
        willApply: 0,
        locked: 1,
        noPermission: 0,
        unchanged: 2,
        notProvided: 0,
      },
      pluginId: "test-plugin-id",
      pluginName: "Test Plugin",
      externalId: "ext-123",
      externalUrl: "https://example.com/series/ext-123",
    };

    (
      pluginActionsApi.previewSeriesMetadata as ReturnType<typeof vi.fn>
    ).mockResolvedValue(noChangesResponse);

    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("No Changes to Apply")).toBeInTheDocument();
    });
  });

  it("calls onApplyComplete when apply succeeds", async () => {
    renderWithProviders(
      <MetadataPreview
        seriesId="test-series-id"
        pluginId="test-plugin-id"
        externalId="ext-123"
        pluginName="Test Plugin"
        onApplyComplete={onApplyComplete}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Apply 2 Fields")).toBeInTheDocument();
    });

    // Click apply button
    const applyButton = screen.getByText("Apply 2 Fields");
    applyButton.click();

    await waitFor(() => {
      expect(onApplyComplete).toHaveBeenCalledWith(true, ["title", "genres"]);
    });
  });
});
