import {
  ActionIcon,
  Badge,
  Card,
  Center,
  Group,
  Loader,
  Menu,
  Stack,
  Switch,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconDotsVertical,
  IconEdit,
  IconPlayerPlay,
  IconTrash,
} from "@tabler/icons-react";
import type { LibraryJob } from "@/api/libraryJobs";
import {
  useRunLibraryJobNow,
  useUpdateLibraryJob,
} from "@/hooks/useLibraryJobs";

interface JobListProps {
  libraryId: string;
  jobs: LibraryJob[];
  isLoading: boolean;
  onEdit: (job: LibraryJob) => void;
  onDelete: (job: LibraryJob) => void;
}

export function JobList({
  libraryId,
  jobs,
  isLoading,
  onEdit,
  onDelete,
}: JobListProps) {
  if (isLoading) {
    return (
      <Center py="xl">
        <Loader />
      </Center>
    );
  }

  if (jobs.length === 0) {
    return (
      <Card withBorder>
        <Stack align="center" py="xl" gap="xs">
          <Text fw={500}>No scheduled jobs</Text>
          <Text c="dimmed" size="sm" ta="center">
            Add a job to refresh series metadata on a recurring schedule.
            <br />
            Each job targets one provider with the field groups you choose.
          </Text>
        </Stack>
      </Card>
    );
  }

  return (
    <Stack>
      {jobs.map((job) => (
        <JobRow
          key={job.id}
          libraryId={libraryId}
          job={job}
          onEdit={onEdit}
          onDelete={onDelete}
        />
      ))}
    </Stack>
  );
}

function JobRow({
  libraryId,
  job,
  onEdit,
  onDelete,
}: {
  libraryId: string;
  job: LibraryJob;
  onEdit: (job: LibraryJob) => void;
  onDelete: (job: LibraryJob) => void;
}) {
  const update = useUpdateLibraryJob(libraryId);
  const runNow = useRunLibraryJobNow(libraryId);

  const provider = job.config.provider.replace(/^plugin:/, "");
  const groups = job.config.fieldGroups ?? [];
  const lastRun = formatLastRun(job);

  return (
    <Card withBorder padding="md">
      <Group justify="space-between" align="flex-start" wrap="nowrap">
        <Stack gap={4} style={{ flex: 1, minWidth: 0 }}>
          <Group gap="xs" wrap="nowrap">
            <Text fw={500} truncate>
              {job.name}
            </Text>
            <Badge size="xs" variant="light">
              metadata refresh
            </Badge>
            {!job.enabled && (
              <Badge size="xs" variant="default" color="gray">
                disabled
              </Badge>
            )}
          </Group>
          <Text size="sm" c="dimmed">
            <strong>{provider}</strong> ·{" "}
            {groups.length === 0 ? "all fields" : groups.join(", ")} · cron{" "}
            <code>{job.cronSchedule}</code>
            {job.timezone ? ` (${job.timezone})` : ""}
          </Text>
          <Text size="xs" c="dimmed">
            {lastRun}
          </Text>
        </Stack>

        <Group gap="xs" wrap="nowrap">
          <Tooltip label={job.enabled ? "Disable" : "Enable"}>
            <Switch
              checked={job.enabled}
              onChange={(event) =>
                update.mutate({
                  jobId: job.id,
                  patch: { enabled: event.currentTarget.checked },
                })
              }
              disabled={update.isPending}
              aria-label="Enable job"
            />
          </Tooltip>
          <Tooltip label="Run now">
            <ActionIcon
              variant="subtle"
              onClick={() => runNow.mutate(job.id)}
              loading={runNow.isPending}
            >
              <IconPlayerPlay size={16} />
            </ActionIcon>
          </Tooltip>
          <Menu position="bottom-end">
            <Menu.Target>
              <ActionIcon variant="subtle">
                <IconDotsVertical size={16} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item
                leftSection={<IconEdit size={14} />}
                onClick={() => onEdit(job)}
              >
                Edit
              </Menu.Item>
              <Menu.Divider />
              <Menu.Item
                color="red"
                leftSection={<IconTrash size={14} />}
                onClick={() => onDelete(job)}
              >
                Delete
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
        </Group>
      </Group>
    </Card>
  );
}

function formatLastRun(job: LibraryJob): string {
  if (!job.lastRunAt) return "Never run";
  const when = new Date(job.lastRunAt).toLocaleString();
  const status = job.lastRunStatus ?? "unknown";
  const tail = job.lastRunMessage ? ` — ${job.lastRunMessage}` : "";
  return `Last run: ${when} (${status})${tail}`;
}
