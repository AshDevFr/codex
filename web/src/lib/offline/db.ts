/**
 * IndexedDB access layer for the offline-reading feature (Phase 12).
 *
 * Two stores:
 * - `downloads`: per-book metadata, source of truth for whether a book is
 *   available offline. The service worker reads this at boot and intercepts
 *   page/file requests for matching ids; the page side writes when a download
 *   completes or is removed.
 * - `outbox`: queued reading-progress mutations that failed offline. Drained
 *   on `online` and `visibilitychange` events.
 *
 * Works in both Window and ServiceWorkerGlobalScope contexts. No external
 * deps; hand-rolled to keep the SW bundle small.
 */

export const DB_NAME = "codex-offline";
export const DB_VERSION = 1;

export const DOWNLOADS_STORE = "downloads";
export const OUTBOX_STORE = "outbox";

export const DOWNLOADS_BROADCAST_CHANNEL = "codex:downloads";

export type DownloadFormat = "comic" | "epub" | "pdf";
export type DownloadStatus = "queued" | "downloading" | "complete" | "error";

export interface DownloadRecord {
  /** Book id; primary key. */
  id: string;
  format: DownloadFormat;
  status: DownloadStatus;
  /** Bytes already cached. */
  bytes: number;
  /** Total page count for comics; 1 for single-file formats. */
  pageCount: number;
  /** ms epoch when the download completed; undefined while queued/downloading. */
  downloadedAt?: number;
  /** ms epoch of the most recent reader session. */
  lastReadAt?: number;
  /** Error message if status === "error". */
  error?: string;
}

export interface OutboxRecord {
  /** Auto-incremented key. */
  id?: number;
  /** Serialised fetch input. Body is stored as string to keep cloning cheap. */
  request: {
    url: string;
    method: string;
    headers: Record<string, string>;
    body?: string;
  };
  createdAt: number;
  retryCount: number;
}

/**
 * Broadcast payload published on `codex:downloads` when a record changes.
 * Subscribers (SW route handler, Downloads page, DownloadButton) refresh
 * their in-memory view from this.
 */
export type DownloadsBroadcast =
  | { kind: "put"; record: DownloadRecord }
  | { kind: "delete"; id: string }
  | { kind: "clear" };

type IDBContext = {
  indexedDB: IDBFactory;
};

/**
 * Resolve the IndexedDB factory in the current scope. Window has
 * `self.indexedDB`; ServiceWorkerGlobalScope has the same. Tests can pass an
 * override via `setDbContext` (used by fake-indexeddb).
 */
let dbContext: IDBContext | null = null;

export function setDbContext(ctx: IDBContext | null): void {
  dbContext = ctx;
  cachedDb = null;
}

function getIndexedDB(): IDBFactory {
  if (dbContext) return dbContext.indexedDB;
  const scopeIDB =
    typeof self !== "undefined"
      ? (self as unknown as { indexedDB?: IDBFactory }).indexedDB
      : undefined;
  if (!scopeIDB) {
    throw new Error("IndexedDB is not available in this environment");
  }
  return scopeIDB;
}

let cachedDb: IDBDatabase | null = null;
let openPromise: Promise<IDBDatabase> | null = null;

export async function openDatabase(): Promise<IDBDatabase> {
  if (cachedDb) return cachedDb;
  if (openPromise) return openPromise;

  openPromise = new Promise<IDBDatabase>((resolve, reject) => {
    const request = getIndexedDB().open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(DOWNLOADS_STORE)) {
        db.createObjectStore(DOWNLOADS_STORE, { keyPath: "id" });
      }
      if (!db.objectStoreNames.contains(OUTBOX_STORE)) {
        db.createObjectStore(OUTBOX_STORE, {
          keyPath: "id",
          autoIncrement: true,
        });
      }
    };

    request.onsuccess = () => {
      cachedDb = request.result;
      cachedDb.onclose = () => {
        cachedDb = null;
      };
      resolve(cachedDb);
    };

    request.onerror = () => reject(request.error);
    request.onblocked = () =>
      reject(new Error("IndexedDB open blocked by another connection"));
  }).finally(() => {
    openPromise = null;
  });

  return openPromise;
}

/**
 * Reset cached state. Used by tests; not part of the runtime API.
 */
export function _resetForTests(): void {
  cachedDb = null;
  openPromise = null;
}

async function runTransaction<T>(
  storeName: string,
  mode: IDBTransactionMode,
  fn: (store: IDBObjectStore) => IDBRequest<T> | T,
): Promise<T> {
  const db = await openDatabase();
  return new Promise<T>((resolve, reject) => {
    const tx = db.transaction(storeName, mode);
    const store = tx.objectStore(storeName);
    let value: T | undefined;
    let didResolve = false;

    try {
      const result = fn(store);
      if (
        result &&
        typeof (result as IDBRequest<T>).onsuccess !== "undefined"
      ) {
        (result as IDBRequest<T>).onsuccess = () => {
          value = (result as IDBRequest<T>).result;
          didResolve = true;
        };
        (result as IDBRequest<T>).onerror = () =>
          reject((result as IDBRequest<T>).error);
      } else {
        value = result as T;
        didResolve = true;
      }
    } catch (err) {
      reject(err);
      return;
    }

    tx.oncomplete = () => resolve(didResolve ? (value as T) : (undefined as T));
    tx.onerror = () => reject(tx.error);
    tx.onabort = () =>
      reject(tx.error ?? new Error("IndexedDB transaction aborted"));
  });
}

// -- downloads store -----------------------------------------------------

export async function getAllDownloads(): Promise<DownloadRecord[]> {
  return runTransaction<DownloadRecord[]>(
    DOWNLOADS_STORE,
    "readonly",
    (store) => store.getAll() as IDBRequest<DownloadRecord[]>,
  );
}

export async function getDownload(
  id: string,
): Promise<DownloadRecord | undefined> {
  return runTransaction<DownloadRecord | undefined>(
    DOWNLOADS_STORE,
    "readonly",
    (store) => store.get(id) as IDBRequest<DownloadRecord | undefined>,
  );
}

export async function putDownload(record: DownloadRecord): Promise<void> {
  await runTransaction<IDBValidKey>(DOWNLOADS_STORE, "readwrite", (store) =>
    store.put(record),
  );
}

export async function deleteDownload(id: string): Promise<void> {
  await runTransaction<undefined>(DOWNLOADS_STORE, "readwrite", (store) => {
    store.delete(id);
    return undefined;
  });
}

export async function clearDownloads(): Promise<void> {
  await runTransaction<undefined>(DOWNLOADS_STORE, "readwrite", (store) => {
    store.clear();
    return undefined;
  });
}

// -- outbox store --------------------------------------------------------

export async function enqueueOutbox(
  request: OutboxRecord["request"],
): Promise<number> {
  const record: OutboxRecord = {
    request,
    createdAt: Date.now(),
    retryCount: 0,
  };
  const key = await runTransaction<IDBValidKey>(
    OUTBOX_STORE,
    "readwrite",
    (store) => store.add(record),
  );
  return Number(key);
}

export async function getOutbox(): Promise<OutboxRecord[]> {
  return runTransaction<OutboxRecord[]>(
    OUTBOX_STORE,
    "readonly",
    (store) => store.getAll() as IDBRequest<OutboxRecord[]>,
  );
}

export async function deleteOutboxEntry(id: number): Promise<void> {
  await runTransaction<undefined>(OUTBOX_STORE, "readwrite", (store) => {
    store.delete(id);
    return undefined;
  });
}

export async function clearOutbox(): Promise<void> {
  await runTransaction<undefined>(OUTBOX_STORE, "readwrite", (store) => {
    store.clear();
    return undefined;
  });
}

/**
 * Drain the outbox in insertion order. `send` is invoked sequentially for
 * each record; on success the record is removed. On failure the drain stops
 * (preserves order; the failing record stays at the head for the next
 * attempt) and the failed record's `retryCount` is bumped. Returns the
 * number of records successfully sent.
 *
 * Sequential rather than parallel because reading-progress updates for the
 * same book must apply in order; the server's last-write-wins resolution
 * would otherwise reorder them.
 */
export async function drainOutbox(
  send: (record: OutboxRecord) => Promise<void>,
): Promise<number> {
  const all = await getOutbox();
  all.sort((a, b) => (a.id ?? 0) - (b.id ?? 0));
  let sent = 0;
  for (const record of all) {
    try {
      await send(record);
      if (record.id !== undefined) {
        await deleteOutboxEntry(record.id);
      }
      sent += 1;
    } catch {
      if (record.id !== undefined) {
        await runTransaction<undefined>(OUTBOX_STORE, "readwrite", (store) => {
          store.put({ ...record, retryCount: record.retryCount + 1 });
          return undefined;
        });
      }
      break;
    }
  }
  return sent;
}

// -- broadcast helpers ---------------------------------------------------

/**
 * Publish a downloads-store change. Returns silently in environments without
 * BroadcastChannel (older Safari, test JSDOM without the polyfill).
 */
export function broadcastDownloadsChange(payload: DownloadsBroadcast): void {
  if (typeof BroadcastChannel === "undefined") return;
  const channel = new BroadcastChannel(DOWNLOADS_BROADCAST_CHANNEL);
  try {
    channel.postMessage(payload);
  } finally {
    channel.close();
  }
}
