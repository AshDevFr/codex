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
import { _resetForTests, setDbContext } from "@/lib/offline/db";
import { _resetPersistenceForTests } from "@/lib/offline/downloadManager";
import * as seriesQueueModule from "@/lib/offline/seriesDownloadQueue";
import {
  type BookQueueState,
  QuotaExceededError,
  type SeriesDownloadController,
  type SeriesQueueState,
} from "@/lib/offline/seriesDownloadQueue";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { SeriesDownloadButton } from "./SeriesDownloadButton";

type BatchFn = typeof seriesQueueModule.downloadSeriesBatch;
let batchSpy: MockInstance<BatchFn> | null = null;

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  _resetPersistenceForTests();
  batchSpy?.mockRestore();
  batchSpy = null;
});

function stubBatch(
  impl: (opts: Parameters<BatchFn>[0]) => Promise<SeriesDownloadController>,
) {
  batchSpy = vi
    .spyOn(seriesQueueModule, "downloadSeriesBatch")
    .mockImplementation(impl);
}

/**
 * Build a synthetic controller whose subscribe/done lifecycle is driven
 * by the test. The factory returns the controller and a `push` helper
 * the test calls to deliver a new snapshot to subscribers.
 */
function makeController(
  seriesId: string,
  bookIds: string[],
): {
  controller: SeriesDownloadController;
  push: (state: SeriesQueueState) => void;
  resolve: (result?: SeriesQueueState) => void;
} {
  const listeners = new Set<(s: SeriesQueueState) => void>();
  const initial: SeriesQueueState = {
    seriesId,
    total: bookIds.length,
    completed: 0,
    failed: 0,
    cancelled: 0,
    perBook: new Map(
      bookIds.map((id) => [
        id,
        {
          bookId: id,
          status: "queued",
          loaded: 0,
          total: null,
        } satisfies BookQueueState,
      ]),
    ),
  };
  let current = initial;
  let resolveDone!: (
    result: ReturnType<SeriesDownloadController["getState"]>,
  ) => void;
  const donePromise = new Promise<SeriesQueueState>((res) => {
    resolveDone = res;
  });

  const controller: SeriesDownloadController = {
    cancelBook: vi.fn(),
    cancelAll: vi.fn(),
    subscribe(listener) {
      listeners.add(listener);
      listener(current);
      return () => listeners.delete(listener);
    },
    getState: () => current,
    // The component awaits this and uses the snapshot to render the
    // "done" panel; the test resolves it directly when ready.
    done: donePromise.then((finalState) => ({
      completed: Array.from(finalState.perBook.values())
        .filter((b) => b.status === "complete")
        .map((b) => b.bookId),
      failed: Array.from(finalState.perBook.values())
        .filter((b) => b.status === "error")
        .map((b) => ({ bookId: b.bookId, error: b.error ?? "err" })),
      cancelled: Array.from(finalState.perBook.values())
        .filter((b) => b.status === "cancelled")
        .map((b) => b.bookId),
    })),
  };

  return {
    controller,
    push: (state) => {
      current = state;
      for (const l of Array.from(listeners)) l(state);
    },
    resolve: (result) => {
      resolveDone(result ?? current);
    },
  };
}

const epubBooks = [
  { id: "a", fileFormat: "epub", pageCount: 1, fileSize: 4 },
  { id: "b", fileFormat: "epub", pageCount: 1, fileSize: 4 },
];

describe("SeriesDownloadButton: idle state", () => {
  it("renders a Download series button and opens the modal on click", async () => {
    renderWithProviders(
      <SeriesDownloadButton seriesId="s-1" books={epubBooks} />,
    );
    const trigger = screen.getByRole("button", { name: /download series/i });
    await userEvent.click(trigger);
    expect(
      await screen.findByRole("button", { name: /start downloading/i }),
    ).toBeInTheDocument();
  });

  it("shows an Unsupported badge for books the queue cannot handle", async () => {
    renderWithProviders(
      <SeriesDownloadButton
        seriesId="s-mix"
        books={[
          { id: "a", fileFormat: "epub", pageCount: 1, fileSize: 4 },
          { id: "b", fileFormat: "mobi", pageCount: 1, fileSize: 4 },
        ]}
      />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /download series/i }),
    );
    expect(await screen.findByText(/unsupported/i)).toBeInTheDocument();
  });
});

describe("SeriesDownloadButton: pre-flight refusal", () => {
  it("displays the quota refusal message and never enters the running state", async () => {
    stubBatch(async () => {
      throw new QuotaExceededError({
        estimatedBytes: 1_000_000,
        usage: 900_000,
        quota: 1_000_000,
        threshold: 0.9,
      });
    });
    renderWithProviders(
      <SeriesDownloadButton seriesId="s-quota" books={epubBooks} />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /download series/i }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /start downloading/i }),
    );
    expect(
      await screen.findByText(/would exceed storage quota/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /cancel all/i }),
    ).not.toBeInTheDocument();
  });
});

describe("SeriesDownloadButton: running state", () => {
  it("renders per-book progress as the controller emits updates", async () => {
    const ctx = makeController("s-run", ["a", "b"]);
    stubBatch(async () => ctx.controller);
    renderWithProviders(
      <SeriesDownloadButton seriesId="s-run" books={epubBooks} />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /download series/i }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /start downloading/i }),
    );
    // Wait for the running phase to render the Cancel-all button.
    await screen.findByRole("button", { name: /cancel all/i });

    // Push a progress update — book a downloading at 50%.
    const next: SeriesQueueState = {
      seriesId: "s-run",
      total: 2,
      completed: 0,
      failed: 0,
      cancelled: 0,
      perBook: new Map([
        [
          "a",
          {
            bookId: "a",
            status: "downloading",
            loaded: 50,
            total: 100,
          },
        ],
        ["b", { bookId: "b", status: "queued", loaded: 0, total: null }],
      ]),
    };
    ctx.push(next);
    await screen.findByText(/downloading/i);
  });

  it("Cancel all invokes controller.cancelAll", async () => {
    const ctx = makeController("s-cancel", ["a", "b"]);
    stubBatch(async () => ctx.controller);
    renderWithProviders(
      <SeriesDownloadButton seriesId="s-cancel" books={epubBooks} />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /download series/i }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /start downloading/i }),
    );
    const cancelAll = await screen.findByRole("button", {
      name: /cancel all/i,
    });
    await userEvent.click(cancelAll);
    expect(ctx.controller.cancelAll).toHaveBeenCalled();
  });
});

describe("SeriesDownloadButton: done state", () => {
  it("flips to the done panel when the controller resolves", async () => {
    const ctx = makeController("s-done", ["a", "b"]);
    stubBatch(async () => ctx.controller);
    renderWithProviders(
      <SeriesDownloadButton seriesId="s-done" books={epubBooks} />,
    );
    await userEvent.click(
      screen.getByRole("button", { name: /download series/i }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /start downloading/i }),
    );
    await screen.findByRole("button", { name: /cancel all/i });

    const final: SeriesQueueState = {
      seriesId: "s-done",
      total: 2,
      completed: 2,
      failed: 0,
      cancelled: 0,
      perBook: new Map([
        ["a", { bookId: "a", status: "complete", loaded: 1, total: 1 }],
        ["b", { bookId: "b", status: "complete", loaded: 1, total: 1 }],
      ]),
    };
    ctx.push(final);
    ctx.resolve(final);
    await waitFor(() => {
      expect(screen.getByText(/2 downloaded/i)).toBeInTheDocument();
    });
  });
});
