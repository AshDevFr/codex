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
  Select,
  SimpleGrid,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconCheck,
  IconClock,
  IconPlayerPlay,
  IconRefresh,
  IconX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "@/api/client";
import {
  fetchTaskStats,
  fetchTasksByStatus,
  subscribeToTaskProgress,
} from "@/api/tasks";
import { ResponsiveTable } from "@/components/ui";
import type { TaskProgressEvent, TaskResponse } from "@/types";

// Stat card component
function StatCard({
  title,
  value,
  color,
  icon,
}: {
  title: string;
  value: number;
  color: string;
  icon: React.ReactNode;
}) {
  return (
    <Card withBorder padding="md">
      <Group justify="space-between">
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
            {title}
          </Text>
          <Text size="xl" fw={700}>
            {value.toLocaleString()}
          </Text>
        </div>
        <Box c={color}>{icon}</Box>
      </Group>
    </Card>
  );
}

function getTaskStatusColor(status: string): string {
  return (
    {
      pending: "yellow",
      processing: "blue",
      completed: "green",
      failed: "red",
      cancelled: "gray",
    }[status] || "gray"
  );
}

function TaskActions({
  task,
  onCancel,
  onRetry,
  onUnlock,
}: {
  task: TaskResponse;
  onCancel: () => void;
  onRetry: () => void;
  onUnlock: () => void;
}) {
  return (
    <>
      {task.status === "pending" && (
        <Tooltip label="Cancel Task">
          <ActionIcon
            variant="subtle"
            color="red"
            onClick={onCancel}
            aria-label="Cancel task"
          >
            <IconX size={16} />
          </ActionIcon>
        </Tooltip>
      )}
      {task.status === "failed" && (
        <Tooltip label="Retry Task">
          <ActionIcon
            variant="subtle"
            color="blue"
            onClick={onRetry}
            aria-label="Retry task"
          >
            <IconRefresh size={16} />
          </ActionIcon>
        </Tooltip>
      )}
      {task.lockedBy && task.status === "processing" && (
        <Tooltip label="Unlock Task (Force)">
          <ActionIcon
            variant="subtle"
            color="orange"
            onClick={onUnlock}
            aria-label="Unlock task"
          >
            <IconPlayerPlay size={16} />
          </ActionIcon>
        </Tooltip>
      )}
    </>
  );
}

export function TasksSettings() {
  const queryClient = useQueryClient();
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [purgeModalOpened, setPurgeModalOpened] = useState(false);
  const [nukeModalOpened, setNukeModalOpened] = useState(false);
  const [activeProgress, setActiveProgress] = useState<
    Map<string, TaskProgressEvent>
  >(new Map());

  // Fetch task stats
  const { data: stats, isLoading: statsLoading } = useQuery({
    queryKey: ["task-stats"],
    queryFn: fetchTaskStats,
    refetchInterval: 5000, // Refresh every 5 seconds
  });

  // Fetch tasks by status
  const {
    data: tasks,
    isLoading: tasksLoading,
    refetch: refetchTasks,
  } = useQuery({
    queryKey: ["tasks", statusFilter],
    queryFn: () =>
      statusFilter === "all"
        ? Promise.all([
            fetchTasksByStatus("pending"),
            fetchTasksByStatus("processing"),
            fetchTasksByStatus("completed", 20),
            fetchTasksByStatus("failed", 20),
          ]).then((results) => results.flat())
        : fetchTasksByStatus(statusFilter),
    refetchInterval: 5000,
  });

  // Seed `activeProgress` from currently-processing tasks so the "Active
  // Tasks" panel reflects them immediately on page load, even before any
  // SSE progress event arrives. Without this, opening the page mid-poll
  // shows an empty panel until the running task fires its next progress
  // emit (which can be many seconds for slow polls or never for handlers
  // that don't emit progress at all).
  //
  // Existing entries (already populated by SSE) win — we never overwrite
  // a richer, more recent event with a bare polling snapshot.
  useEffect(() => {
    if (!tasks || tasks.length === 0) return;
    setActiveProgress((prev) => {
      let changed = false;
      const next = new Map(prev);
      const processingIds = new Set<string>();
      for (const t of tasks) {
        if (t.status !== "processing") continue;
        processingIds.add(t.id);
        if (next.has(t.id)) continue;
        next.set(t.id, {
          taskId: t.id,
          taskType: t.taskType,
          status: "running",
          progress: undefined,
          error: undefined,
          startedAt: t.startedAt ?? new Date().toISOString(),
          completedAt: undefined,
          libraryId: t.libraryId ?? undefined,
          seriesId: t.seriesId ?? undefined,
          bookId: t.bookId ?? undefined,
        });
        changed = true;
      }
      // Drop running entries that are no longer in the processing list —
      // they completed/failed without an SSE delete reaching us (e.g. SSE
      // dropped the event, or the page just opened post-completion).
      for (const [id, ev] of prev) {
        if (ev.status === "running" && !processingIds.has(id)) {
          next.delete(id);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [tasks]);

  // Subscribe to real-time task progress
  useEffect(() => {
    const unsubscribe = subscribeToTaskProgress(
      (event) => {
        setActiveProgress((prev) => {
          const next = new Map(prev);
          if (event.status === "completed" || event.status === "failed") {
            next.delete(event.taskId);
            // Refetch tasks when a task completes
            refetchTasks();
            queryClient.invalidateQueries({ queryKey: ["task-stats"] });
          } else {
            next.set(event.taskId, event);
          }
          return next;
        });
      },
      (error) => {
        console.error("Task progress error:", error);
      },
    );

    return () => unsubscribe();
  }, [refetchTasks, queryClient]);

  // Mutations
  const cancelTaskMutation = useMutation({
    mutationFn: async (taskId: string) => {
      await api.post(`/tasks/${taskId}/cancel`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
      notifications.show({
        title: "Success",
        message: "Task cancelled",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to cancel task",
        color: "red",
      });
    },
  });

  const retryTaskMutation = useMutation({
    mutationFn: async (taskId: string) => {
      await api.post(`/tasks/${taskId}/retry`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
      notifications.show({
        title: "Success",
        message: "Task queued for retry",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to retry task",
        color: "red",
      });
    },
  });

  const unlockTaskMutation = useMutation({
    mutationFn: async (taskId: string) => {
      await api.post(`/tasks/${taskId}/unlock`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      notifications.show({
        title: "Success",
        message: "Task unlocked",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to unlock task",
        color: "red",
      });
    },
  });

  const purgeTasksMutation = useMutation({
    mutationFn: async () => {
      await api.delete("/tasks/purge");
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
      setPurgeModalOpened(false);
      notifications.show({
        title: "Success",
        message: "Old completed tasks purged",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to purge tasks",
        color: "red",
      });
    },
  });

  const nukeTasksMutation = useMutation({
    mutationFn: async () => {
      await api.delete("/tasks/nuke");
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tasks"] });
      queryClient.invalidateQueries({ queryKey: ["task-stats"] });
      setNukeModalOpened(false);
      notifications.show({
        title: "Success",
        message: "All tasks deleted",
        color: "green",
      });
    },
    onError: () => {
      notifications.show({
        title: "Error",
        message: "Failed to delete tasks",
        color: "red",
      });
    },
  });

  const totalTasks = stats?.total || 0;
  // Reserved for future use
  void totalTasks;

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        <Group justify="space-between">
          <Title order={1}>Task Queue</Title>
          <Group gap="xs">
            <Button
              variant="light"
              leftSection={<IconRefresh size={16} />}
              onClick={() => {
                queryClient.invalidateQueries({ queryKey: ["tasks"] });
                queryClient.invalidateQueries({ queryKey: ["task-stats"] });
              }}
            >
              Refresh
            </Button>
            <Button
              variant="light"
              color="orange"
              onClick={() => setPurgeModalOpened(true)}
            >
              Purge Old Tasks
            </Button>
            <Button
              variant="light"
              color="red"
              onClick={() => setNukeModalOpened(true)}
            >
              Delete All
            </Button>
          </Group>
        </Group>

        {/* Stats Overview */}
        {statsLoading ? (
          <Group justify="center">
            <Loader />
          </Group>
        ) : stats ? (
          <SimpleGrid cols={{ base: 2, sm: 3, md: 5 }}>
            <StatCard
              title="Pending"
              value={stats.pending}
              color="yellow"
              icon={<IconClock size={24} />}
            />
            <StatCard
              title="Processing"
              value={stats.processing}
              color="blue"
              icon={<IconPlayerPlay size={24} />}
            />
            <StatCard
              title="Completed"
              value={stats.completed}
              color="green"
              icon={<IconCheck size={24} />}
            />
            <StatCard
              title="Failed"
              value={stats.failed}
              color="red"
              icon={<IconX size={24} />}
            />
            <StatCard
              title="Total"
              value={stats.total}
              color="gray"
              icon={<IconAlertCircle size={24} />}
            />
          </SimpleGrid>
        ) : null}

        {/* Active Progress */}
        {activeProgress.size > 0 && (
          <Card withBorder>
            <Stack gap="md">
              <Title order={3}>Active Tasks</Title>
              {Array.from(activeProgress.values())
                .sort((a, b) => a.taskType.localeCompare(b.taskType))
                .map((event) => (
                  <div key={event.taskId}>
                    <Group justify="space-between" mb="xs">
                      <Group gap="xs">
                        <Badge variant="light">{event.taskType}</Badge>
                        <Text size="sm">
                          {event.progress?.message || "Processing..."}
                        </Text>
                      </Group>
                      {event.progress?.current !== undefined &&
                        event.progress?.total !== undefined && (
                          <Text size="sm" c="dimmed">
                            {event.progress.current} / {event.progress.total}
                          </Text>
                        )}
                    </Group>
                    {event.progress?.current !== undefined &&
                      event.progress?.total !== undefined &&
                      event.progress.total > 0 && (
                        <Progress
                          value={
                            (event.progress.current / event.progress.total) *
                            100
                          }
                          size="sm"
                          animated
                        />
                      )}
                  </div>
                ))}
            </Stack>
          </Card>
        )}

        {/* Task List */}
        <Card withBorder>
          <Stack gap="md">
            <Group justify="space-between">
              <Title order={3}>Tasks</Title>
              <Select
                value={statusFilter}
                onChange={(value) => setStatusFilter(value || "all")}
                data={[
                  { label: "All", value: "all" },
                  { label: "Pending", value: "pending" },
                  { label: "Processing", value: "processing" },
                  { label: "Completed", value: "completed" },
                  { label: "Failed", value: "failed" },
                ]}
                w={150}
              />
            </Group>

            {tasksLoading ? (
              <Group justify="center" py="xl">
                <Loader />
              </Group>
            ) : tasks && tasks.length > 0 ? (
              <ResponsiveTable<TaskResponse>
                data={tasks}
                columns={[
                  {
                    key: "id",
                    header: "ID",
                    accessor: (task) => (
                      <Text size="sm" style={{ fontFamily: "monospace" }}>
                        {task.id.slice(0, 8)}...
                      </Text>
                    ),
                  },
                  {
                    key: "type",
                    header: "Type",
                    mobilePrimary: true,
                    accessor: (task) => (
                      <Badge variant="light">{task.taskType}</Badge>
                    ),
                  },
                  {
                    key: "status",
                    header: "Status",
                    accessor: (task) => (
                      <Badge color={getTaskStatusColor(task.status)}>
                        {task.status}
                      </Badge>
                    ),
                  },
                  {
                    key: "attempts",
                    header: "Attempts",
                    accessor: (task) => (
                      <Text size="sm">
                        {task.attempts}/{task.maxAttempts}
                      </Text>
                    ),
                  },
                  {
                    key: "created",
                    header: "Created",
                    accessor: (task) => (
                      <Text size="sm">
                        {new Date(task.createdAt).toLocaleString()}
                      </Text>
                    ),
                  },
                  {
                    key: "error",
                    header: "Error",
                    mobileFullWidth: true,
                    accessor: (task) =>
                      task.lastError ? (
                        <Tooltip label={task.lastError}>
                          <Text
                            size="sm"
                            c="red"
                            lineClamp={1}
                            style={{ maxWidth: 200 }}
                          >
                            {task.lastError}
                          </Text>
                        </Tooltip>
                      ) : (
                        <Text size="sm" c="dimmed">
                          -
                        </Text>
                      ),
                  },
                ]}
                getRowKey={(task) => task.id}
                rowActions={(task) => (
                  <TaskActions
                    task={task}
                    onCancel={() => cancelTaskMutation.mutate(task.id)}
                    onRetry={() => retryTaskMutation.mutate(task.id)}
                    onUnlock={() => unlockTaskMutation.mutate(task.id)}
                  />
                )}
              />
            ) : (
              <Text c="dimmed" ta="center" py="xl">
                No tasks found.
              </Text>
            )}
          </Stack>
        </Card>

        {/* Task Type Breakdown */}
        {stats?.byType && Object.keys(stats.byType).length > 0 && (
          <Card withBorder>
            <Stack gap="md">
              <Title order={3}>By Task Type</Title>
              <ResponsiveTable<{
                type: string;
                pending: number;
                processing: number;
                completed: number;
                failed: number;
                total: number;
              }>
                data={Object.entries(stats.byType)
                  .sort(([typeA], [typeB]) => typeA.localeCompare(typeB))
                  .map(([type, typeStats]) => ({
                    type,
                    pending: typeStats.pending,
                    processing: typeStats.processing,
                    completed: typeStats.completed,
                    failed: typeStats.failed,
                    total: typeStats.total,
                  }))}
                columns={[
                  {
                    key: "type",
                    header: "Type",
                    mobilePrimary: true,
                    accessor: (row) => (
                      <Badge variant="light">{row.type}</Badge>
                    ),
                  },
                  {
                    key: "pending",
                    header: "Pending",
                    accessor: (row) => row.pending,
                  },
                  {
                    key: "processing",
                    header: "Processing",
                    accessor: (row) => row.processing,
                  },
                  {
                    key: "completed",
                    header: "Completed",
                    accessor: (row) => row.completed,
                  },
                  {
                    key: "failed",
                    header: "Failed",
                    accessor: (row) => row.failed,
                  },
                  {
                    key: "total",
                    header: "Total",
                    accessor: (row) => row.total,
                  },
                ]}
                getRowKey={(row) => row.type}
              />
            </Stack>
          </Card>
        )}
      </Stack>

      {/* Purge Modal */}
      <Modal
        opened={purgeModalOpened}
        onClose={() => setPurgeModalOpened(false)}
        title="Purge Old Tasks"
      >
        <Stack gap="md">
          <Text>
            This will delete all completed tasks older than 7 days and all
            failed tasks older than 30 days.
          </Text>
          <Text size="sm" c="dimmed">
            This action cannot be undone.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setPurgeModalOpened(false)}>
              Cancel
            </Button>
            <Button
              color="orange"
              loading={purgeTasksMutation.isPending}
              onClick={() => purgeTasksMutation.mutate()}
            >
              Purge Tasks
            </Button>
          </Group>
        </Stack>
      </Modal>

      {/* Nuke Modal */}
      <Modal
        opened={nukeModalOpened}
        onClose={() => setNukeModalOpened(false)}
        title="Delete All Tasks"
      >
        <Stack gap="md">
          <Alert icon={<IconAlertCircle size={16} />} color="red">
            This will delete ALL tasks including pending and processing tasks!
          </Alert>
          <Text>
            Are you sure you want to delete all tasks? This action cannot be
            undone.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setNukeModalOpened(false)}>
              Cancel
            </Button>
            <Button
              color="red"
              loading={nukeTasksMutation.isPending}
              onClick={() => nukeTasksMutation.mutate()}
            >
              Delete All Tasks
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
