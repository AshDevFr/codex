#!/usr/bin/env npx tsx
/**
 * Komga to Codex Reading Progress Sync
 *
 * Syncs reading progress from Komga to Codex by matching books via file path.
 * Dry run by default - shows what would be synced without making changes.
 *
 * Usage:
 *   npx tsx scripts/komga-sync.ts --komga-url http://localhost:25600 --komga-user admin --komga-password xxx --codex-url http://localhost:3000 --codex-token xxx
 *   npx tsx scripts/komga-sync.ts --apply  # Actually apply changes
 */

import { parseArgs } from "node:util";
import { writeFile } from "node:fs/promises";

// ============================================================================
// Types
// ============================================================================

interface KomgaBook {
  id: string;
  name: string; // filename
  url: string; // file path
  seriesId: string;
  seriesTitle: string;
  readProgress?: {
    page: number;
    completed: boolean;
  } | null;
  media: {
    pagesCount: number;
  };
}

interface KomgaPageResponse<T> {
  content: T[];
  pageable: { pageNumber: number; pageSize: number };
  totalElements: number;
  totalPages: number;
  last: boolean;
  first: boolean;
}

interface CodexBook {
  id: string;
  filePath: string;
  seriesId: string;
  seriesName: string;
  pageCount: number;
  readProgress?: {
    currentPage: number;
    completed: boolean;
  } | null;
}

interface CodexBooksResponse {
  data: CodexBook[];
  page: number;
  pageSize: number;
  totalPages: number;
  totalItems: number;
}

interface SyncResult {
  komgaBook: KomgaBook;
  codexBook: CodexBook | null;
  status: "match" | "not-found";
  progressDiff: {
    komga: { page: number; completed: boolean } | null;
    codex: { page: number; completed: boolean } | null;
    needsSync: boolean;
  };
}

interface SeriesGroup {
  seriesTitle: string;
  books: SyncResult[];
}

// ============================================================================
// Utilities
// ============================================================================

async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchWithRetry(
  url: string,
  options: RequestInit,
  maxRetries = 3,
  baseDelayMs = 1000
): Promise<Response> {
  let lastError: Error | null = null;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    const resp = await fetch(url, options);

    // Success or client error (4xx) - don't retry
    if (resp.ok || (resp.status >= 400 && resp.status < 500 && resp.status !== 429)) {
      return resp;
    }

    // Rate limited (429) or server error (5xx) - retry with backoff
    if (resp.status === 429 || resp.status >= 500) {
      if (attempt < maxRetries) {
        // Check for Retry-After header
        const retryAfter = resp.headers.get("Retry-After");
        let delayMs: number;

        if (retryAfter) {
          // Retry-After can be seconds or a date
          const seconds = parseInt(retryAfter, 10);
          delayMs = isNaN(seconds) ? baseDelayMs : seconds * 1000;
        } else {
          // Exponential backoff: 1s, 2s, 4s, ...
          delayMs = baseDelayMs * Math.pow(2, attempt);
        }

        console.log(`  Rate limited (${resp.status}), retrying in ${delayMs}ms...`);
        await sleep(delayMs);
        continue;
      }
    }

    // Max retries exceeded or non-retryable error
    return resp;
  }

  // Should not reach here, but just in case
  throw lastError ?? new Error("Max retries exceeded");
}

// ============================================================================
// API Clients
// ============================================================================

async function createKomgaClient(url: string, apiKey: string) {
  const headers = {
    "X-API-Key": apiKey,
    Accept: "application/json",
  };

  // Test connection
  const testUrl = `${url}/api/v2/users/me`;
  console.log(`Connecting to Komga: ${testUrl}`);
  const resp = await fetch(testUrl, { headers });
  if (!resp.ok) {
    const body = await resp.text().catch(() => "");
    throw new Error(`Failed to connect to Komga: ${resp.status} ${resp.statusText}\n  URL: ${testUrl}\n  Response: ${body.slice(0, 200)}`);
  }
  const me = await resp.json();
  console.log(`Connected to Komga as: ${me.email}`);

  return {
    async getBooks(readStatus?: "READ" | "IN_PROGRESS" | "UNREAD"): Promise<KomgaBook[]> {
      const books: KomgaBook[] = [];
      let page = 0;

      while (true) {
        const params = new URLSearchParams({
          page: page.toString(),
          size: "500",
        });
        if (readStatus) {
          params.set("read_status", readStatus);
        }

        const resp = await fetch(`${url}/api/v1/books?${params}`, { headers });
        if (!resp.ok) {
          throw new Error(`Failed to fetch books: ${resp.status}`);
        }

        const data: KomgaPageResponse<KomgaBook> = await resp.json();
        books.push(...data.content);

        if (data.last) break;
        page++;
      }

      return books;
    },
  };
}

async function createCodexClient(url: string, apiKey: string) {
  const headers = {
    "X-API-Key": apiKey,
    Accept: "application/json",
    "Content-Type": "application/json",
  };

  console.log(`Connecting to Codex: ${url}`);

  return {
    async getAllBooks(): Promise<CodexBook[]> {
      const books: CodexBook[] = [];
      let page = 1;

      while (true) {
        const params = new URLSearchParams({
          page: page.toString(),
          pageSize: "100", // max allowed by API
        });

        const resp = await fetch(`${url}/api/v1/books?${params}`, { headers });
        if (!resp.ok) {
          throw new Error(`Failed to fetch books: ${resp.status}`);
        }

        const data: CodexBooksResponse = await resp.json();
        books.push(...data.data);

        if (page >= data.totalPages) break;
        page++;
      }

      return books;
    },

    async markAsRead(bookId: string): Promise<{ ok: boolean; status?: number; error?: string }> {
      const resp = await fetchWithRetry(`${url}/api/v1/books/${bookId}/read`, {
        method: "POST",
        headers,
      });
      if (resp.ok) return { ok: true };
      const body = await resp.text().catch(() => "");
      return { ok: false, status: resp.status, error: `${resp.status} ${resp.statusText}: ${body.slice(0, 200)}` };
    },

    async updateProgress(bookId: string, currentPage: number, completed: boolean): Promise<{ ok: boolean; status?: number; error?: string }> {
      const resp = await fetchWithRetry(`${url}/api/v1/books/${bookId}/progress`, {
        method: "PUT",
        headers,
        body: JSON.stringify({ currentPage, completed }),
      });
      if (resp.ok) return { ok: true };
      const body = await resp.text().catch(() => "");
      return { ok: false, status: resp.status, error: `${resp.status} ${resp.statusText}: ${body.slice(0, 200)}` };
    },
  };
}

// ============================================================================
// Sync Logic
// ============================================================================

function normalizePath(path: string): string {
  // Handle file:// URLs that Komga might return
  if (path.startsWith("file://")) {
    path = decodeURIComponent(new URL(path).pathname);
  }
  // Normalize slashes and remove trailing slash
  return path.replace(/\\/g, "/").replace(/\/$/, "");
}

function getMatchKey(path: string): string {
  // Extract last directory + filename for matching
  // e.g., "/data/comics/Batman/issue1.cbz" -> "Batman/issue1.cbz"
  const normalized = normalizePath(path);
  const parts = normalized.split("/");
  if (parts.length >= 2) {
    return parts.slice(-2).join("/").toLowerCase();
  }
  return parts[parts.length - 1].toLowerCase();
}

function buildPathIndex(books: CodexBook[]): Map<string, CodexBook> {
  const index = new Map<string, CodexBook>();
  for (const book of books) {
    const key = getMatchKey(book.filePath);
    index.set(key, book);
  }

  return index;
}

function analyzeSync(komgaBooks: KomgaBook[], codexIndex: Map<string, CodexBook>, debug: boolean): SyncResult[] {
  const results: SyncResult[] = [];

  for (const komgaBook of komgaBooks) {
    const key = getMatchKey(komgaBook.url);
    const codexBook = codexIndex.get(key);

    if (debug) {
      const match = codexBook ? "MATCH" : "NO MATCH";
      console.log(`  [${match}] ${komgaBook.seriesTitle} / ${komgaBook.name}`);
      console.log(`    Komga url: ${komgaBook.url}`);
      console.log(`    Match key: ${key}`);
      if (codexBook) {
        console.log(`    Codex path: ${codexBook.filePath}`);
      }
    }

    const komgaProgress = komgaBook.readProgress
      ? { page: komgaBook.readProgress.page, completed: komgaBook.readProgress.completed }
      : null;

    const codexProgress = codexBook?.readProgress
      ? { page: codexBook.readProgress.currentPage, completed: codexBook.readProgress.completed }
      : null;

    // Determine if sync is needed
    let needsSync = false;
    if (komgaProgress && codexBook) {
      if (!codexProgress) {
        // Codex has no progress, Komga does
        needsSync = true;
      } else if (komgaProgress.completed && !codexProgress.completed) {
        // Komga says completed, Codex doesn't
        needsSync = true;
      } else if (komgaProgress.page > codexProgress.page && !codexProgress.completed) {
        // Komga is further ahead (but skip if Codex already completed — page
        // counts differ between systems, especially for EPUBs)
        needsSync = true;
      }
    }

    results.push({
      komgaBook,
      codexBook: codexBook ?? null,
      status: codexBook ? "match" : "not-found",
      progressDiff: {
        komga: komgaProgress,
        codex: codexProgress,
        needsSync,
      },
    });
  }

  return results;
}

function groupBySeries(results: SyncResult[]): SeriesGroup[] {
  const groups = new Map<string, SeriesGroup>();

  for (const result of results) {
    const seriesTitle = result.komgaBook.seriesTitle;
    if (!groups.has(seriesTitle)) {
      groups.set(seriesTitle, { seriesTitle, books: [] });
    }
    groups.get(seriesTitle)!.books.push(result);
  }

  // Sort groups by series title, and books within each group by name
  const sortedGroups = Array.from(groups.values()).sort((a, b) =>
    a.seriesTitle.localeCompare(b.seriesTitle)
  );

  for (const group of sortedGroups) {
    group.books.sort((a, b) => a.komgaBook.name.localeCompare(b.komgaBook.name));
  }

  return sortedGroups;
}

// ============================================================================
// Display
// ============================================================================

const COLORS = {
  reset: "\x1b[0m",
  dim: "\x1b[2m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  cyan: "\x1b[36m",
  bold: "\x1b[1m",
};

function formatProgress(progress: { page: number; completed: boolean } | null, pageCount?: number): string {
  if (!progress) {
    return `${COLORS.dim}unread${COLORS.reset}`;
  }
  if (progress.completed) {
    return `${COLORS.green}completed${COLORS.reset}`;
  }
  const pages = pageCount ? `/${pageCount}` : "";
  return `${COLORS.yellow}p${progress.page}${pages}${COLORS.reset}`;
}

function printResults(groups: SeriesGroup[], showAll: boolean): void {
  let totalBooks = 0;
  let matchedBooks = 0;
  let notFoundBooks = 0;
  let needsSyncBooks = 0;
  let alreadySyncedBooks = 0;

  for (const group of groups) {
    const hasRelevantBooks = showAll || group.books.some((b) => b.progressDiff.needsSync || b.status === "not-found");

    if (!hasRelevantBooks) {
      // Still count stats
      for (const book of group.books) {
        totalBooks++;
        if (book.status === "match") matchedBooks++;
        else notFoundBooks++;
        if (!book.progressDiff.needsSync && book.status === "match") alreadySyncedBooks++;
      }
      continue;
    }

    console.log(`\n${COLORS.bold}${COLORS.cyan}${group.seriesTitle}${COLORS.reset}`);

    for (const book of group.books) {
      totalBooks++;

      const name = book.komgaBook.name;
      const pageCount = book.komgaBook.media?.pagesCount;

      if (book.status === "not-found") {
        notFoundBooks++;
        console.log(`  ${COLORS.red}[NOT FOUND]${COLORS.reset} ${name}`);
        continue;
      }

      matchedBooks++;

      const komgaStr = formatProgress(book.progressDiff.komga, pageCount);
      const codexStr = formatProgress(book.progressDiff.codex, book.codexBook?.pageCount);

      if (book.progressDiff.needsSync) {
        needsSyncBooks++;
        console.log(`  ${COLORS.yellow}[SYNC]${COLORS.reset} ${name}`);
        console.log(`         Komga: ${komgaStr}  ->  Codex: ${codexStr}`);
      } else if (showAll) {
        alreadySyncedBooks++;
        console.log(`  ${COLORS.green}[OK]${COLORS.reset} ${name} (${codexStr})`);
      } else {
        alreadySyncedBooks++;
      }
    }
  }

  // Summary
  console.log(`\n${"=".repeat(60)}`);
  console.log(`${COLORS.bold}Summary${COLORS.reset}`);
  console.log(`  Total books in Komga (with progress): ${totalBooks}`);
  console.log(`  ${COLORS.green}Matched in Codex:${COLORS.reset} ${matchedBooks}`);
  console.log(`  ${COLORS.red}Not found in Codex:${COLORS.reset} ${notFoundBooks}`);
  console.log(`  ${COLORS.yellow}Need sync:${COLORS.reset} ${needsSyncBooks}`);
  console.log(`  ${COLORS.green}Already synced:${COLORS.reset} ${alreadySyncedBooks}`);
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const { values } = parseArgs({
    options: {
      "komga-url": { type: "string", default: "http://localhost:25600" },
      "komga-key": { type: "string" },
      "codex-url": { type: "string", default: "http://localhost:3000" },
      "codex-key": { type: "string" },
      apply: { type: "boolean", default: false },
      "show-all": { type: "boolean", default: false },
      limit: { type: "string" },
      debug: { type: "boolean", default: false },
      export: { type: "string" },
      help: { type: "boolean", short: "h", default: false },
    },
  });

  if (values.help) {
    console.log(`
Komga to Codex Reading Progress Sync

Usage:
  npx tsx scripts/komga-sync.ts [options]

Options:
  --komga-url <url>       Komga server URL (default: http://localhost:25600)
  --komga-key <key>       Komga API key (required)
  --codex-url <url>       Codex server URL (default: http://localhost:3000)
  --codex-key <key>       Codex API key (required)
  --apply                 Actually apply changes (default: dry run)
  --show-all              Show all books, not just those needing sync
  --limit <n>             Only process first n books from Komga
  --debug                 Print detailed matching info for each book
  --export <file>         Export all books from both libraries to JSON file
  -h, --help              Show this help

Examples:
  # Dry run - see what would be synced
  npx tsx scripts/komga-sync.ts --komga-key xxx --codex-key xxx

  # Debug first 20 books
  npx tsx scripts/komga-sync.ts --komga-key xxx --codex-key xxx --limit 20 --debug

  # Export books to JSON for analysis
  npx tsx scripts/komga-sync.ts --komga-key xxx --codex-key xxx --export books.json

  # Apply changes
  npx tsx scripts/komga-sync.ts --komga-key xxx --codex-key xxx --apply
`);
    process.exit(0);
  }

  if (!values["komga-key"]) {
    console.error("Error: --komga-key is required");
    process.exit(1);
  }
  if (!values["codex-key"]) {
    console.error("Error: --codex-key is required");
    process.exit(1);
  }

  const dryRun = !values.apply;

  if (dryRun) {
    console.log(`${COLORS.bold}${COLORS.yellow}=== DRY RUN MODE ===${COLORS.reset}`);
    console.log("No changes will be made. Use --apply to sync.\n");
  } else {
    console.log(`${COLORS.bold}${COLORS.green}=== APPLYING CHANGES ===${COLORS.reset}\n`);
  }

  // Connect to both servers
  const komga = await createKomgaClient(values["komga-url"]!, values["komga-key"]!);
  const codex = await createCodexClient(values["codex-url"]!, values["codex-key"]!);

  console.log();

  const limit = values.limit ? parseInt(values.limit, 10) : undefined;
  const debug = values.debug ?? false;

  // Fetch books
  console.log("Fetching books from Komga (READ and IN_PROGRESS)...");
  const [readBooks, inProgressBooks] = await Promise.all([
    komga.getBooks("READ"),
    komga.getBooks("IN_PROGRESS"),
  ]);
  let komgaBooks = [...readBooks, ...inProgressBooks];
  console.log(`  Found ${readBooks.length} read + ${inProgressBooks.length} in-progress = ${komgaBooks.length} total`);

  if (limit) {
    komgaBooks = komgaBooks.slice(0, limit);
    console.log(`  Limited to first ${limit} books`);
  }

  console.log("Fetching books from Codex...");
  const codexBooks = await codex.getAllBooks();
  console.log(`  Found ${codexBooks.length} books`);

  // Build index and analyze
  const codexIndex = buildPathIndex(codexBooks);
  const results = analyzeSync(komgaBooks, codexIndex, debug);
  const groups = groupBySeries(results);

  // Build comprehensive export data
  const exportData = values.export
    ? {
        timestamp: new Date().toISOString(),
        options: {
          komgaUrl: values["komga-url"],
          codexUrl: values["codex-url"],
          dryRun,
          limit: limit ?? null,
          debug,
        },
        komga: {
          books: komgaBooks.map((b) => ({
            id: b.id,
            name: b.name,
            url: b.url,
            seriesId: b.seriesId,
            seriesTitle: b.seriesTitle,
            matchKey: getMatchKey(b.url),
            pageCount: b.media?.pagesCount ?? null,
            readProgress: b.readProgress ?? null,
          })),
        },
        codex: {
          books: codexBooks.map((b) => ({
            id: b.id,
            filePath: b.filePath,
            seriesId: b.seriesId,
            seriesName: b.seriesName,
            matchKey: getMatchKey(b.filePath),
            pageCount: b.pageCount,
            readProgress: b.readProgress ?? null,
          })),
        },
        analysis: {
          matches: results
            .filter((r) => r.status === "match")
            .map((r) => ({
              komgaId: r.komgaBook.id,
              komgaName: r.komgaBook.name,
              komgaUrl: r.komgaBook.url,
              komgaSeriesTitle: r.komgaBook.seriesTitle,
              codexId: r.codexBook?.id,
              codexFilePath: r.codexBook?.filePath,
              codexSeriesName: r.codexBook?.seriesName,
              matchKey: getMatchKey(r.komgaBook.url),
              needsSync: r.progressDiff.needsSync,
              komgaProgress: r.progressDiff.komga,
              codexProgress: r.progressDiff.codex,
            })),
          notFound: results
            .filter((r) => r.status === "not-found")
            .map((r) => ({
              komgaId: r.komgaBook.id,
              komgaName: r.komgaBook.name,
              komgaUrl: r.komgaBook.url,
              komgaSeriesTitle: r.komgaBook.seriesTitle,
              matchKey: getMatchKey(r.komgaBook.url),
              komgaProgress: r.progressDiff.komga,
            })),
        },
        actions: [] as Array<{
          komgaId: string;
          komgaName: string;
          codexId: string;
          action: "markAsRead" | "updateProgress";
          fromProgress: { page: number; completed: boolean } | null;
          toProgress: { page: number; completed: boolean };
          success: boolean;
          status?: number;
          error?: string;
        }>,
        summary: {
          totalKomgaWithProgress: komgaBooks.length,
          totalCodex: codexBooks.length,
          matched: results.filter((r) => r.status === "match").length,
          notFound: results.filter((r) => r.status === "not-found").length,
          needsSync: results.filter((r) => r.progressDiff.needsSync).length,
          synced: 0,
          errors: 0,
          applied: false,
        },
      }
    : null;

  // Display results if not exporting
  if (!values.export) {
    printResults(groups, values["show-all"] ?? false);
  }

  // Apply if not dry run
  if (!dryRun) {
    const toSync = results.filter((r) => r.progressDiff.needsSync && r.codexBook);

    if (toSync.length === 0) {
      console.log("\nNothing to sync!");
    } else {
      console.log(`\n${COLORS.bold}Syncing ${toSync.length} books...${COLORS.reset}`);

      let synced = 0;
      let errors = 0;

      for (const result of toSync) {
        const book = result.codexBook!;
        const progress = result.progressDiff.komga!;

        let apiResult: { ok: boolean; status?: number; error?: string };
        let action: "markAsRead" | "updateProgress";

        try {
          if (progress.completed) {
            action = "markAsRead";
            apiResult = await codex.markAsRead(book.id);
          } else {
            action = "updateProgress";
            apiResult = await codex.updateProgress(book.id, progress.page, progress.completed);
          }
        } catch (err) {
          action = progress.completed ? "markAsRead" : "updateProgress";
          apiResult = { ok: false, error: err instanceof Error ? err.message : String(err) };
        }

        // Record action in export data
        if (exportData) {
          exportData.actions.push({
            komgaId: result.komgaBook.id,
            komgaName: result.komgaBook.name,
            codexId: book.id,
            action,
            fromProgress: result.progressDiff.codex,
            toProgress: progress,
            success: apiResult.ok,
            status: apiResult.status,
            ...(apiResult.error && { error: apiResult.error }),
          });
        }

        if (apiResult.ok) {
          synced++;
          console.log(`  ${COLORS.green}[OK]${COLORS.reset} ${result.komgaBook.name}`);
        } else {
          errors++;
          console.log(`  ${COLORS.red}[FAIL]${COLORS.reset} ${result.komgaBook.name}${apiResult.error ? `: ${apiResult.error}` : ""}`);
        }
      }

      // Update summary
      if (exportData) {
        exportData.summary.synced = synced;
        exportData.summary.errors = errors;
        exportData.summary.applied = true;
      }

      console.log(`\n${COLORS.bold}Done!${COLORS.reset} Synced: ${synced}, Errors: ${errors}`);
    }
  } else {
    const toSync = results.filter((r) => r.progressDiff.needsSync && r.codexBook);
    if (toSync.length > 0) {
      console.log(`\n${COLORS.dim}Run with --apply to sync ${toSync.length} books${COLORS.reset}`);
    }
  }

  // Export if requested (after actions are recorded)
  if (values.export && exportData) {
    await writeFile(values.export, JSON.stringify(exportData, null, 2));
    console.log(`\nExported to ${values.export}`);

    // Print summary
    console.log(`\n${"=".repeat(60)}`);
    console.log(`${COLORS.bold}Summary${COLORS.reset}`);
    console.log(`  Total books in Komga (with progress): ${exportData.summary.totalKomgaWithProgress}`);
    console.log(`  Total books in Codex: ${exportData.summary.totalCodex}`);
    console.log(`  ${COLORS.green}Matched:${COLORS.reset} ${exportData.summary.matched}`);
    console.log(`  ${COLORS.red}Not found in Codex:${COLORS.reset} ${exportData.summary.notFound}`);
    console.log(`  ${COLORS.yellow}Need sync:${COLORS.reset} ${exportData.summary.needsSync}`);
    if (exportData.summary.applied) {
      console.log(`  ${COLORS.green}Synced:${COLORS.reset} ${exportData.summary.synced}`);
      console.log(`  ${COLORS.red}Errors:${COLORS.reset} ${exportData.summary.errors}`);
    }
  }
}

main().catch((err) => {
  console.error("Error:", err.message);
  process.exit(1);
});
