import {
  ActionIcon,
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Group,
  Loader,
  Modal,
  Progress,
  Stack,
  Table,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconCloudOff,
  IconRefresh,
  IconShieldCheck,
  IconShieldOff,
  IconTrash,
} from "@tabler/icons-react";
import { formatDistanceToNow } from "date-fns";
import { useCallback, useEffect, useState } from "react";
import { MOBILE_MEDIA_QUERY } from "@/components/ui/ResponsiveTable";
import {
  broadcastDownloadsChange,
  clearDownloads,
  DOWNLOADS_BROADCAST_CHANNEL,
  type DownloadRecord,
  type DownloadsBroadcast,
  deleteDownload,
  getAllDownloads,
} from "@/lib/offline/db";
import {
  getStoragePersistence,
  requestStoragePersistence,
  type StoragePersistence,
} from "@/lib/offline/downloadManager";
import { cacheNameForBook } from "@/lib/offline/routeMatcher";

/**
 * Downloads management page.
 *
 * Lists every book currently stored in IndexedDB's `downloads` store with
 * its size, format, last-read timestamp, and a Remove action. Surfaces the
 * Storage Manager's quota estimate at the top, alongside the "Storage
 * durability" (`navigator.storage.persist()` result) so users on iOS Safari
 * have a visible signal that the browser may evict their offline copies.
 * Subscribes to the `codex:downloads` BroadcastChannel so the list updates
 * live while downloads run in this or any other tab.
 *
 * This is device-local data, so the page renders for every authenticated
 * user, not just admins.
 */

interface QuotaEstimate {
  usage: number | null;
  quota: number | null;
}

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes === undefined) return "-";
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
}

function formatLastRead(record: DownloadRecord): string {
  const ts = record.lastReadAt ?? record.downloadedAt;
  if (!ts) return "never";
  try {
    return formatDistanceToNow(new Date(ts), { addSuffix: true });
  } catch {
    return "unknown";
  }
}

function formatLabel(record: DownloadRecord): string {
  switch (record.status) {
    case "complete":
      return "Saved";
    case "downloading":
      return "Downloading";
    case "queued":
      return "Queued";
    case "error":
      return "Error";
  }
}

function statusColor(status: DownloadRecord["status"]): string {
  switch (status) {
    case "complete":
      return "green";
    case "downloading":
      return "blue";
    case "queued":
      return "gray";
    case "error":
      return "red";
  }
}

export function DownloadsSettings() {
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY);
  const [records, setRecords] = useState<DownloadRecord[] | null>(null);
  const [quota, setQuota] = useState<QuotaEstimate>({
    usage: null,
    quota: null,
  });
  const [persistence, setPersistence] = useState<StoragePersistence>(
    getStoragePersistence(),
  );
  const [clearOpen, { open: openClear, close: closeClear }] =
    useDisclosure(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  const refreshRecords = useCallback(async () => {
    try {
      const all = await getAllDownloads();
      all.sort((a, b) => (b.downloadedAt ?? 0) - (a.downloadedAt ?? 0));
      setRecords(all);
    } catch {
      setRecords([]);
    }
  }, []);

  const refreshQuota = useCallback(async () => {
    if (
      typeof navigator === "undefined" ||
      !navigator.storage ||
      typeof navigator.storage.estimate !== "function"
    ) {
      setQuota({ usage: null, quota: null });
      return;
    }
    try {
      const estimate = await navigator.storage.estimate();
      setQuota({
        usage: typeof estimate.usage === "number" ? estimate.usage : null,
        quota: typeof estimate.quota === "number" ? estimate.quota : null,
      });
    } catch {
      setQuota({ usage: null, quota: null });
    }
  }, []);

  const refreshPersistence = useCallback(async () => {
    // Opportunistically request persistence when the user lands here so the
    // indicator can flip to "granted" without waiting on a download.
    const result = await requestStoragePersistence();
    setPersistence(result);
  }, []);

  useEffect(() => {
    void refreshRecords();
    void refreshQuota();
    void refreshPersistence();

    let channel: BroadcastChannel | null = null;
    if (typeof BroadcastChannel !== "undefined") {
      channel = new BroadcastChannel(DOWNLOADS_BROADCAST_CHANNEL);
      channel.addEventListener("message", handleBroadcast);
    }

    function handleBroadcast(_ev: MessageEvent<DownloadsBroadcast>) {
      // Refresh both lists on any broadcast; the channel volume is low
      // (one message per IDB write) and refreshing the page-local view
      // from IDB ordering is simpler than maintaining a delta in memory.
      void refreshRecords();
      void refreshQuota();
    }

    return () => {
      if (channel) {
        channel.removeEventListener("message", handleBroadcast);
        channel.close();
      }
    };
  }, [refreshRecords, refreshQuota, refreshPersistence]);

  const handleRemove = useCallback(
    async (id: string) => {
      setBusyId(id);
      try {
        await deleteDownload(id);
        broadcastDownloadsChange({ kind: "delete", id });
        if (typeof caches !== "undefined") {
          await caches.delete(cacheNameForBook(id));
        }
        await refreshRecords();
        await refreshQuota();
      } catch (err) {
        notifications.show({
          color: "red",
          title: "Could not remove offline copy",
          message: err instanceof Error ? err.message : String(err),
        });
      } finally {
        setBusyId(null);
      }
    },
    [refreshRecords, refreshQuota],
  );

  const handleClearAll = useCallback(async () => {
    closeClear();
    try {
      const all = await getAllDownloads();
      await clearDownloads();
      broadcastDownloadsChange({ kind: "clear" });
      if (typeof caches !== "undefined") {
        await Promise.all(
          all.map((r) => caches.delete(cacheNameForBook(r.id))),
        );
      }
      await refreshRecords();
      await refreshQuota();
      notifications.show({
        color: "green",
        title: "Offline downloads cleared",
        message: `Removed ${all.length} downloaded book${all.length === 1 ? "" : "s"}.`,
      });
    } catch (err) {
      notifications.show({
        color: "red",
        title: "Could not clear offline downloads",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [closeClear, refreshRecords, refreshQuota]);

  const usagePct =
    quota.quota && quota.usage !== null
      ? Math.min(100, Math.round((quota.usage / quota.quota) * 100))
      : null;

  const totalBytes = (records ?? []).reduce(
    (acc, r) => acc + (r.status === "complete" ? r.bytes : 0),
    0,
  );

  return (
    <Box py="md" px="md">
      <Stack gap="lg">
        <Stack gap={4}>
          <Title order={2}>Offline downloads</Title>
          <Text c="dimmed" size="sm">
            Books you have saved to read without a network connection. These are
            stored in this browser only; opening Codex in another browser or on
            another device will not see this list.
          </Text>
        </Stack>

        <Card withBorder padding="md">
          <Stack gap="sm">
            <Group justify="space-between" wrap="nowrap">
              <Stack gap={2}>
                <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
                  Storage used
                </Text>
                <Group gap={6} align="baseline">
                  <Text size="lg" fw={700}>
                    {formatBytes(quota.usage)}
                  </Text>
                  {quota.quota !== null && (
                    <Text size="sm" c="dimmed">
                      / {formatBytes(quota.quota)} available
                    </Text>
                  )}
                </Group>
              </Stack>
              <Tooltip label="Refresh quota estimate">
                <ActionIcon
                  variant="subtle"
                  size="lg"
                  onClick={() => void refreshQuota()}
                  aria-label="Refresh quota estimate"
                >
                  <IconRefresh size={18} />
                </ActionIcon>
              </Tooltip>
            </Group>
            {usagePct !== null && (
              <Progress
                value={usagePct}
                size="md"
                color={usagePct > 80 ? "orange" : "blue"}
                aria-label="Storage usage"
              />
            )}
            <PersistenceIndicator value={persistence} />
          </Stack>
        </Card>

        {records === null && (
          <Group justify="center" py="xl">
            <Loader size="sm" />
          </Group>
        )}

        {records !== null && records.length === 0 && (
          <Alert
            icon={<IconCloudOff size={18} />}
            color="gray"
            variant="light"
            title="No offline downloads yet"
          >
            Tap the cloud-down icon on a book to save it for offline reading.
            EPUB, PDF, CBZ, and CBR are all supported.
          </Alert>
        )}

        {records !== null && records.length > 0 && (
          <>
            <Group justify="space-between" align="center">
              <Text size="sm" c="dimmed">
                {records.length} book{records.length === 1 ? "" : "s"} saved,{" "}
                {formatBytes(totalBytes)} total.
              </Text>
              <Button
                size="xs"
                color="red"
                variant="outline"
                leftSection={<IconTrash size={14} />}
                onClick={openClear}
              >
                Clear all downloads
              </Button>
            </Group>
            {isMobile ? (
              <MobileRecordList
                records={records}
                busyId={busyId}
                onRemove={handleRemove}
              />
            ) : (
              <DesktopRecordTable
                records={records}
                busyId={busyId}
                onRemove={handleRemove}
              />
            )}
          </>
        )}
      </Stack>

      <Modal
        opened={clearOpen}
        onClose={closeClear}
        title="Clear all offline downloads?"
        centered
      >
        <Stack gap="md">
          <Text size="sm">
            This will remove every book you have saved offline on this device.
            Books on the server are not affected.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={closeClear}>
              Cancel
            </Button>
            <Button color="red" onClick={handleClearAll}>
              Remove all
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}

function PersistenceIndicator({ value }: { value: StoragePersistence }) {
  if (value === true) {
    return (
      <Group gap={6} c="green.6">
        <IconShieldCheck size={16} />
        <Text size="sm">
          Storage is persistent: the browser will not evict your offline
          downloads under ordinary storage pressure.
        </Text>
      </Group>
    );
  }
  if (value === false) {
    return (
      <Group gap={6} c="orange.6" align="flex-start" wrap="nowrap">
        <IconShieldOff size={16} style={{ flexShrink: 0, marginTop: 2 }} />
        <Text size="sm">
          Storage is not marked persistent. Some browsers (notably iOS Safari in
          a tab) may clear your downloads after a period of inactivity.
          Installing Codex to your home screen can improve durability.
        </Text>
      </Group>
    );
  }
  return (
    <Group gap={6} c="dimmed" align="flex-start" wrap="nowrap">
      <IconAlertCircle size={16} style={{ flexShrink: 0, marginTop: 2 }} />
      <Text size="sm">
        Storage durability is unknown in this browser. Downloads may or may not
        survive a long period of inactivity.
      </Text>
    </Group>
  );
}

interface RowProps {
  records: DownloadRecord[];
  busyId: string | null;
  onRemove: (id: string) => void;
}

function DesktopRecordTable({ records, busyId, onRemove }: RowProps) {
  return (
    <Table striped highlightOnHover withTableBorder>
      <Table.Thead>
        <Table.Tr>
          <Table.Th>Book id</Table.Th>
          <Table.Th>Format</Table.Th>
          <Table.Th>Status</Table.Th>
          <Table.Th>Size</Table.Th>
          <Table.Th>Saved</Table.Th>
          <Table.Th aria-label="Actions" />
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {records.map((r) => (
          <Table.Tr key={r.id}>
            <Table.Td>
              <Text size="sm" ff="monospace" lineClamp={1}>
                {r.id}
              </Text>
            </Table.Td>
            <Table.Td>
              <Badge size="sm" variant="light">
                {r.format.toUpperCase()}
              </Badge>
            </Table.Td>
            <Table.Td>
              <Badge size="sm" variant="filled" color={statusColor(r.status)}>
                {formatLabel(r)}
              </Badge>
            </Table.Td>
            <Table.Td>
              <Text size="sm">{formatBytes(r.bytes)}</Text>
            </Table.Td>
            <Table.Td>
              <Text size="sm" c="dimmed">
                {formatLastRead(r)}
              </Text>
            </Table.Td>
            <Table.Td>
              <Tooltip label="Remove offline copy">
                <ActionIcon
                  variant="subtle"
                  color="red"
                  size="sm"
                  loading={busyId === r.id}
                  onClick={() => onRemove(r.id)}
                  aria-label={`Remove offline copy of ${r.id}`}
                >
                  <IconTrash size={14} />
                </ActionIcon>
              </Tooltip>
            </Table.Td>
          </Table.Tr>
        ))}
      </Table.Tbody>
    </Table>
  );
}

function MobileRecordList({ records, busyId, onRemove }: RowProps) {
  return (
    <Stack gap="xs">
      {records.map((r) => (
        <Card key={r.id} withBorder padding="sm">
          <Stack gap={4}>
            <Group justify="space-between" wrap="nowrap" align="flex-start">
              <Stack gap={2} style={{ minWidth: 0, flex: 1 }}>
                <Text
                  size="sm"
                  ff="monospace"
                  style={{ overflowWrap: "anywhere" }}
                >
                  {r.id}
                </Text>
                <Group gap={6}>
                  <Badge size="xs" variant="light">
                    {r.format.toUpperCase()}
                  </Badge>
                  <Badge
                    size="xs"
                    variant="filled"
                    color={statusColor(r.status)}
                  >
                    {formatLabel(r)}
                  </Badge>
                </Group>
              </Stack>
              <ActionIcon
                variant="subtle"
                color="red"
                size="lg"
                loading={busyId === r.id}
                onClick={() => onRemove(r.id)}
                aria-label={`Remove offline copy of ${r.id}`}
              >
                <IconTrash size={16} />
              </ActionIcon>
            </Group>
            <Group justify="space-between">
              <Text size="xs" c="dimmed">
                {formatBytes(r.bytes)}
              </Text>
              <Text size="xs" c="dimmed">
                {formatLastRead(r)}
              </Text>
            </Group>
          </Stack>
        </Card>
      ))}
    </Stack>
  );
}
