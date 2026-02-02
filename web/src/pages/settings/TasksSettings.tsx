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
  Table,
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

// Task row component
function TaskRow({
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
  const statusColor =
    {
      pending: "yellow",
      processing: "blue",
      completed: "green",
      failed: "red",
      cancelled: "gray",
    }[task.status] || "gray";

  return (
    <Table.Tr>
      <Table.Td>
        <Text size="sm" style={{ fontFamily: "monospace" }}>
          {task.id.slice(0, 8)}...
        </Text>
      </Table.Td>
      <Table.Td>
        <Badge variant="light">{task.task_type}</Badge>
      </Table.Td>
      <Table.Td>
        <Badge color={statusColor}>{task.status}</Badge>
      </Table.Td>
      <Table.Td>
        <Text size="sm">
          {task.attempts}/{task.max_attempts}
        </Text>
      </Table.Td>
      <Table.Td>
        <Text size="sm">{new Date(task.created_at).toLocaleString()}</Text>
      </Table.Td>
      <Table.Td>
        {task.last_error ? (
          <Tooltip label={task.last_error}>
            <Text size="sm" c="red" lineClamp={1} style={{ maxWidth: 200 }}>
              {task.last_error}
            </Text>
          </Tooltip>
        ) : (
          <Text size="sm" c="dimmed">
            -
          </Text>
        )}
      </Table.Td>
      <Table.Td>
        <Group gap="xs">
          {task.status === "pending" && (
            <Tooltip label="Cancel Task">
              <ActionIcon variant="subtle" color="red" onClick={onCancel}>
                <IconX size={16} />
              </ActionIcon>
            </Tooltip>
          )}
          {task.status === "failed" && (
            <Tooltip label="Retry Task">
              <ActionIcon variant="subtle" color="blue" onClick={onRetry}>
                <IconRefresh size={16} />
              </ActionIcon>
            </Tooltip>
          )}
          {task.locked_by && task.status === "processing" && (
            <Tooltip label="Unlock Task (Force)">
              <ActionIcon variant="subtle" color="orange" onClick={onUnlock}>
                <IconPlayerPlay size={16} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      </Table.Td>
    </Table.Tr>
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

  // Subscribe to real-time task progress
  useEffect(() => {
    const unsubscribe = subscribeToTaskProgress(
      (event) => {
        setActiveProgress((prev) => {
          const next = new Map(prev);
          if (event.status === "completed" || event.status === "failed") {
            next.delete(event.task_id);
            // Refetch tasks when a task completes
            refetchTasks();
            queryClient.invalidateQueries({ queryKey: ["task-stats"] });
          } else {
            next.set(event.task_id, event);
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
                .sort((a, b) => a.task_type.localeCompare(b.task_type))
                .map((event) => (
                  <div key={event.task_id}>
                    <Group justify="space-between" mb="xs">
                      <Group gap="xs">
                        <Badge variant="light">{event.task_type}</Badge>
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
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>ID</Table.Th>
                    <Table.Th>Type</Table.Th>
                    <Table.Th>Status</Table.Th>
                    <Table.Th>Attempts</Table.Th>
                    <Table.Th>Created</Table.Th>
                    <Table.Th>Error</Table.Th>
                    <Table.Th>Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {tasks.map((task: TaskResponse) => (
                    <TaskRow
                      key={task.id}
                      task={task}
                      onCancel={() => cancelTaskMutation.mutate(task.id)}
                      onRetry={() => retryTaskMutation.mutate(task.id)}
                      onUnlock={() => unlockTaskMutation.mutate(task.id)}
                    />
                  ))}
                </Table.Tbody>
              </Table>
            ) : (
              <Text c="dimmed" ta="center" py="xl">
                No tasks found.
              </Text>
            )}
          </Stack>
        </Card>

        {/* Task Type Breakdown */}
        {stats?.by_type && Object.keys(stats.by_type).length > 0 && (
          <Card withBorder>
            <Stack gap="md">
              <Title order={3}>By Task Type</Title>
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Type</Table.Th>
                    <Table.Th>Pending</Table.Th>
                    <Table.Th>Processing</Table.Th>
                    <Table.Th>Completed</Table.Th>
                    <Table.Th>Failed</Table.Th>
                    <Table.Th>Total</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {Object.entries(stats.by_type)
                    .sort(([typeA], [typeB]) => typeA.localeCompare(typeB))
                    .map(([type, typeStats]) => (
                      <Table.Tr key={type}>
                        <Table.Td>
                          <Badge variant="light">{type}</Badge>
                        </Table.Td>
                        <Table.Td>{typeStats.pending}</Table.Td>
                        <Table.Td>{typeStats.processing}</Table.Td>
                        <Table.Td>{typeStats.completed}</Table.Td>
                        <Table.Td>{typeStats.failed}</Table.Td>
                        <Table.Td>{typeStats.total}</Table.Td>
                      </Table.Tr>
                    ))}
                </Table.Tbody>
              </Table>
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
