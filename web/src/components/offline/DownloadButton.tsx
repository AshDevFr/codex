import {
  ActionIcon,
  Group,
  Menu,
  RingProgress,
  Text,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconCloudCheck,
  IconCloudDownload,
  IconDotsVertical,
  IconRefresh,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { useEffect, useRef, useState } from "react";
import {
  broadcastDownloadsChange,
  DOWNLOADS_BROADCAST_CHANNEL,
  type DownloadsBroadcast,
  deleteDownload,
  getDownload,
} from "@/lib/offline/db";
import {
  type ComicFormat,
  downloadComicBook,
  downloadSingleFileBook,
  type ProgressUpdate,
  type SingleFileFormat,
} from "@/lib/offline/downloadManager";
import { shouldShowInstallNudge } from "@/lib/offline/installNudge";
import { cacheNameForBook } from "@/lib/offline/routeMatcher";
import { InstallNudgeModal } from "./InstallNudgeModal";

/**
 * Phase 12 T8: per-book download button.
 *
 * Renders a single ActionIcon (or icon + ring) that hydrates from IDB on
 * mount, subscribes to the downloads BroadcastChannel for cross-tab updates,
 * and dispatches to the right `downloadManager` entry point on click:
 * `downloadSingleFileBook` for EPUB/PDF, `downloadComicBook` for CBZ/CBR.
 * Five visible states cycle through `loading` -> `not-downloaded` ->
 * `downloading` (RingProgress + cancel) -> `downloaded` (Menu) or `error`.
 *
 * Series batch (T5) wraps this component in a queue; the series-level
 * "Download series" button is intentionally not part of this slice.
 */

type ButtonState =
  | { kind: "loading" }
  | { kind: "not-downloaded" }
  | { kind: "downloading"; loaded: number; total: number | null }
  | { kind: "downloaded"; bytes: number }
  | { kind: "error"; message: string };

export type DownloadButtonFormat =
  | SingleFileFormat
  | ComicFormat
  | (string & {});

export interface DownloadButtonProps {
  bookId: string;
  /** Lowercase book file format from the API (e.g. "epub", "pdf", "cbz"). */
  fileFormat: DownloadButtonFormat;
  /**
   * Total page count. Required for comic formats so the per-page download
   * knows how many pages to fetch; ignored for single-file formats but
   * accepted for callers (BookDetail) that always have it on hand.
   */
  pageCount?: number;
  /** Tooltip / menu label, defaults to "Save for offline reading". */
  label?: string;
}

function isSingleFileFormat(format: string): format is SingleFileFormat {
  return format === "epub" || format === "pdf";
}

function isComicFormat(format: string): format is ComicFormat {
  return format === "cbz" || format === "cbr";
}

function progressPercent(state: ButtonState): number {
  if (state.kind !== "downloading") return 0;
  if (state.total === null || state.total <= 0) return 0;
  return Math.min(100, Math.round((state.loaded / state.total) * 100));
}

export function DownloadButton({
  bookId,
  fileFormat,
  pageCount,
  label = "Save for offline reading",
}: DownloadButtonProps) {
  const [state, setState] = useState<ButtonState>({ kind: "loading" });
  const [nudgeOpen, setNudgeOpen] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const supported =
    isSingleFileFormat(fileFormat) ||
    (isComicFormat(fileFormat) && (pageCount ?? 0) > 0);

  // Hydrate from IDB + subscribe to broadcast updates from other tabs.
  // Effect intentionally does not depend on `supported` so the listener
  // would still fire if T4 later flips comics into the supported set.
  useEffect(() => {
    let cancelled = false;

    async function hydrate() {
      try {
        const record = await getDownload(bookId);
        if (cancelled) return;
        // Only set state if we're still in the initial loading phase. The
        // user may have clicked the trigger during the async IDB read, in
        // which case `state.kind` is already "downloading" and we must not
        // clobber that with whatever was on disk.
        setState((prev) => {
          if (prev.kind !== "loading") return prev;
          if (!record) return { kind: "not-downloaded" };
          if (record.status === "complete") {
            return { kind: "downloaded", bytes: record.bytes };
          }
          if (record.status === "downloading") {
            return { kind: "downloading", loaded: record.bytes, total: null };
          }
          if (record.status === "error") {
            return {
              kind: "error",
              message: record.error ?? "Download failed",
            };
          }
          return { kind: "not-downloaded" };
        });
      } catch {
        if (!cancelled) {
          setState((prev) =>
            prev.kind === "loading" ? { kind: "not-downloaded" } : prev,
          );
        }
      }
    }

    void hydrate();

    let channel: BroadcastChannel | null = null;
    if (typeof BroadcastChannel !== "undefined") {
      channel = new BroadcastChannel(DOWNLOADS_BROADCAST_CHANNEL);
      channel.addEventListener("message", handleBroadcast);
    }

    function handleBroadcast(ev: MessageEvent<DownloadsBroadcast>) {
      const payload = ev.data;
      if (payload.kind === "delete" && payload.id === bookId) {
        setState({ kind: "not-downloaded" });
        return;
      }
      if (payload.kind === "clear") {
        setState({ kind: "not-downloaded" });
        return;
      }
      if (payload.kind === "put" && payload.record.id === bookId) {
        const r = payload.record;
        if (r.status === "complete") {
          setState({ kind: "downloaded", bytes: r.bytes });
        } else if (r.status === "downloading") {
          setState((prev) => {
            // Preserve the in-progress total/loaded from a local download in
            // flight; cross-tab broadcasts only carry the initial 0-byte row.
            if (prev.kind === "downloading") return prev;
            return { kind: "downloading", loaded: r.bytes, total: null };
          });
        } else if (r.status === "error") {
          setState({
            kind: "error",
            message: r.error ?? "Download failed",
          });
        }
      }
    }

    return () => {
      cancelled = true;
      if (channel) {
        channel.removeEventListener("message", handleBroadcast);
        channel.close();
      }
    };
  }, [bookId]);

  if (!supported) return null;

  function maybeNudgeThenDownload() {
    // T10: On a fresh iOS Safari tab, show the install nudge before the
    // first download instead of jumping straight in. After the user picks
    // Continue (or dismisses), `startDownload` runs as usual; subsequent
    // taps within the 30-day TTL skip the modal entirely.
    if (shouldShowInstallNudge()) {
      setNudgeOpen(true);
      return;
    }
    void startDownload();
  }

  async function startDownload() {
    const controller = new AbortController();
    abortRef.current = controller;
    const initialTotal = isComicFormat(fileFormat) ? (pageCount ?? null) : null;
    setState({ kind: "downloading", loaded: 0, total: initialTotal });
    const onProgress = (p: ProgressUpdate) => {
      setState({ kind: "downloading", loaded: p.loaded, total: p.total });
    };
    try {
      if (isSingleFileFormat(fileFormat)) {
        await downloadSingleFileBook({
          bookId,
          format: fileFormat,
          signal: controller.signal,
          onProgress,
        });
      } else if (isComicFormat(fileFormat) && (pageCount ?? 0) > 0) {
        await downloadComicBook({
          bookId,
          format: fileFormat,
          pageCount: pageCount as number,
          signal: controller.signal,
          onProgress,
        });
      } else {
        throw new Error(
          `Unsupported format for offline download: ${fileFormat}`,
        );
      }
      // Final "downloaded" state lands via the broadcast from the manager.
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") {
        // The manager already deleted the IDB row + cache on abort, but the
        // broadcast may not have arrived yet; reset local state immediately.
        setState({ kind: "not-downloaded" });
      } else {
        const message = err instanceof Error ? err.message : String(err);
        notifications.show({
          color: "red",
          title: "Download failed",
          message,
        });
      }
    } finally {
      abortRef.current = null;
    }
  }

  function cancelDownload() {
    abortRef.current?.abort();
  }

  async function removeDownload() {
    try {
      await deleteDownload(bookId);
      broadcastDownloadsChange({ kind: "delete", id: bookId });
      if (typeof caches !== "undefined") {
        await caches.delete(cacheNameForBook(bookId));
      }
      setState({ kind: "not-downloaded" });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      notifications.show({
        color: "red",
        title: "Could not remove offline copy",
        message,
      });
    }
  }

  if (state.kind === "loading") {
    return (
      <ActionIcon
        variant="subtle"
        size="md"
        disabled
        aria-label="Loading offline download status"
      >
        <IconCloudDownload size={18} />
      </ActionIcon>
    );
  }

  if (state.kind === "not-downloaded") {
    return (
      <>
        <Tooltip label={label}>
          <ActionIcon
            variant="subtle"
            size="md"
            onClick={maybeNudgeThenDownload}
            aria-label={label}
          >
            <IconCloudDownload size={18} />
          </ActionIcon>
        </Tooltip>
        <InstallNudgeModal
          opened={nudgeOpen}
          onContinue={() => {
            setNudgeOpen(false);
            void startDownload();
          }}
          onClose={() => setNudgeOpen(false)}
        />
      </>
    );
  }

  if (state.kind === "downloading") {
    const pct = progressPercent(state);
    return (
      <Group gap={4} wrap="nowrap" align="center">
        <Tooltip
          label={
            state.total
              ? `Downloading: ${pct}%`
              : `Downloading: ${state.loaded} bytes`
          }
        >
          <RingProgress
            size={28}
            thickness={3}
            sections={[{ value: pct, color: "blue" }]}
            aria-label="Download progress"
          />
        </Tooltip>
        <Tooltip label="Cancel download">
          <ActionIcon
            variant="subtle"
            size="sm"
            color="red"
            onClick={cancelDownload}
            aria-label="Cancel download"
          >
            <IconX size={14} />
          </ActionIcon>
        </Tooltip>
      </Group>
    );
  }

  if (state.kind === "error") {
    return (
      <Tooltip label={`Download failed: ${state.message}. Click to retry.`}>
        <ActionIcon
          variant="subtle"
          size="md"
          color="red"
          onClick={maybeNudgeThenDownload}
          aria-label="Retry download"
        >
          <IconAlertCircle size={18} />
        </ActionIcon>
      </Tooltip>
    );
  }

  // state.kind === "downloaded"
  return (
    <Menu shadow="md" width={220} position="bottom-end">
      <Menu.Target>
        <Tooltip label="Available offline">
          <ActionIcon
            variant="subtle"
            size="md"
            color="green"
            aria-label="Offline download options"
          >
            <IconCloudCheck size={18} />
          </ActionIcon>
        </Tooltip>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>
          <Text size="xs" c="dimmed">
            Saved offline
          </Text>
        </Menu.Label>
        <Menu.Item
          leftSection={<IconRefresh size={14} />}
          onClick={startDownload}
        >
          Re-download
        </Menu.Item>
        <Menu.Item
          leftSection={<IconTrash size={14} />}
          color="red"
          onClick={removeDownload}
        >
          Remove offline copy
        </Menu.Item>
        <Menu.Divider />
        <Menu.Item disabled leftSection={<IconDotsVertical size={14} />}>
          <Text size="xs" c="dimmed">
            More controls in Settings
          </Text>
        </Menu.Item>
      </Menu.Dropdown>
    </Menu>
  );
}
