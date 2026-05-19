import type React from "react";
import { describe, expect, it, vi } from "vitest";
import type { ReleaseLedgerEntry, ReleaseSource } from "@/api/releases";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { ReleasesTable } from "./ReleasesTable";

// -----------------------------------------------------------------------------
// `formatChapterVolume` is internal but its behavior is the user-visible
// difference between "Vol 1" (the old, lying display) and "Vol 1-9, 11"
// (the truth for a compilation torrent). We exercise it via the rendered
// "Ch / Vol" cell so the tests describe the contract the UI presents.
// -----------------------------------------------------------------------------

const SOURCE: ReleaseSource = {
  id: "11111111-1111-1111-1111-111111111111",
  pluginId: "release-nyaa",
  sourceKey: "user:1r0n",
  displayName: "Nyaa: 1r0n",
  kind: "rss-uploader",
  enabled: true,
  cronSchedule: null,
  config: null,
  etag: null,
  lastPolledAt: null,
  lastError: null,
  lastErrorAt: null,
  lastSummary: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
} as unknown as ReleaseSource;

function entry(overrides: Partial<ReleaseLedgerEntry>): ReleaseLedgerEntry {
  return {
    id: "ent-1",
    seriesId: "00000000-0000-0000-0000-000000000001",
    seriesTitle: "Test Series",
    sourceId: SOURCE.id,
    externalReleaseId: "ext-1",
    payloadUrl: "https://nyaa.si/view/1",
    confidence: 0.95,
    state: "announced",
    observedAt: "2026-05-01T00:00:00Z",
    createdAt: "2026-05-01T00:00:00Z",
    chapters: null,
    volumes: null,
    language: "en",
    groupOrUploader: "1r0n",
    ...overrides,
  } as ReleaseLedgerEntry;
}

function renderRow(
  e: ReleaseLedgerEntry,
  overrides: Partial<React.ComponentProps<typeof ReleasesTable>> = {},
) {
  return renderWithProviders(
    <ReleasesTable
      entries={[e]}
      sourceById={new Map([[SOURCE.id, SOURCE]])}
      selected={new Set()}
      onToggleOne={vi.fn()}
      onToggleAll={vi.fn()}
      onDismiss={vi.fn()}
      onMarkAcquired={vi.fn()}
      onIgnore={vi.fn()}
      onReset={vi.fn()}
      onDelete={vi.fn()}
      {...overrides}
    />,
  );
}

describe("ReleasesTable Ch / Vol formatting", () => {
  it("renders a dash when neither axis has spans", () => {
    renderRow(entry({}));
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("renders a single-point chapter span as `Ch N`", () => {
    renderRow(entry({ chapters: [{ start: 142, end: 142 }] }));
    expect(screen.getByText("Ch 142")).toBeInTheDocument();
  });

  it("renders a single-point volume span as `Vol N`", () => {
    renderRow(entry({ volumes: [{ start: 13, end: 13 }] }));
    expect(screen.getByText("Vol 13")).toBeInTheDocument();
  });

  it("renders a chapter range as `Ch start-end`", () => {
    renderRow(entry({ chapters: [{ start: 126, end: 142 }] }));
    expect(screen.getByText("Ch 126-142")).toBeInTheDocument();
  });

  it("renders a volume range as `Vol start-end`", () => {
    renderRow(entry({ volumes: [{ start: 1, end: 9 }] }));
    expect(screen.getByText("Vol 1-9")).toBeInTheDocument();
  });

  it("renders both axes with a separator (`001-050 as v01-10`)", () => {
    renderRow(
      entry({
        chapters: [{ start: 1, end: 50 }],
        volumes: [{ start: 1, end: 10 }],
      }),
    );
    expect(screen.getByText("Ch 1-50 · Vol 1-10")).toBeInTheDocument();
  });

  it("preserves the gap in a disjoint volume bundle (`v01-04 + v06-09`)", () => {
    renderRow(
      entry({
        volumes: [
          { start: 1, end: 4 },
          { start: 6, end: 9 },
        ],
      }),
    );
    expect(screen.getByText("Vol 1-4, 6-9")).toBeInTheDocument();
  });

  it("renders the Charlotte mixed bundle honestly (single vol + chapter pair)", () => {
    // `001-005 as v01 + 006-009`: one volume span + two chapter spans.
    renderRow(
      entry({
        chapters: [
          { start: 1, end: 5 },
          { start: 6, end: 9 },
        ],
        volumes: [{ start: 1, end: 1 }],
      }),
    );
    expect(screen.getByText("Ch 1-5, 6-9 · Vol 1")).toBeInTheDocument();
  });

  it("preserves decimal chapters in single-point spans", () => {
    renderRow(entry({ chapters: [{ start: 12.5, end: 12.5 }] }));
    expect(screen.getByText("Ch 12.5")).toBeInTheDocument();
  });

  it("treats an empty span list as no info (renders dash)", () => {
    renderRow(entry({ chapters: [], volumes: [] }));
    expect(screen.getByText("—")).toBeInTheDocument();
  });
});

describe("ReleasesTable row actions", () => {
  it("shows Mark acquired / Dismiss / Ignore on announced rows, hides Reset", () => {
    renderRow(entry({ state: "announced" }));
    expect(screen.getByLabelText("Mark acquired")).toBeInTheDocument();
    expect(screen.getByLabelText("Dismiss")).toBeInTheDocument();
    expect(screen.getByLabelText("Ignore")).toBeInTheDocument();
    expect(screen.queryByLabelText("Reset")).not.toBeInTheDocument();
  });

  it("shows Reset on non-announced rows and hides the announced-only actions", () => {
    renderRow(entry({ state: "dismissed" }));
    expect(screen.getByLabelText("Reset")).toBeInTheDocument();
    expect(screen.queryByLabelText("Mark acquired")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Dismiss")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Ignore")).not.toBeInTheDocument();
  });

  it("renders the Ignored state badge instead of the raw state string", () => {
    renderRow(entry({ state: "ignored" }));
    expect(screen.getByText("Ignored")).toBeInTheDocument();
  });

  it("invokes onIgnore with the row id when the Ignore icon is clicked", async () => {
    const onIgnore = vi.fn();
    renderRow(entry({ id: "row-42", state: "announced" }), { onIgnore });
    await userEvent.setup().click(screen.getByLabelText("Ignore"));
    expect(onIgnore).toHaveBeenCalledWith("row-42");
  });

  it("invokes onReset with the row id when the Reset icon is clicked", async () => {
    const onReset = vi.fn();
    renderRow(entry({ id: "row-99", state: "marked_acquired" }), { onReset });
    await userEvent.setup().click(screen.getByLabelText("Reset"));
    expect(onReset).toHaveBeenCalledWith("row-99");
  });
});
