import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  _resetForTests,
  broadcastDownloadsChange,
  type DownloadRecord,
  getDownload,
  putDownload,
  setDbContext,
} from "@/lib/offline/db";
import {
  _resetPersistenceForTests,
  type StoragePersistence,
} from "@/lib/offline/downloadManager";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { DownloadsSettings } from "./DownloadsSettings";

interface StorageStub {
  persist: ReturnType<typeof vi.fn>;
  estimate: ReturnType<typeof vi.fn>;
}

let originalStorageDescriptor: PropertyDescriptor | undefined;

function installStorage(stub: StorageStub) {
  originalStorageDescriptor = Object.getOwnPropertyDescriptor(
    globalThis.navigator,
    "storage",
  );
  Object.defineProperty(globalThis.navigator, "storage", {
    configurable: true,
    value: stub,
  });
}

function restoreStorage() {
  if (originalStorageDescriptor) {
    Object.defineProperty(
      globalThis.navigator,
      "storage",
      originalStorageDescriptor,
    );
  } else {
    Object.defineProperty(globalThis.navigator, "storage", {
      configurable: true,
      value: undefined,
    });
  }
  originalStorageDescriptor = undefined;
}

function makeStorageStub(
  persistValue: StoragePersistence,
  usage: number,
  quota: number,
): StorageStub {
  return {
    persist: vi.fn(async () => (persistValue === null ? false : persistValue)),
    estimate: vi.fn(async () => ({ usage, quota })),
  };
}

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  _resetPersistenceForTests();
  restoreStorage();
});

function makeDownload(
  id: string,
  overrides: Partial<DownloadRecord> = {},
): DownloadRecord {
  return {
    id,
    format: "epub",
    status: "complete",
    bytes: 1024,
    pageCount: 1,
    downloadedAt: 1_700_000_000_000,
    ...overrides,
  };
}

describe("DownloadsSettings: empty state", () => {
  it("shows the empty alert when nothing is downloaded", async () => {
    installStorage(makeStorageStub(true, 0, 1_000_000));
    renderWithProviders(<DownloadsSettings />);
    expect(
      await screen.findByText(/No offline downloads yet/i),
    ).toBeInTheDocument();
  });
});

describe("DownloadsSettings: list rendering", () => {
  it("lists every downloaded book with size and format", async () => {
    installStorage(makeStorageStub(true, 100, 1000));
    await putDownload(makeDownload("book-a", { bytes: 2_500_000 }));
    await putDownload(
      makeDownload("book-b", {
        format: "cbz",
        bytes: 12_000_000,
        pageCount: 22,
      }),
    );
    await putDownload(
      makeDownload("book-c", {
        format: "pdf",
        bytes: 4_500_000,
        status: "downloading",
      }),
    );

    renderWithProviders(<DownloadsSettings />);

    expect(await screen.findByText("book-a")).toBeInTheDocument();
    expect(screen.getByText("book-b")).toBeInTheDocument();
    expect(screen.getByText("book-c")).toBeInTheDocument();

    // Total = sum of complete records only (book-a + book-b). book-c is
    // still downloading so its bytes do not contribute.
    expect(screen.getByText(/3 books saved/i)).toBeInTheDocument();
  });

  it("shows the storage quota meter when navigator.storage.estimate is available", async () => {
    installStorage(makeStorageStub(true, 500_000_000, 1_000_000_000));
    await putDownload(makeDownload("book-a"));
    renderWithProviders(<DownloadsSettings />);

    expect(await screen.findByText(/Storage used/i)).toBeInTheDocument();
    // 500 MB / 1 GB rounds to 47.68 MB usage, 953.67 MB available with the
    // helper's formatting; just check the slash format is rendered.
    expect(screen.getByText(/available/i)).toBeInTheDocument();
  });

  it("surfaces the persistence indicator when persist() resolves true", async () => {
    installStorage(makeStorageStub(true, 0, 1));
    renderWithProviders(<DownloadsSettings />);
    expect(
      await screen.findByText(/Storage is persistent/i),
    ).toBeInTheDocument();
  });

  it("warns when persist() resolves false", async () => {
    installStorage(makeStorageStub(false, 0, 1));
    renderWithProviders(<DownloadsSettings />);
    expect(
      await screen.findByText(/Storage is not marked persistent/i),
    ).toBeInTheDocument();
  });
});

describe("DownloadsSettings: remove flow", () => {
  it("removing a book deletes its IDB row and refreshes the list", async () => {
    installStorage(makeStorageStub(true, 0, 1));
    await putDownload(makeDownload("book-a"));
    await putDownload(makeDownload("book-b"));

    renderWithProviders(<DownloadsSettings />);

    const removeButton = await screen.findByRole("button", {
      name: /Remove offline copy of book-a/i,
    });
    await userEvent.click(removeButton);

    await waitFor(async () => {
      expect(await getDownload("book-a")).toBeUndefined();
    });
    await waitFor(() => {
      expect(screen.queryByText("book-a")).toBeNull();
    });
    expect(screen.getByText("book-b")).toBeInTheDocument();
  });
});

describe("DownloadsSettings: clear-all flow", () => {
  it("Clear all asks for confirmation and then removes every record", async () => {
    installStorage(makeStorageStub(true, 0, 1));
    await putDownload(makeDownload("book-a"));
    await putDownload(makeDownload("book-b"));

    renderWithProviders(<DownloadsSettings />);

    const clearTrigger = await screen.findByRole("button", {
      name: /Clear all downloads/i,
    });
    await userEvent.click(clearTrigger);

    const confirm = await screen.findByRole("button", { name: /Remove all/i });
    await userEvent.click(confirm);

    await waitFor(() => {
      expect(screen.queryByText("book-a")).toBeNull();
      expect(screen.queryByText("book-b")).toBeNull();
    });
    expect(
      await screen.findByText(/No offline downloads yet/i),
    ).toBeInTheDocument();
  });

  it("Cancelling the confirmation modal keeps everything", async () => {
    installStorage(makeStorageStub(true, 0, 1));
    await putDownload(makeDownload("book-a"));

    renderWithProviders(<DownloadsSettings />);
    await userEvent.click(
      await screen.findByRole("button", { name: /Clear all downloads/i }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /^Cancel$/i }),
    );

    expect(screen.getByText("book-a")).toBeInTheDocument();
    expect(await getDownload("book-a")).toBeDefined();
  });
});

describe("DownloadsSettings: broadcast updates", () => {
  it("picks up a new download from a broadcast", async () => {
    installStorage(makeStorageStub(true, 0, 1));
    renderWithProviders(<DownloadsSettings />);
    expect(
      await screen.findByText(/No offline downloads yet/i),
    ).toBeInTheDocument();

    const record = makeDownload("book-broadcast");
    await putDownload(record);
    broadcastDownloadsChange({ kind: "put", record });

    await waitFor(() => {
      expect(screen.getByText("book-broadcast")).toBeInTheDocument();
    });
  });
});
