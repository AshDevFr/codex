import { Badge, Group, Stack, Text, Tooltip } from "@mantine/core";
import { IconLoader2 } from "@tabler/icons-react";
import { useNavigate } from "react-router-dom";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import { useAuthStore } from "@/store/authStore";

/**
 * Task notification badge that appears at the bottom of the navigation sidebar
 * Shows count of active tasks with a tooltip containing task details
 */
export function TaskNotificationBadge() {
  const navigate = useNavigate();
  const { user } = useAuthStore();
  const { activeTasks, pendingCounts } = useTaskProgress();
  const isAdmin = user?.role === "admin";

  // Filter to only running tasks (processing tasks are shown as running)
  // Note: pending tasks are shown separately via pendingCounts, not from activeTasks
  const runningTasks = activeTasks.filter((task) => task.status === "running");

  // Calculate total pending count
  const totalPendingCount = Object.values(pendingCounts).reduce(
    (sum, count) => sum + count,
    0,
  );

  // If no running tasks and no pending tasks, don't show the badge
  if (runningTasks.length === 0 && totalPendingCount === 0) {
    return null;
  }

  const formatTaskType = (type: string) => {
    return type
      .replace(/([A-Z])/g, " $1")
      .replace(/_/g, " ")
      .trim()
      .split(" ")
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  };

  const getTaskSummary = (task: (typeof runningTasks)[0]) => {
    const taskName = formatTaskType(task.task_type);
    const progress = task.progress
      ? ` (${task.progress.current}/${task.progress.total})`
      : "";
    return `${taskName}${progress}`;
  };

  // Sort running tasks by formatted task type name
  const sortedRunningTasks = [...runningTasks].sort((a, b) => {
    const nameA = formatTaskType(a.task_type);
    const nameB = formatTaskType(b.task_type);
    return nameA.localeCompare(nameB);
  });

  // Filter and sort pending task entries by formatted name, excluding entries with 0 count
  const sortedPendingEntries = Object.entries(pendingCounts)
    .filter(([, count]) => count > 0)
    .sort(([typeA], [typeB]) => {
      const nameA = formatTaskType(typeA);
      const nameB = formatTaskType(typeB);
      return nameA.localeCompare(nameB);
    });

  const tooltipContent = (
    <Stack gap={8}>
      {sortedRunningTasks.length > 0 && (
        <Stack gap={4}>
          <Text size="xs" fw={600}>
            Running Tasks
          </Text>
          {sortedRunningTasks.map((task) => (
            <Group key={task.task_id} gap="xs">
              <IconLoader2
                size={12}
                style={{ color: "var(--mantine-color-blue-4)" }}
                className="rotating-icon-small"
              />
              <Text size="xs">{getTaskSummary(task)}</Text>
            </Group>
          ))}
        </Stack>
      )}

      {sortedPendingEntries.length > 0 && (
        <Stack gap={4}>
          <Text size="xs" fw={600}>
            Pending Tasks ({totalPendingCount})
          </Text>
          {sortedPendingEntries.map(([taskType, count]) => (
            <Group key={taskType} gap="xs">
              <Text size="xs" c="dimmed">
                {formatTaskType(taskType)}: {count}
              </Text>
            </Group>
          ))}
        </Stack>
      )}

      <style>
        {`
					@keyframes rotate-small {
						from { transform: rotate(0deg); }
						to { transform: rotate(360deg); }
					}
					.rotating-icon-small {
						animation: rotate-small 1s linear infinite;
					}
				`}
      </style>
    </Stack>
  );

  const totalTasks = runningTasks.length + totalPendingCount;

  return (
    <Tooltip label={tooltipContent} withArrow position="top-start">
      <Badge
        color="blue"
        variant="filled"
        size="sm"
        style={{
          cursor: isAdmin ? "pointer" : "default",
          animation: "pulse 2s ease-in-out infinite",
        }}
        onClick={isAdmin ? () => navigate("/settings/tasks") : undefined}
        fullWidth
      >
        {totalTasks} pending task
        {totalTasks !== 1 ? "s" : ""}
      </Badge>
    </Tooltip>
  );
}
