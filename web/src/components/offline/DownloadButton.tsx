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
  IconDeviceFloppy,
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
 * Per-book download button.
 *
 * Renders a single ActionIcon (or icon + ring) that hydrates from IDB on
 * mount, subscribes to the downloads BroadcastChannel for cross-tab updates,
 * and dispatches to the right `downloadManager` entry point on click:
 * `downloadSingleFileBook` for EPUB/PDF, `downloadComicBook` for CBZ/CBR.
 * Five visible states cycle through `loading` -> `not-downloaded` ->
 * `downloading` (RingProgress + cancel) -> `downloaded` (Menu) or `error`.
 *
 * The series batch download wraps this component in a queue; the
 * series-level "Download series" button is a separate component.
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
  /**
   * Optional direct file URL. When provided, the button menu also exposes a
   * "Download file" action that links to this URL. Lets BookDetail collapse
   * its old `<a href={downloadUrl}>` button into the same dropdown so users
   * see one unambiguous Download surface instead of two adjacent buttons
   * that both say "Download".
   */
  fileDownloadUrl?: string;
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
  fileDownloadUrl,
}: DownloadButtonProps) {
  const [state, setState] = useState<ButtonState>({ kind: "loading" });
  const [nudgeOpen, setNudgeOpen] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const supported =
    isSingleFileFormat(fileFormat) ||
    (isComicFormat(fileFormat) && (pageCount ?? 0) > 0);

  // Hydrate from IDB + subscribe to broadcast updates from other tabs.
  // Effect intentionally does not depend on `supported` so the listener
  // would still fire if comics later get flipped into the supported set.
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

  // When we have a fallback file URL, we always render *something* (the
  // Menu with a "Download file" entry), even for formats that can't be
  // cached for offline reading.
  if (!supported && !fileDownloadUrl) return null;

  function maybeNudgeThenDownload() {
    // On a fresh iOS Safari tab, show the install nudge before the first
    // download instead of jumping straight in. After the user picks
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
        aria-label="Loading download options"
      >
        <IconCloudDownload size={18} />
      </ActionIcon>
    );
  }

  // Downloading state stays compact (ring progress + cancel) so the user can
  // see progress and bail mid-flight without opening a menu.
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

  // All other states (not-downloaded / error / downloaded) render the same
  // Menu shape so a single Download surface covers both the PWA offline cache
  // and the file URL. Distinct icons + colors disambiguate state at a glance.
  // We deliberately don't wrap the Menu.Target in a Tooltip — Mantine's Menu
  // forwards its click handler via cloneElement, and a Tooltip in between
  // intermittently swallows that click under test load. The aria-label below
  // carries the same accessible name.
  const targetIcon = (() => {
    if (state.kind === "downloaded") return <IconCloudCheck size={18} />;
    if (state.kind === "error") return <IconAlertCircle size={18} />;
    return <IconCloudDownload size={18} />;
  })();
  const targetColor =
    state.kind === "downloaded"
      ? "green"
      : state.kind === "error"
        ? "red"
        : undefined;
  const targetAria =
    state.kind === "downloaded"
      ? "Offline download options"
      : state.kind === "error"
        ? "Download options (retry available)"
        : "Download options";

  return (
    <>
      <Menu shadow="md" width={240} position="bottom-end">
        <Menu.Target>
          <ActionIcon
            variant="subtle"
            size="md"
            color={targetColor}
            aria-label={targetAria}
          >
            {targetIcon}
          </ActionIcon>
        </Menu.Target>
        <Menu.Dropdown>
          {state.kind === "downloaded" && (
            <>
              <Menu.Label>
                <Text size="xs" c="dimmed">
                  Saved offline
                </Text>
              </Menu.Label>
              {supported && (
                <Menu.Item
                  leftSection={<IconRefresh size={14} />}
                  onClick={startDownload}
                >
                  Re-download offline copy
                </Menu.Item>
              )}
              <Menu.Item
                leftSection={<IconTrash size={14} />}
                color="red"
                onClick={removeDownload}
              >
                Remove offline copy
              </Menu.Item>
            </>
          )}
          {state.kind === "not-downloaded" && supported && (
            <Menu.Item
              leftSection={<IconCloudDownload size={14} />}
              onClick={maybeNudgeThenDownload}
            >
              {label}
            </Menu.Item>
          )}
          {state.kind === "error" && supported && (
            <Menu.Item
              leftSection={<IconCloudDownload size={14} />}
              onClick={maybeNudgeThenDownload}
            >
              Retry offline download
            </Menu.Item>
          )}
          {fileDownloadUrl && (
            <>
              {state.kind !== "not-downloaded" && <Menu.Divider />}
              <Menu.Item
                leftSection={<IconDeviceFloppy size={14} />}
                component="a"
                href={fileDownloadUrl}
              >
                Download file
              </Menu.Item>
            </>
          )}
          {state.kind === "downloaded" && (
            <>
              <Menu.Divider />
              <Menu.Item disabled leftSection={<IconDotsVertical size={14} />}>
                <Text size="xs" c="dimmed">
                  More controls in Settings
                </Text>
              </Menu.Item>
            </>
          )}
          {state.kind === "error" && (
            <>
              <Menu.Divider />
              <Menu.Label>
                <Text size="xs" c="red.6">
                  {state.message}
                </Text>
              </Menu.Label>
            </>
          )}
        </Menu.Dropdown>
      </Menu>
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
