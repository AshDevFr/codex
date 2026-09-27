import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ImportReadingProgressResponse,
  ReadingProgressExportDocument,
} from "@/api/readingProgressTransfer";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { ReadingProgressTransferSettings } from "./ReadingProgressTransferSettings";

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
  exported_at: "2026-09-27T00:00:00Z",
  includes_sessions: true,
  series: [
    {
      external_ids: [],
      library_relative_path: "Naruto",
      name: "Naruto",
      books: [
        {
          path: "v01.cbz",
          file_name: "v01.cbz",
          file_hash: "",
          partial_hash: "",
          completions: [],
        },
      ],
    },
  ],
};

function dryRunResponse(): ImportReadingProgressResponse {
  return {
    dry_run: true,
    sessions_in_file: true,
    notices: [],
    summary: {
      series_total: 1,
      series_matched: 1,
      series_ambiguous: 0,
      series_unmatched: 0,
      series_committed: 0,
      books_total: 1,
      books_matched: 1,
      books_stem_matched: 0,
      books_ambiguous: 0,
      books_unmatched: 0,
      books_hash_mismatch: 0,
      progress_written: 1,
      ratings_written: 0,
      completions_inserted: 0,
      completions_reattached: 0,
      sessions_inserted: 0,
      sessions_reattached: 0,
    },
    series: [
      {
        library_relative_path: "Naruto",
        name: "Naruto",
        disposition: "matched",
        matched_series_id: "11111111-1111-1111-1111-111111111111",
        attempted: true,
        committed: false,
        books: [
          {
            path: "v01.cbz",
            file_name: "v01.cbz",
            disposition: "matched",
            matched_book_id: "22222222-2222-2222-2222-222222222222",
            applied: true,
            progress: "inserted",
            completions: { inserted: 0, reattached: 0, skipped: 0 },
            sessions: { inserted: 0, reattached: 0, skipped: 0 },
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
      expect(exportProgress).toHaveBeenCalledWith(true);
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
      expect.objectContaining({ dry_run: true }),
    );
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

  it("calls import with dry_run: false when Apply is pressed", async () => {
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
      dry_run: false,
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
        expect.objectContaining({ dry_run: false }),
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
});
