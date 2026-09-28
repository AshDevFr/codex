import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ImportReadingProgressResponse,
  ImportSummary,
  ReadingProgressExportDocument,
} from "@/api/readingProgressTransfer";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import {
  ReadingProgressTransferSettings,
  SummaryLine,
} from "./ReadingProgressTransferSettings";

const exportProgress = vi.fn();
const importProgress = vi.fn();

vi.mock("@/api/readingProgressTransfer", async () => {
  const actual = await vi.importActual<
    typeof import("@/api/readingProgressTransfer")
  >("@/api/readingProgressTransfer");
  return {
    ...actual,
    readingProgressTransferApi: {
      exportProgress: (...args: unknown[]) => exportProgress(...args),
      importProgress: (...args: unknown[]) => importProgress(...args),
    },
  };
});

const exportDocument: ReadingProgressExportDocument = {
  format: "codex-reading-progress",
  version: 1,
  exportedAt: "2026-09-27T00:00:00Z",
  includesSessions: true,
  series: [
    {
      externalIds: [],
      libraryRelativePath: "Naruto",
      name: "Naruto",
      books: [
        {
          path: "v01.cbz",
          fileName: "v01.cbz",
          fileHash: "",
          partialHash: "",
          completions: [],
        },
      ],
    },
  ],
};

/**
 * Builds a summary with every field present. Test files are excluded from
 * `tsc -b`, so a stale key here would not fail the type check: this fixture
 * sat in snake_case for a release after the wire format became camelCase,
 * and passed only because nothing asserted a rendered count. The tests below
 * assert rendered numbers, which is what catches that at run time.
 */
function summary(overrides: Partial<ImportSummary> = {}): ImportSummary {
  return {
    seriesTotal: 1,
    seriesMatched: 1,
    seriesAmbiguous: 0,
    seriesUnmatched: 0,
    seriesCommitted: 0,
    booksTotal: 1,
    booksMatched: 1,
    booksStemMatched: 0,
    booksAmbiguous: 0,
    booksUnmatched: 0,
    booksHashMismatch: 0,
    progressWritten: 1,
    ratingsWritten: 0,
    completionsInserted: 0,
    completionsReattached: 0,
    sessionsInserted: 0,
    sessionsReattached: 0,
    rowsStranded: 0,
    seriesMatchedDistinct: 1,
    booksMatchedDistinct: 1,
    seriesInSelectedLibraries: null,
    booksInSelectedLibraries: null,
    booksInUnmatchedSeries: 0,
    wantToReadRestored: 0,
    ...overrides,
  };
}

function dryRunResponse(
  overrides: Partial<ImportSummary> = {},
): ImportReadingProgressResponse {
  return {
    dryRun: true,
    sessionsInFile: true,
    notices: [],
    summary: summary(overrides),
    series: [
      {
        libraryRelativePath: "Naruto",
        name: "Naruto",
        disposition: "matched",
        matchedSeriesId: "11111111-1111-1111-1111-111111111111",
        attempted: true,
        committed: false,
        books: [
          {
            path: "v01.cbz",
            fileName: "v01.cbz",
            disposition: "matched",
            matchedBookId: "22222222-2222-2222-2222-222222222222",
            applied: true,
            progress: "inserted",
            completions: {
              inserted: 0,
              reattached: 0,
              skipped: 0,
              stranded: 0,
            },
            sessions: { inserted: 0, reattached: 0, skipped: 0, stranded: 0 },
          },
        ],
      },
    ],
  };
}

async function uploadDocument(user: ReturnType<typeof userEvent.setup>) {
  const file = new File([JSON.stringify(exportDocument)], "export.json", {
    type: "application/json",
  });
  const input = fileInput();
  await user.upload(input, file);
  // The button label updates as soon as the file is selected, but parsing
  // (`FileReader`) is async; wait for the preview button to actually unlock
  // rather than the label, or a click can race ahead of the parsed document.
  await waitFor(() => {
    expect(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    ).toBeEnabled();
  });
}

// The FileButton render prop wraps a hidden native file input with no
// accessible label, so it is queried directly rather than by role/label.
function fileInput(): HTMLInputElement {
  const input = window.document.querySelector('input[type="file"]');
  if (!input) throw new Error("file input not found");
  return input as HTMLInputElement;
}

describe("ReadingProgressTransferSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("downloads an export when the button is clicked", async () => {
    exportProgress.mockResolvedValue(exportDocument);
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await user.click(screen.getByRole("button", { name: /download export/i }));

    await waitFor(() => {
      // No library selected means every library, which the client sends as
      // an empty list rather than a libraryIds parameter.
      expect(exportProgress).toHaveBeenCalledWith(true, []);
    });
  });

  it("disables Apply until a dry run has been previewed for the selected file", async () => {
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);

    const applyButton = await screen.findByRole("button", {
      name: /apply import/i,
    });
    expect(applyButton).toBeDisabled();

    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    await waitFor(() => {
      expect(applyButton).toBeEnabled();
    });
    expect(importProgress).toHaveBeenCalledWith(
      expect.objectContaining({ dryRun: true }),
    );
  });

  it("omits sourcePreference rather than sending an empty list", async () => {
    // An empty list means "skip external ids" to the server, and an id is the
    // only matching key that survives a rename. Sending [] here silently
    // downgraded every import to path and name matching.
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    await waitFor(() => {
      expect(importProgress).toHaveBeenCalled();
    });
    const request = importProgress.mock.calls[0][0];
    expect(request.sourcePreference).toBeUndefined();
    expect(request.libraryIds).toBeUndefined();
  });

  it("re-locks Apply when an option changes after the preview", async () => {
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    const applyButton = await screen.findByRole("button", {
      name: /apply import/i,
    });
    await waitFor(() => expect(applyButton).toBeEnabled());

    await user.click(
      screen.getByRole("checkbox", { name: /accept filename-stem matches/i }),
    );

    expect(applyButton).toBeDisabled();
  });

  it("renders the report table with per-series and per-book disposition", async () => {
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    expect((await screen.findAllByText("Naruto")).length).toBeGreaterThan(0);
    expect(screen.getByText("v01.cbz")).toBeInTheDocument();
    expect(screen.getAllByText("matched").length).toBeGreaterThan(0);
  });

  it("calls import with dryRun: false when Apply is pressed", async () => {
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    const applyButton = await screen.findByRole("button", {
      name: /apply import/i,
    });
    await waitFor(() => expect(applyButton).toBeEnabled());

    importProgress.mockResolvedValue({
      ...dryRunResponse(),
      dryRun: false,
      series: [
        {
          ...dryRunResponse().series[0],
          committed: true,
        },
      ],
    });
    await user.click(applyButton);

    await waitFor(() => {
      expect(importProgress).toHaveBeenLastCalledWith(
        expect.objectContaining({ dryRun: false }),
      );
    });
  });

  /// Options cannot change under a dry run in flight, or its result would
  /// unlock Apply for options it never previewed.
  it("locks the options while a dry run is in flight", async () => {
    let finish: (value: ImportReadingProgressResponse) => void = () => {};
    importProgress.mockReturnValue(
      new Promise<ImportReadingProgressResponse>((resolve) => {
        finish = resolve;
      }),
    );
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );

    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: /accept filename-stem matches/i }),
      ).toBeDisabled(),
    );

    finish(dryRunResponse());
    await waitFor(() =>
      expect(
        screen.getByRole("checkbox", { name: /accept filename-stem matches/i }),
      ).toBeEnabled(),
    );
    expect(screen.getByRole("button", { name: /apply import/i })).toBeEnabled();
  });

  /// A failed apply may have committed some series, so the old preview no
  /// longer describes what a second apply would do.
  it("relocks Apply after an apply fails", async () => {
    importProgress.mockResolvedValue(dryRunResponse());
    const user = userEvent.setup();
    renderWithProviders(<ReadingProgressTransferSettings />);

    await uploadDocument(user);
    await user.click(
      screen.getByRole("button", { name: /preview \(dry run\)/i }),
    );
    const applyButton = await screen.findByRole("button", {
      name: /apply import/i,
    });
    await waitFor(() => expect(applyButton).toBeEnabled());

    importProgress.mockRejectedValue(
      Object.assign(new Error("Request failed"), {
        response: { status: 413 },
      }),
    );
    await user.click(applyButton);

    expect(
      await screen.findByText(/larger than the server accepts/i),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /apply import/i }),
    ).toBeDisabled();
  });

  describe("report wording", () => {
    // The shape of the first real import: the file covered far more than the
    // destination, and every destination series matched.
    const scoped = dryRunResponse({
      seriesMatched: 32,
      seriesMatchedDistinct: 32,
      seriesUnmatched: 244,
      seriesInSelectedLibraries: 32,
      booksMatched: 1838,
      booksMatchedDistinct: 1838,
      booksUnmatched: 3183,
      booksInUnmatchedSeries: 3121,
      booksInSelectedLibraries: 1900,
    });

    it("states a scoped import as coverage of the destination", () => {
      renderWithProviders(<SummaryLine report={scoped} scopeLabel="Shonen" />);
      const text = document.body.textContent ?? "";

      expect(text).toContain("Series: 32 of the 32 in Shonen matched");
      expect(text).toContain("Books: 1838 of the 1900 in Shonen matched");
      expect(text).toContain(
        "244 series (3121 books) that belong to other libraries",
      );
      expect(text).not.toMatch(/unmatched/);
    });

    it("keeps books missed inside a matched series visible", () => {
      // 3183 unmatched, 3121 of them in series that live elsewhere: the other
      // 62 were missed inside series that did match, and must not be folded
      // into the reassuring note.
      renderWithProviders(<SummaryLine report={scoped} scopeLabel="Shonen" />);

      expect(document.body.textContent).toContain("62 missed");
    });

    it("keeps the plain counts when the import was not scoped", () => {
      const unscoped = dryRunResponse({ seriesUnmatched: 4 });
      renderWithProviders(<SummaryLine report={unscoped} scopeLabel={null} />);
      const text = document.body.textContent ?? "";

      expect(text).toContain("4 unmatched");
      expect(text).not.toMatch(/belong to other libraries/);
    });
  });

  describe("restoring want to read", () => {
    it("is requested by default", async () => {
      importProgress.mockResolvedValue(dryRunResponse());
      const user = userEvent.setup();
      renderWithProviders(<ReadingProgressTransferSettings />);

      await uploadDocument(user);
      await user.click(
        screen.getByRole("button", { name: /preview \(dry run\)/i }),
      );

      await waitFor(() => expect(importProgress).toHaveBeenCalled());
      expect(importProgress.mock.calls[0][0].restoreWantToRead).toBe(true);
    });

    it("can be switched off", async () => {
      importProgress.mockResolvedValue(dryRunResponse());
      const user = userEvent.setup();
      renderWithProviders(<ReadingProgressTransferSettings />);

      await uploadDocument(user);
      await user.click(
        screen.getByRole("checkbox", { name: /restore want to read/i }),
      );
      await user.click(
        screen.getByRole("button", { name: /preview \(dry run\)/i }),
      );

      await waitFor(() => expect(importProgress).toHaveBeenCalled());
      expect(importProgress.mock.calls[0][0].restoreWantToRead).toBe(false);
    });
  });
});
