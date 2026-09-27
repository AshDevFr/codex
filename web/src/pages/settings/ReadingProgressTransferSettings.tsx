import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  Divider,
  FileButton,
  Group,
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
import { useRef, useState } from "react";
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

function SummaryLine({ report }: { report: ImportReadingProgressResponse }) {
  const { summary } = report;
  return (
    <Group gap="lg" wrap="wrap">
      <Text size="sm">
        Series: <b>{summary.series_matched}</b> matched,{" "}
        <b>{summary.series_ambiguous}</b> ambiguous,{" "}
        <b>{summary.series_unmatched}</b> unmatched
        {!report.dry_run && (
          <>
            , <b>{summary.series_committed}</b> committed
          </>
        )}
      </Text>
      <Text size="sm">
        Books: <b>{summary.books_matched}</b> matched,{" "}
        <b>{summary.books_stem_matched}</b> stem match,{" "}
        <b>{summary.books_ambiguous}</b> ambiguous,{" "}
        <b>{summary.books_unmatched}</b> unmatched,{" "}
        <b>{summary.books_hash_mismatch}</b> hash mismatch
      </Text>
      <Text size="sm">
        Progress written: <b>{summary.progress_written}</b> &middot;
        Completions: <b>{summary.completions_inserted}</b> inserted /{" "}
        <b>{summary.completions_reattached}</b> reattached &middot; Sessions:{" "}
        <b>{summary.sessions_inserted}</b> inserted /{" "}
        <b>{summary.sessions_reattached}</b> reattached &middot; Ratings:{" "}
        <b>{summary.ratings_written}</b>
      </Text>
    </Group>
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
              key={`${index}-${series.library_relative_path}-${series.name}`}
            >
              <Table.Td>
                <Text size="sm" fw={500}>
                  {series.name}
                </Text>
                <Text size="xs" c="dimmed">
                  {series.library_relative_path}
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
                      key={`${book.path}-${book.file_name}`}
                      size="sm"
                      variant="dot"
                      color={bookDispositionColor(book.disposition)}
                      title={book.path}
                    >
                      {book.file_name}
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
  const [includeSessions, setIncludeSessions] = useState(true);

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
        dry_run: dryRun,
        hash_mode: hashMode,
        source_preference: [],
        conflict_policy: conflictPolicy,
        reattach_sessions: reattachSessions,
        accept_stem_matches: acceptStemMatches,
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
              onClick={() => exportMutation.mutate(includeSessions)}
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
                  report.dry_run
                    ? "Preview (nothing written yet)"
                    : "Import result"
                }
                labelPosition="left"
              />
              {!report.sessions_in_file && (
                <Text size="xs" c="dimmed">
                  This file does not include sessions.
                </Text>
              )}
              {(report.notices ?? []).map((notice) => (
                <Text key={notice} size="xs" c="dimmed">
                  {notice}
                </Text>
              ))}
              <SummaryLine report={report} />
              <ReportTable report={report} />
            </Stack>
          )}
        </Stack>
      </Card>
    </Stack>
  );
}
