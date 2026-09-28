import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  Divider,
  FileButton,
  Group,
  MultiSelect,
  Select,
  Stack,
  Table,
  Text,
  Title,
} from "@mantine/core";
import {
  IconAlertTriangle,
  IconCheck,
  IconDownload,
  IconUpload,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { librariesApi } from "@/api/libraries";
import type {
  BookDisposition,
  ConflictPolicy,
  HashMode,
  ImportReadingProgressResponse,
  ReadingProgressExportDocument,
  SeriesDisposition,
} from "@/api/readingProgressTransfer";
import {
  useExportReadingProgress,
  useImportReadingProgress,
} from "@/hooks/useReadingProgressTransfer";

type ApiErrorLike = Error & {
  response?: {
    status?: number;
    data?: { message?: string; error?: string };
  };
};

function errorMessage(error: unknown, fallback: string): string {
  const err = error as ApiErrorLike;
  if (err?.response?.status === 413) {
    return "This file is larger than the server accepts for an import (64 MB).";
  }
  return (
    err?.response?.data?.message ||
    err?.response?.data?.error ||
    err?.message ||
    fallback
  );
}

/**
 * Read a `File` as text via `FileReader` rather than `Blob.text()`: broader
 * runtime support (older WebViews, some test environments) for what is
 * otherwise a one-line read.
 */
function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () =>
      reject(reader.error ?? new Error("Failed to read file"));
    reader.readAsText(file);
  });
}

function seriesDispositionColor(disposition: SeriesDisposition): string {
  switch (disposition) {
    case "matched":
      return "green";
    case "ambiguous":
      return "yellow";
    default:
      return "gray";
  }
}

function bookDispositionColor(disposition: BookDisposition): string {
  switch (disposition) {
    case "matched":
      return "green";
    case "stem_match":
      return "blue";
    case "ambiguous":
      return "yellow";
    case "hash_mismatch":
      return "red";
    default:
      return "gray";
  }
}

/**
 * The import's headline counts.
 *
 * Scoped, the counts are stated in terms of the destination: a split exports a
 * whole old library and imports it into one of several new ones, so most file
 * series are meant to miss, and reporting them as "unmatched" made a complete
 * import read like a failure. Unscoped there is no destination to measure
 * against, and an unmatched series really was not found, so the plain counts
 * stay.
 */
export function SummaryLine({
  report,
  scopeLabel,
}: {
  report: ImportReadingProgressResponse;
  /** Name of the one selected library, or a phrase for several; null if unscoped. */
  scopeLabel: string | null;
}) {
  const { summary } = report;
  const committed = !report.dryRun && (
    <>
      , <b>{summary.seriesCommitted}</b> committed
    </>
  );
  const writes = (
    <Text size="sm">
      Progress written: <b>{summary.progressWritten}</b> &middot; Completions:{" "}
      <b>{summary.completionsInserted}</b> inserted /{" "}
      <b>{summary.completionsReattached}</b> reattached &middot; Sessions:{" "}
      <b>{summary.sessionsInserted}</b> inserted /{" "}
      <b>{summary.sessionsReattached}</b> reattached &middot; Ratings:{" "}
      <b>{summary.ratingsWritten}</b> &middot; Want to read:{" "}
      <b>{summary.wantToReadRestored}</b>
    </Text>
  );

  const seriesInScope = summary.seriesInSelectedLibraries;
  const booksInScope = summary.booksInSelectedLibraries;
  if (scopeLabel === null || seriesInScope == null || booksInScope == null) {
    return (
      <Group gap="lg" wrap="wrap">
        <Text size="sm">
          Series: <b>{summary.seriesMatched}</b> matched,{" "}
          <b>{summary.seriesAmbiguous}</b> ambiguous,{" "}
          <b>{summary.seriesUnmatched}</b> unmatched
          {committed}
        </Text>
        <Text size="sm">
          Books: <b>{summary.booksMatched}</b> matched,{" "}
          <b>{summary.booksStemMatched}</b> stem match,{" "}
          <b>{summary.booksAmbiguous}</b> ambiguous,{" "}
          <b>{summary.booksUnmatched}</b> unmatched,{" "}
          <b>{summary.booksHashMismatch}</b> hash mismatch
        </Text>
        {writes}
      </Group>
    );
  }

  // Books missed inside a series that did match are a real gap and stay
  // visible; only books whose whole series lives elsewhere are set aside.
  const booksMissed = summary.booksUnmatched - summary.booksInUnmatchedSeries;
  return (
    <Stack gap={4}>
      <Group gap="lg" wrap="wrap">
        <Text size="sm">
          Series: <b>{summary.seriesMatchedDistinct}</b> of the{" "}
          <b>{seriesInScope}</b> in {scopeLabel} matched,{" "}
          <b>{summary.seriesAmbiguous}</b> ambiguous
          {committed}
        </Text>
        <Text size="sm">
          Books: <b>{summary.booksMatchedDistinct}</b> of the{" "}
          <b>{booksInScope}</b> in {scopeLabel} matched, <b>{booksMissed}</b>{" "}
          missed, <b>{summary.booksStemMatched}</b> stem match,{" "}
          <b>{summary.booksAmbiguous}</b> ambiguous,{" "}
          <b>{summary.booksHashMismatch}</b> hash mismatch
        </Text>
        {writes}
      </Group>
      <Text size="xs" c="dimmed">
        The file also holds {summary.seriesUnmatched} series (
        {summary.booksInUnmatchedSeries} books) that belong to other libraries.
        That is expected when importing part of a split.
      </Text>
    </Stack>
  );
}

function ReportTable({ report }: { report: ImportReadingProgressResponse }) {
  return (
    <Table.ScrollContainer minWidth={600}>
      <Table striped highlightOnHover verticalSpacing="xs">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Series</Table.Th>
            <Table.Th>Disposition</Table.Th>
            <Table.Th>Committed</Table.Th>
            <Table.Th>Books</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {report.series.map((series, index) => (
            // A split exports same-named series from several libraries, so
            // path and name alone are not unique.
            <Table.Tr
              key={`${index}-${series.libraryRelativePath}-${series.name}`}
            >
              <Table.Td>
                <Text size="sm" fw={500}>
                  {series.name}
                </Text>
                <Text size="xs" c="dimmed">
                  {series.libraryRelativePath}
                </Text>
                {series.error && (
                  <Text size="xs" c="red">
                    {series.error}
                  </Text>
                )}
              </Table.Td>
              <Table.Td>
                <Badge
                  color={seriesDispositionColor(series.disposition)}
                  variant="light"
                >
                  {series.disposition}
                </Badge>
              </Table.Td>
              <Table.Td>
                {series.attempted ? (
                  series.committed ? (
                    <IconCheck size={16} color="var(--mantine-color-green-6)" />
                  ) : (
                    <Text size="xs" c="dimmed">
                      no
                    </Text>
                  )
                ) : (
                  <Text size="xs" c="dimmed">
                    &ndash;
                  </Text>
                )}
              </Table.Td>
              <Table.Td>
                <Group gap={4} wrap="wrap">
                  {series.books.map((book) => (
                    <Badge
                      key={`${book.path}-${book.fileName}`}
                      size="sm"
                      variant="dot"
                      color={bookDispositionColor(book.disposition)}
                      title={book.path}
                    >
                      {book.fileName}
                    </Badge>
                  ))}
                </Group>
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

export function ReadingProgressTransferSettings() {
  const { data: libraries = [] } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });
  const libraryOptions = libraries.map((library) => ({
    value: library.id,
    label: library.name,
  }));

  const [includeSessions, setIncludeSessions] = useState(true);
  const [exportLibraryIds, setExportLibraryIds] = useState<string[]>([]);
  const [importLibraryIds, setImportLibraryIds] = useState<string[]>([]);
  const [sourcePreference, setSourcePreference] = useState<string[]>([]);
  const [restoreWantToRead, setRestoreWantToRead] = useState(true);

  const [file, setFile] = useState<File | null>(null);
  const [parsedDocument, setParsedDocument] =
    useState<ReadingProgressExportDocument | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);

  const [conflictPolicy, setConflictPolicy] =
    useState<ConflictPolicy>("newest");
  const [hashMode, setHashMode] = useState<HashMode>("verify");
  const [reattachSessions, setReattachSessions] = useState(true);
  const [acceptStemMatches, setAcceptStemMatches] = useState(false);

  const [report, setReport] = useState<ImportReadingProgressResponse | null>(
    null,
  );
  // Every change to the file or an option starts a new generation. A preview
  // unlocks Apply only for the generation it was run against, so a dry run
  // still in flight when an option changes cannot unlock Apply for options
  // it never previewed.
  const generation = useRef(0);
  const [previewedGeneration, setPreviewedGeneration] = useState<number | null>(
    null,
  );
  const [importError, setImportError] = useState<string | null>(null);

  const availableSources = useMemo(() => {
    const seen = new Set<string>();
    for (const series of parsedDocument?.series ?? []) {
      for (const external of series.externalIds ?? [])
        seen.add(external.source);
    }
    return [...seen];
  }, [parsedDocument]);

  const scopeLabel =
    importLibraryIds.length === 0
      ? null
      : importLibraryIds.length === 1
        ? (libraries.find((library) => library.id === importLibraryIds[0])
            ?.name ?? "the selected library")
        : "the selected libraries";

  const exportMutation = useExportReadingProgress();
  const importMutation = useImportReadingProgress();

  const clearPreview = () => {
    generation.current += 1;
    setReport(null);
    setPreviewedGeneration(null);
    setImportError(null);
  };

  const handleFile = async (selected: File | null) => {
    setFile(selected);
    setParsedDocument(null);
    setParseError(null);
    clearPreview();
    if (!selected) return;
    try {
      const text = await readFileAsText(selected);
      setParsedDocument(JSON.parse(text) as ReadingProgressExportDocument);
    } catch {
      setParseError("This file is not valid JSON.");
    }
  };

  const runImport = (dryRun: boolean) => {
    if (!parsedDocument) return;
    const requestedFor = generation.current;
    setImportError(null);
    importMutation.mutate(
      {
        dryRun,
        hashMode,
        sourcePreference:
          sourcePreference.length > 0 ? sourcePreference : undefined,
        libraryIds: importLibraryIds.length > 0 ? importLibraryIds : undefined,
        restoreWantToRead,
        conflictPolicy,
        reattachSessions,
        acceptStemMatches,
        file: parsedDocument,
      },
      {
        onSuccess: (response) => {
          if (requestedFor !== generation.current) return;
          setReport(response);
          setPreviewedGeneration(dryRun ? requestedFor : null);
        },
        onError: (error) => {
          if (requestedFor !== generation.current) return;
          setImportError(errorMessage(error, "Import failed."));
          setReport(null);
          // A failed apply may have committed some series, so the old
          // preview no longer describes what applying would do.
          setPreviewedGeneration(null);
        },
      },
    );
  };

  const previewed =
    previewedGeneration !== null && previewedGeneration === generation.current;
  const canApply =
    Boolean(parsedDocument) && previewed && !importMutation.isPending;
  const busy = importMutation.isPending;

  return (
    <Stack gap="lg">
      <div>
        <Title order={2}>Reading Progress</Title>
        <Text size="sm" c="dimmed" mt={4}>
          Export your reading progress, completions, sessions, and ratings to a
          file, and import it back after reorganising or moving your library.
        </Text>
      </div>

      <Alert
        color="yellow"
        icon={<IconAlertTriangle size={16} />}
        title="Export before you delete"
      >
        Take an export <b>before</b> deleting an old library during a split or
        migration. History survives a hard delete as orphaned rows, but nothing
        except an export taken beforehand can say which book an orphan used to
        belong to.
      </Alert>

      <Card withBorder>
        <Stack gap="sm">
          <Title order={4}>Export</Title>
          <MultiSelect
            label="Libraries"
            description="Leave empty to export every library. Narrowing to the one you are reorganising keeps the file small and gives the import less to match against."
            placeholder={
              exportLibraryIds.length === 0 ? "All libraries" : undefined
            }
            data={libraryOptions}
            value={exportLibraryIds}
            onChange={setExportLibraryIds}
            disabled={exportMutation.isPending}
            clearable
            searchable
          />
          <Checkbox
            label="Include the reading-session log"
            description="Sessions are the only source of every reading statistic; leave this on unless you specifically want a smaller file."
            checked={includeSessions}
            onChange={(event) =>
              setIncludeSessions(event.currentTarget.checked)
            }
          />
          <Group>
            <Button
              leftSection={<IconDownload size={16} />}
              loading={exportMutation.isPending}
              onClick={() =>
                exportMutation.mutate({
                  includeSessions,
                  libraryIds: exportLibraryIds,
                })
              }
            >
              Download export
            </Button>
          </Group>
        </Stack>
      </Card>

      <Card withBorder>
        <Stack gap="sm">
          <Title order={4}>Import</Title>

          <Group align="flex-end">
            <FileButton
              onChange={handleFile}
              accept="application/json"
              disabled={busy}
            >
              {(props) => (
                <Button
                  {...props}
                  variant="default"
                  leftSection={<IconUpload size={16} />}
                >
                  {file ? file.name : "Choose export file"}
                </Button>
              )}
            </FileButton>
          </Group>

          {parseError && (
            <Alert color="red" icon={<IconAlertTriangle size={16} />}>
              {parseError}
            </Alert>
          )}

          <Divider label="Options" labelPosition="left" />

          <MultiSelect
            label="Match into these libraries"
            description="Leave empty to search every library. Naming the target library is what lets an import run before the old library has been rescanned: otherwise both copies of a series match and the import reports it as ambiguous."
            placeholder={
              importLibraryIds.length === 0 ? "All libraries" : undefined
            }
            data={libraryOptions}
            value={importLibraryIds}
            disabled={busy}
            onChange={(value) => {
              setImportLibraryIds(value);
              clearPreview();
            }}
            clearable
            searchable
          />

          {availableSources.length > 0 && (
            <MultiSelect
              label="External-id sources, most trusted first"
              description="An external id is the only key that survives both a rename and a move, so it is tried before the path and the name. Pick sources in the order you trust them; leave empty to try every source the file carries, in the order it lists them."
              placeholder={
                sourcePreference.length === 0
                  ? "All sources in the file"
                  : undefined
              }
              data={availableSources.map((source) => ({
                value: source,
                label: source,
              }))}
              value={sourcePreference}
              disabled={busy}
              onChange={(value) => {
                setSourcePreference(value);
                clearPreview();
              }}
              clearable
            />
          )}

          <Group grow>
            <Select
              label="Conflict policy"
              description="How to resolve a book/rating that already has progress on this side"
              data={[
                { value: "newest", label: "Newest (by updated_at)" },
                { value: "furthest", label: "Furthest into the book" },
                { value: "skip_existing", label: "Skip existing" },
                { value: "overwrite", label: "Overwrite" },
              ]}
              value={conflictPolicy}
              disabled={busy}
              onChange={(value) => {
                if (value) setConflictPolicy(value as ConflictPolicy);
                clearPreview();
              }}
            />
            <Select
              label="Hash mode"
              description="How aggressively to use file hashes when matching books"
              data={[
                { value: "off", label: "Off" },
                { value: "verify", label: "Verify (default)" },
                { value: "match", label: "Match (rescue a bulk rename)" },
              ]}
              value={hashMode}
              disabled={busy}
              onChange={(value) => {
                if (value) setHashMode(value as HashMode);
                clearPreview();
              }}
            />
          </Group>

          <Checkbox
            label="Reattach orphaned sessions and completions"
            description="When a session or completion already exists as yours but is not on a book that is still on disk (its book was deleted, or its file moved), move it onto the matched book instead of skipping it."
            checked={reattachSessions}
            disabled={busy}
            onChange={(event) => {
              setReattachSessions(event.currentTarget.checked);
              clearPreview();
            }}
          />
          <Checkbox
            label="Accept filename-stem matches"
            description="Apply a book match found only by filename stem (e.g. a .cbr renamed to .cbz). Two files can share a stem, so this is off by default."
            checked={acceptStemMatches}
            disabled={busy}
            onChange={(event) => {
              setAcceptStemMatches(event.currentTarget.checked);
              clearPreview();
            }}
          />

          <Checkbox
            label="Restore want to read"
            description="Put series and books that were queued on the old library back in your want-to-read list. They go after anything already queued, in their original order; anything already queued keeps its place."
            checked={restoreWantToRead}
            disabled={busy}
            onChange={(event) => {
              setRestoreWantToRead(event.currentTarget.checked);
              clearPreview();
            }}
          />

          {importError && (
            <Alert
              color="red"
              icon={<IconAlertTriangle size={16} />}
              title="Import failed"
            >
              {importError}
            </Alert>
          )}

          <Group>
            <Button
              variant="default"
              disabled={!parsedDocument}
              loading={busy && !previewed}
              onClick={() => runImport(true)}
            >
              Preview (dry run)
            </Button>
            <Button
              disabled={!canApply}
              loading={busy && previewed}
              onClick={() => runImport(false)}
            >
              Apply import
            </Button>
          </Group>

          {report && (
            <Stack gap="sm" mt="sm">
              <Divider
                label={
                  report.dryRun
                    ? "Preview (nothing written yet)"
                    : "Import result"
                }
                labelPosition="left"
              />
              {!report.sessionsInFile && (
                <Text size="xs" c="dimmed">
                  This file does not include sessions.
                </Text>
              )}
              {(report.notices ?? []).map((notice) => (
                <Text key={notice} size="xs" c="dimmed">
                  {notice}
                </Text>
              ))}
              <SummaryLine report={report} scopeLabel={scopeLabel} />
              <ReportTable report={report} />
            </Stack>
          )}
        </Stack>
      </Card>
    </Stack>
  );
}
