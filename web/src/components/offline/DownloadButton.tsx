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
  downloadSingleFileBook,
  type ProgressUpdate,
  type SingleFileFormat,
} from "@/lib/offline/downloadManager";
import { cacheNameForBook } from "@/lib/offline/routeMatcher";

/**
 * Phase 12 T8: per-book download button.
 *
 * Renders a single ActionIcon (or icon + ring) that hydrates from IDB on
 * mount, subscribes to the downloads BroadcastChannel for cross-tab updates,
 * and triggers `downloadSingleFileBook` via T3 when the user clicks. Three
 * primary states are visible (`not-downloaded`, `downloading`, `downloaded`)
 * plus an `error` state that lets the user retry.
 *
 * Comic per-page downloads (T4) and series batch (T5) are not in this slice;
 * for unsupported formats the component renders `null` so it stays out of
 * the way until those tasks land.
 */

type ButtonState =
  | { kind: "loading" }
  | { kind: "not-downloaded" }
  | { kind: "downloading"; loaded: number; total: number | null }
  | { kind: "downloaded"; bytes: number }
  | { kind: "error"; message: string };

export type DownloadButtonFormat = SingleFileFormat | "comic" | string;

export interface DownloadButtonProps {
  bookId: string;
  /** Lowercase book file format from the API (e.g. "epub", "pdf", "cbz"). */
  fileFormat: DownloadButtonFormat;
  /** Tooltip / menu label, defaults to "Save for offline reading". */
  label?: string;
}

function isSingleFileFormat(format: string): format is SingleFileFormat {
  return format === "epub" || format === "pdf";
}

function progressPercent(state: ButtonState): number {
  if (state.kind !== "downloading") return 0;
  if (state.total === null || state.total <= 0) return 0;
  return Math.min(100, Math.round((state.loaded / state.total) * 100));
}

export function DownloadButton({
  bookId,
  fileFormat,
  label = "Save for offline reading",
}: DownloadButtonProps) {
  const [state, setState] = useState<ButtonState>({ kind: "loading" });
  const abortRef = useRef<AbortController | null>(null);
  const supported = isSingleFileFormat(fileFormat);

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

  async function startDownload() {
    const controller = new AbortController();
    abortRef.current = controller;
    setState({ kind: "downloading", loaded: 0, total: null });
    try {
      await downloadSingleFileBook({
        bookId,
        format: fileFormat as SingleFileFormat,
        signal: controller.signal,
        onProgress: (p: ProgressUpdate) => {
          setState({ kind: "downloading", loaded: p.loaded, total: p.total });
        },
      });
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
      <Tooltip label={label}>
        <ActionIcon
          variant="subtle"
          size="md"
          onClick={startDownload}
          aria-label={label}
        >
          <IconCloudDownload size={18} />
        </ActionIcon>
      </Tooltip>
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
          onClick={startDownload}
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
