import {
  ActionIcon,
  Badge,
  Button,
  Card,
  Group,
  Menu,
  Modal,
  Progress,
  ScrollArea,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconCheck,
  IconCloudCheck,
  IconCloudDownload,
  IconDeviceFloppy,
  IconExclamationCircle,
  IconX,
} from "@tabler/icons-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { shouldShowInstallNudge } from "@/lib/offline/installNudge";
import {
  downloadSeriesBatch,
  QuotaExceededError,
  type SeriesBookSummary,
  type SeriesDownloadController,
  type SeriesQueueState,
} from "@/lib/offline/seriesDownloadQueue";
import { InstallNudgeModal } from "./InstallNudgeModal";

/**
 * Phase 12 T5: "Download series" entry point for SeriesDetail.
 *
 * Renders a primary action that opens a modal listing every book in the
 * series with its planned size and current state. Confirming kicks off
 * `downloadSeriesBatch`; the modal then renders per-book progress + per-
 * book cancel + a queue-wide "Cancel all". Closing the modal while the
 * queue is running keeps it running (button shows compact aggregate
 * progress); reopening re-attaches the listener to the existing
 * controller.
 *
 * Pre-flight `QuotaExceededError` surfaces as a destructive notification
 * and a banner inside the modal; the queue never starts, so no IDB rows
 * or per-book caches are written.
 */

export interface SeriesDownloadButtonProps {
  seriesId: string;
  books: SeriesBookSummary[];
  /** Optional label for the button. Defaults to "Download series". */
  label?: string;
  /**
   * Optional series-archive URL. When provided the dropdown exposes a
   * "Download as archive" action that links directly to the URL alongside
   * "Save series for offline". Lets SeriesDetail fold the legacy
   * `/api/v1/series/:id/download` link into the same control instead of
   * rendering a second adjacent button.
   */
  archiveDownloadUrl?: string;
}

type Phase =
  | { kind: "idle" }
  | { kind: "preflight-error"; message: string }
  | {
      kind: "running";
      controller: SeriesDownloadController;
      state: SeriesQueueState;
    }
  | { kind: "done"; result: SeriesQueueState };

function statusColor(status: string): string {
  switch (status) {
    case "complete":
      return "green";
    case "downloading":
      return "blue";
    case "error":
      return "red";
    case "cancelled":
      return "gray";
    case "skipped":
      return "gray";
    default:
      return "gray";
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case "complete":
      return "Saved";
    case "downloading":
      return "Downloading";
    case "queued":
      return "Queued";
    case "error":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "skipped":
      return "Skipped";
    default:
      return status;
  }
}

function bookProgressPercent(loaded: number, total: number | null): number {
  if (total === null || total <= 0) return 0;
  return Math.min(100, Math.round((loaded / total) * 100));
}

export function SeriesDownloadButton({
  seriesId,
  books,
  label = "Download series",
  archiveDownloadUrl,
}: SeriesDownloadButtonProps) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [opened, { open, close }] = useDisclosure(false);
  const [nudgeOpen, setNudgeOpen] = useState(false);
  // Hold the controller in a ref so cancel handlers can reach it without
  // forcing the listener to close over fresh closures.
  const controllerRef = useRef<SeriesDownloadController | null>(null);
  // Effects below depend on controller identity, not the whole `phase`
  // object: the subscribe listener mutates `phase.state` on every emit,
  // which would otherwise re-trigger the subscribe effect in a loop.
  const activeController = phase.kind === "running" ? phase.controller : null;

  // Unsubscribe is owned by the running phase; reset on transitions.
  useEffect(() => {
    if (!activeController) return;
    const unsubscribe = activeController.subscribe((s) => {
      setPhase((prev) =>
        prev.kind === "running" ? { ...prev, state: cloneState(s) } : prev,
      );
    });
    return unsubscribe;
  }, [activeController]);

  // When the queue resolves, flip to `done` and surface a notification.
  useEffect(() => {
    if (!activeController) return;
    let cancelled = false;
    const ctrl = activeController;
    ctrl.done.then((result) => {
      if (cancelled) return;
      const finalState = cloneState(ctrl.getState());
      setPhase({ kind: "done", result: finalState });
      controllerRef.current = null;
      const total = result.completed.length + result.failed.length;
      if (result.failed.length === 0 && result.cancelled.length === 0) {
        notifications.show({
          color: "green",
          title: "Series saved offline",
          message: `${result.completed.length} book${result.completed.length === 1 ? "" : "s"} downloaded.`,
        });
      } else if (result.failed.length > 0) {
        notifications.show({
          color: "orange",
          title: "Series partially downloaded",
          message: `${result.completed.length}/${total} books saved, ${result.failed.length} failed.`,
        });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [activeController]);

  const startInternal = useCallback(async () => {
    try {
      const controller = await downloadSeriesBatch({
        seriesId,
        books,
      });
      controllerRef.current = controller;
      setPhase({
        kind: "running",
        controller,
        state: cloneState(controller.getState()),
      });
    } catch (err) {
      if (err instanceof QuotaExceededError) {
        const message = err.message;
        setPhase({ kind: "preflight-error", message });
        notifications.show({
          color: "red",
          title: "Not enough storage",
          message,
        });
      } else {
        const message = err instanceof Error ? err.message : String(err);
        notifications.show({
          color: "red",
          title: "Could not start series download",
          message,
        });
      }
    }
  }, [seriesId, books]);

  const handleStart = useCallback(() => {
    // T10: iOS Safari tab gets the install nudge before the batch starts.
    // Continue runs `startInternal`; dismissal just closes the nudge and
    // leaves the user on the confirmation panel so they can opt back in.
    if (shouldShowInstallNudge()) {
      setNudgeOpen(true);
      return;
    }
    void startInternal();
  }, [startInternal]);

  const handleCancelBook = useCallback((bookId: string) => {
    controllerRef.current?.cancelBook(bookId);
  }, []);

  const handleCancelAll = useCallback(() => {
    controllerRef.current?.cancelAll();
  }, []);

  const aggregate = (() => {
    if (phase.kind === "running") {
      return {
        completed: phase.state.completed,
        total: phase.state.total,
        failed: phase.state.failed,
      };
    }
    if (phase.kind === "done") {
      return {
        completed: phase.result.completed,
        total: phase.result.total,
        failed: phase.result.failed,
      };
    }
    return null;
  })();

  const supportedCount = books.filter((b) =>
    ["epub", "pdf", "cbz", "cbr"].includes(b.fileFormat),
  ).length;
  const allDone =
    phase.kind === "done" &&
    phase.result.failed === 0 &&
    phase.result.cancelled === 0 &&
    phase.result.completed === supportedCount;

  // Primary button: when an archive URL is provided, the visible Button
  // opens a Menu so the user can choose between "Save series for offline"
  // and "Download as archive". Without the archive URL we just open the
  // modal directly (legacy single-purpose UX).
  const primaryButton = archiveDownloadUrl ? (
    <Menu shadow="md" width={260} position="bottom-start">
      <Menu.Target>
        <Button
          size="xs"
          variant={allDone ? "light" : "outline"}
          color={allDone ? "green" : undefined}
          leftSection={
            allDone ? (
              <IconCloudCheck size={14} />
            ) : (
              <IconCloudDownload size={14} />
            )
          }
          aria-label={label}
        >
          {label}
        </Button>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Item leftSection={<IconCloudDownload size={14} />} onClick={open}>
          Save series for offline
        </Menu.Item>
        <Menu.Item
          leftSection={<IconDeviceFloppy size={14} />}
          component="a"
          href={archiveDownloadUrl}
        >
          Download as archive
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  ) : (
    <Button
      size="xs"
      variant={allDone ? "light" : "outline"}
      color={allDone ? "green" : undefined}
      leftSection={
        allDone ? <IconCloudCheck size={14} /> : <IconCloudDownload size={14} />
      }
      onClick={open}
      aria-label={label}
    >
      {label}
    </Button>
  );

  return (
    <>
      <Group gap={6} wrap="nowrap" align="center">
        {primaryButton}
        {phase.kind === "running" && aggregate && (
          <Tooltip
            label={`Downloading ${aggregate.completed} of ${aggregate.total}`}
          >
            <Badge size="sm" color="blue" variant="filled">
              {aggregate.completed}/{aggregate.total}
            </Badge>
          </Tooltip>
        )}
      </Group>

      <Modal
        opened={opened}
        onClose={close}
        title={label}
        size="lg"
        scrollAreaComponent={ScrollArea.Autosize}
        centered
      >
        <Stack gap="md">
          {phase.kind === "idle" && (
            <Stack gap="sm">
              <Text size="sm">
                Save every supported book in this series to this device for
                offline reading. Downloads happen one book at a time so the
                queue does not flood the network.
              </Text>
              <BookList books={books} />
              <Group justify="flex-end">
                <Button variant="subtle" onClick={close}>
                  Cancel
                </Button>
                <Button
                  leftSection={<IconCloudDownload size={14} />}
                  onClick={handleStart}
                  disabled={supportedCount === 0}
                >
                  Start downloading
                </Button>
              </Group>
            </Stack>
          )}

          {phase.kind === "preflight-error" && (
            <Stack gap="sm">
              <Card withBorder padding="sm" bg="red.0">
                <Group gap={8} align="flex-start" wrap="nowrap">
                  <IconExclamationCircle
                    size={20}
                    color="var(--mantine-color-red-6)"
                  />
                  <Text size="sm">{phase.message}</Text>
                </Group>
              </Card>
              <Text size="sm" c="dimmed">
                Free up storage on this device or remove existing offline
                downloads from Settings &rarr; Offline downloads, then try
                again.
              </Text>
              <Group justify="flex-end">
                <Button variant="subtle" onClick={close}>
                  Close
                </Button>
              </Group>
            </Stack>
          )}

          {phase.kind === "running" && (
            <Stack gap="sm">
              <Group justify="space-between" wrap="nowrap">
                <Text size="sm">
                  {aggregate?.completed} of {aggregate?.total} complete
                  {aggregate && aggregate.failed > 0
                    ? ` (${aggregate.failed} failed)`
                    : ""}
                </Text>
                <Button
                  size="xs"
                  variant="subtle"
                  color="red"
                  onClick={handleCancelAll}
                >
                  Cancel all
                </Button>
              </Group>
              {aggregate && (
                <Progress
                  value={Math.round(
                    ((aggregate.completed + aggregate.failed) /
                      Math.max(1, aggregate.total)) *
                      100,
                  )}
                  size="md"
                />
              )}
              <QueueList state={phase.state} onCancelBook={handleCancelBook} />
            </Stack>
          )}

          {phase.kind === "done" && (
            <Stack gap="sm">
              <Group gap={8} align="center">
                <IconCheck size={20} color="var(--mantine-color-green-6)" />
                <Text size="sm">
                  Done. {phase.result.completed} downloaded,{" "}
                  {phase.result.failed} failed, {phase.result.cancelled}{" "}
                  cancelled.
                </Text>
              </Group>
              <QueueList state={phase.result} readOnly />
              <Group justify="flex-end">
                <Button variant="subtle" onClick={close}>
                  Close
                </Button>
              </Group>
            </Stack>
          )}
        </Stack>
      </Modal>

      <InstallNudgeModal
        opened={nudgeOpen}
        onContinue={() => {
          setNudgeOpen(false);
          void startInternal();
        }}
        onClose={() => setNudgeOpen(false)}
      />
    </>
  );
}

function cloneState(state: SeriesQueueState): SeriesQueueState {
  return {
    seriesId: state.seriesId,
    total: state.total,
    completed: state.completed,
    failed: state.failed,
    cancelled: state.cancelled,
    perBook: new Map(state.perBook),
  };
}

function BookList({ books }: { books: SeriesBookSummary[] }) {
  return (
    <Card withBorder padding="xs">
      <ScrollArea.Autosize mah={240}>
        <Stack gap={4}>
          {books.map((b) => {
            const supported = ["epub", "pdf", "cbz", "cbr"].includes(
              b.fileFormat,
            );
            return (
              <Group
                key={b.id}
                justify="space-between"
                gap="xs"
                wrap="nowrap"
                py={2}
                px={4}
              >
                <Text size="xs" ff="monospace" lineClamp={1}>
                  {b.id}
                </Text>
                <Group gap={6} wrap="nowrap">
                  <Badge size="xs" variant="light">
                    {b.fileFormat.toUpperCase()}
                  </Badge>
                  {!supported && (
                    <Badge size="xs" variant="outline" color="gray">
                      Unsupported
                    </Badge>
                  )}
                </Group>
              </Group>
            );
          })}
        </Stack>
      </ScrollArea.Autosize>
    </Card>
  );
}

interface QueueListProps {
  state: SeriesQueueState;
  onCancelBook?: (bookId: string) => void;
  readOnly?: boolean;
}

function QueueList({ state, onCancelBook, readOnly }: QueueListProps) {
  return (
    <Card withBorder padding="xs">
      <ScrollArea.Autosize mah={320}>
        <Stack gap={4}>
          {Array.from(state.perBook.values()).map((b) => {
            const pct = bookProgressPercent(b.loaded, b.total);
            return (
              <Stack key={b.bookId} gap={2} py={4} px={4}>
                <Group justify="space-between" wrap="nowrap">
                  <Text size="xs" ff="monospace" lineClamp={1}>
                    {b.bookId}
                  </Text>
                  <Group gap={6} wrap="nowrap">
                    <Badge
                      size="xs"
                      variant="filled"
                      color={statusColor(b.status)}
                    >
                      {statusLabel(b.status)}
                    </Badge>
                    {!readOnly &&
                      (b.status === "queued" || b.status === "downloading") && (
                        <Tooltip label="Cancel this book">
                          <ActionIcon
                            size="xs"
                            color="red"
                            variant="subtle"
                            onClick={() => onCancelBook?.(b.bookId)}
                            aria-label={`Cancel download of ${b.bookId}`}
                          >
                            <IconX size={12} />
                          </ActionIcon>
                        </Tooltip>
                      )}
                  </Group>
                </Group>
                {b.status === "downloading" && b.total !== null && (
                  <Progress value={pct} size="xs" />
                )}
                {b.status === "error" && b.error && (
                  <Text size="xs" c="red.6">
                    {b.error}
                  </Text>
                )}
              </Stack>
            );
          })}
        </Stack>
      </ScrollArea.Autosize>
    </Card>
  );
}
