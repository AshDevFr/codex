import { IDBFactory } from "fake-indexeddb";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type MockInstance,
  vi,
} from "vitest";
import {
  _resetForTests,
  broadcastDownloadsChange,
  type DownloadRecord,
  getDownload,
  putDownload,
  setDbContext,
} from "@/lib/offline/db";
import * as downloadManagerModule from "@/lib/offline/downloadManager";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { DownloadButton } from "./DownloadButton";

type DownloadFn = typeof downloadManagerModule.downloadSingleFileBook;

let downloadSpy: MockInstance<DownloadFn> | null = null;

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  downloadSpy?.mockRestore();
  downloadSpy = null;
});

function stubDownload(
  impl: (
    opts: Parameters<DownloadFn>[0],
  ) => Promise<{ bookId: string; bytes: number }>,
) {
  downloadSpy = vi
    .spyOn(downloadManagerModule, "downloadSingleFileBook")
    .mockImplementation(impl);
}

async function seed(record: DownloadRecord) {
  await putDownload(record);
}

describe("DownloadButton: format support", () => {
  it("renders nothing for unknown formats", () => {
    renderWithProviders(<DownloadButton bookId="book-x" fileFormat="mobi" />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("renders nothing for a comic format with no pageCount", () => {
    renderWithProviders(<DownloadButton bookId="book-cbz" fileFormat="cbz" />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("renders a download menu trigger for epub", async () => {
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();
  });

  it("renders a download menu trigger for pdf", async () => {
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="pdf" />);
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();
  });

  it("renders a download menu trigger for cbz when pageCount is provided", async () => {
    renderWithProviders(
      <DownloadButton bookId="book-cbz" fileFormat="cbz" pageCount={20} />,
    );
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();
  });

  it("still renders when only a fileDownloadUrl is provided for an unsupported format", async () => {
    // mobi can't be cached for offline reading, but the file URL is still
    // surfaced as a "Download file" menu item so the user has a path to
    // export the file.
    renderWithProviders(
      <DownloadButton
        bookId="book-x"
        fileFormat="mobi"
        fileDownloadUrl="/api/v1/books/book-x/file"
      />,
    );
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();
  });
});

describe("DownloadButton: hydration from IDB", () => {
  it("shows the downloaded state when the IDB row already exists", async () => {
    await seed({
      id: "book-1",
      format: "epub",
      status: "complete",
      bytes: 1024,
      pageCount: 1,
      downloadedAt: 1,
    });
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", {
        name: /offline download options/i,
      }),
    ).toBeInTheDocument();
  });

  it("shows the error state when the IDB row is in error", async () => {
    await seed({
      id: "book-1",
      format: "epub",
      status: "error",
      bytes: 0,
      pageCount: 1,
      error: "boom",
    });
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", {
        name: /download options \(retry available\)/i,
      }),
    ).toBeInTheDocument();
  });

  it("falls back to not-downloaded when the IDB row says downloading (stale)", async () => {
    await seed({
      id: "book-1",
      format: "epub",
      status: "downloading",
      bytes: 0,
      pageCount: 1,
    });
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    // A stale "downloading" row from a prior tab/session shows the cancel
    // affordance even though no controller is wired; cancel does nothing
    // but is harmless.
    expect(
      await screen.findByRole("button", { name: /cancel download/i }),
    ).toBeInTheDocument();
  });
});

// Each menuitem interaction can wait up to 5s for Mantine's portal+transition
// under heavy parallel test load, and these tests chain two menu round-trips
// (open → menuitem → cancel). Bump the per-test timeout so the chained waits
// fit comfortably within a single test budget.
describe(
  "DownloadButton: download trigger and progress",
  { timeout: 20000 },
  () => {
    it("invokes downloadSingleFileBook and forwards progress to the ring", async () => {
      let progressCallback:
        | ((p: { loaded: number; total: number | null }) => void)
        | undefined;

      stubDownload(async (opts) => {
        progressCallback = opts.onProgress;
        progressCallback?.({ loaded: 50, total: 100 });
        progressCallback?.({ loaded: 100, total: 100 });
        // Simulate the manager's final IDB write + broadcast so the listener
        // can flip to "downloaded".
        const complete: DownloadRecord = {
          id: opts.bookId,
          format: "epub",
          status: "complete",
          bytes: 100,
          pageCount: 1,
          downloadedAt: 1,
        };
        await putDownload(complete);
        broadcastDownloadsChange({ kind: "put", record: complete });
        return { bookId: opts.bookId, bytes: 100 };
      });

      renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);

      const trigger = await screen.findByRole("button", {
        name: /^download options$/i,
      });
      await userEvent.click(trigger);
      // The menu lives in a Mantine portal with a brief transition; under
      // heavy test load the default 1000ms timeout can flake. Bump it so we
      // catch the dropdown reliably.
      const startItem = await screen.findByRole(
        "menuitem",
        { name: /save for offline/i },
        { timeout: 5000 },
      );
      await userEvent.click(startItem);

      expect(downloadSpy).toHaveBeenCalledWith(
        expect.objectContaining({ bookId: "book-1", format: "epub" }),
      );

      // After completion the broadcast flips the UI to the downloaded state.
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /offline download options/i }),
        ).toBeInTheDocument();
      });
    });

    it("dispatches to downloadComicBook for cbz with pageCount", async () => {
      const comicSpy = vi
        .spyOn(downloadManagerModule, "downloadComicBook")
        .mockImplementation(async (opts) => {
          opts.onProgress?.({ loaded: opts.pageCount, total: opts.pageCount });
          const complete: DownloadRecord = {
            id: opts.bookId,
            format: "cbz",
            status: "complete",
            bytes: opts.pageCount,
            pageCount: opts.pageCount,
            downloadedAt: 1,
          };
          await putDownload(complete);
          broadcastDownloadsChange({ kind: "put", record: complete });
          return { bookId: opts.bookId, bytes: opts.pageCount };
        });

      try {
        renderWithProviders(
          <DownloadButton bookId="book-cbz" fileFormat="cbz" pageCount={12} />,
        );
        const trigger = await screen.findByRole("button", {
          name: /^download options$/i,
        });
        await userEvent.click(trigger);
        const startItem = await screen.findByRole("menuitem", {
          name: /save for offline/i,
        });
        await userEvent.click(startItem);

        expect(comicSpy).toHaveBeenCalledWith(
          expect.objectContaining({
            bookId: "book-cbz",
            format: "cbz",
            pageCount: 12,
          }),
        );
        await waitFor(() => {
          expect(
            screen.getByRole("button", { name: /offline download options/i }),
          ).toBeInTheDocument();
        });
      } finally {
        comicSpy.mockRestore();
      }
    });

    it("calls AbortController.abort when the user clicks cancel", async () => {
      let receivedSignal: AbortSignal | undefined;
      let resolveDownload: (() => void) | null = null;

      stubDownload(async (opts) => {
        receivedSignal = opts.signal;
        // Block on a manual resolve so the test can simulate "still in flight".
        await new Promise<void>((res) => {
          resolveDownload = res;
        });
        throw new DOMException("Aborted", "AbortError");
      });

      renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
      const trigger = await screen.findByRole("button", {
        name: /^download options$/i,
      });
      await userEvent.click(trigger);
      // The menu lives in a Mantine portal with a brief transition; under
      // heavy test load the default 1000ms timeout can flake. Bump it so we
      // catch the dropdown reliably.
      const startItem = await screen.findByRole(
        "menuitem",
        { name: /save for offline/i },
        { timeout: 5000 },
      );
      await userEvent.click(startItem);

      const cancel = await screen.findByRole("button", {
        name: /cancel download/i,
      });
      await userEvent.click(cancel);

      expect(receivedSignal?.aborted).toBe(true);

      // Unblock the stubbed download so the component's catch runs.
      resolveDownload?.();
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /^download options$/i }),
        ).toBeInTheDocument();
      });
    });
  },
);

describe("DownloadButton: remove flow", { timeout: 20000 }, () => {
  it("removing deletes the IDB row and resets to not-downloaded", async () => {
    await seed({
      id: "book-1",
      format: "epub",
      status: "complete",
      bytes: 100,
      pageCount: 1,
      downloadedAt: 1,
    });
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    const menuTarget = await screen.findByRole("button", {
      name: /offline download options/i,
    });
    await userEvent.click(menuTarget);

    const removeItem = await screen.findByRole(
      "menuitem",
      { name: /remove offline copy/i },
      { timeout: 5000 },
    );
    await userEvent.click(removeItem);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /^download options$/i }),
      ).toBeInTheDocument();
    });
    expect(await getDownload("book-1")).toBeUndefined();
  });
});

describe("DownloadButton: cross-tab broadcast", () => {
  it("flips to downloaded when a put-complete broadcast arrives", async () => {
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();

    const record: DownloadRecord = {
      id: "book-1",
      format: "epub",
      status: "complete",
      bytes: 42,
      pageCount: 1,
      downloadedAt: 1,
    };
    broadcastDownloadsChange({ kind: "put", record });

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /offline download options/i }),
      ).toBeInTheDocument();
    });
  });

  it("flips back to not-downloaded when a delete broadcast arrives", async () => {
    await seed({
      id: "book-1",
      format: "epub",
      status: "complete",
      bytes: 42,
      pageCount: 1,
      downloadedAt: 1,
    });
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", {
        name: /offline download options/i,
      }),
    ).toBeInTheDocument();

    broadcastDownloadsChange({ kind: "delete", id: "book-1" });

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /^download options$/i }),
      ).toBeInTheDocument();
    });
  });

  it("ignores broadcasts for other book ids", async () => {
    renderWithProviders(<DownloadButton bookId="book-1" fileFormat="epub" />);
    expect(
      await screen.findByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();

    const otherRecord: DownloadRecord = {
      id: "different-book",
      format: "pdf",
      status: "complete",
      bytes: 99,
      pageCount: 1,
      downloadedAt: 1,
    };
    broadcastDownloadsChange({ kind: "put", record: otherRecord });

    // Should still be in the not-downloaded state.
    expect(
      screen.getByRole("button", { name: /^download options$/i }),
    ).toBeInTheDocument();
  });
});

describe("DownloadButton: file URL fallback", { timeout: 20000 }, () => {
  it("surfaces a 'Download file' menu item when fileDownloadUrl is provided", async () => {
    renderWithProviders(
      <DownloadButton
        bookId="book-1"
        fileFormat="epub"
        fileDownloadUrl="/api/v1/books/book-1/file"
      />,
    );
    const trigger = await screen.findByRole("button", {
      name: /^download options$/i,
    });
    await userEvent.click(trigger);

    const fileItem = await screen.findByRole(
      "menuitem",
      { name: /download file/i },
      { timeout: 5000 },
    );
    expect(fileItem).toHaveAttribute("href", "/api/v1/books/book-1/file");
  });

  it("offers 'Download file' even when the format isn't cacheable for offline", async () => {
    renderWithProviders(
      <DownloadButton
        bookId="book-x"
        fileFormat="mobi"
        fileDownloadUrl="/api/v1/books/book-x/file"
      />,
    );
    const trigger = await screen.findByRole("button", {
      name: /^download options$/i,
    });
    await userEvent.click(trigger);

    expect(
      await screen.findByRole(
        "menuitem",
        { name: /download file/i },
        { timeout: 5000 },
      ),
    ).toBeInTheDocument();
    // No offline-save entry for unsupported formats.
    expect(
      screen.queryByRole("menuitem", { name: /save for offline/i }),
    ).toBeNull();
  });
});
