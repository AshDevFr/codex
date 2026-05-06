import { Badge, Group, Stack, Text, Tooltip } from "@mantine/core";
import { IconLoader2 } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { usePermissions } from "@/hooks/usePermissions";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { ActiveTask } from "@/types";
import { PERMISSIONS } from "@/types/permissions";
import { elapsedSince, formatElapsed } from "@/utils/duration";
import { getTaskTarget } from "@/utils/tasks";

/**
 * Task notification badge that appears at the bottom of the navigation sidebar
 * Shows count of active tasks with a tooltip containing task details
 * Only visible to users with TASKS_READ permission
 */
export function TaskNotificationBadge() {
  const navigate = useNavigate();
  const { isAdmin, hasPermission } = usePermissions();
  const canReadTasks = hasPermission(PERMISSIONS.TASKS_READ);
  const { activeTasks, pendingCounts } = useTaskProgress();

  // Filter to only running tasks (processing tasks are shown as running).
  // pending tasks are shown separately via pendingCounts, not from activeTasks.
  const runningTasks = activeTasks.filter((task) => task.status === "running");
  const hasRunning = runningTasks.length > 0;

  // Tick once per second only while running tasks exist, so elapsed times
  // refresh without burning CPU when the panel is otherwise idle.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!hasRunning) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [hasRunning]);

  if (!canReadTasks) {
    return null;
  }

  const totalPendingCount = Object.values(pendingCounts).reduce(
    (sum, count) => sum + count,
    0,
  );

  if (!hasRunning && totalPendingCount === 0) {
    return null;
  }

  const formatTaskType = (type: string) =>
    type
      .replace(/([A-Z])/g, " $1")
      .replace(/_/g, " ")
      .trim()
      .split(" ")
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");

  const renderRunningTask = (task: ActiveTask) => {
    const taskName = formatTaskType(task.taskType);
    const target = getTaskTarget(task);
    const elapsed = formatElapsed(elapsedSince(task.startedAt, now));
    const progressSuffix = task.progress
      ? ` (${task.progress.current}/${task.progress.total})`
      : "";

    return (
      <Group key={task.taskId} gap="xs" wrap="nowrap" align="center">
        <IconLoader2
          size={12}
          style={{ color: "var(--mantine-color-blue-4)", flexShrink: 0 }}
          className="rotating-icon-small"
        />
        <Text size="xs" style={{ flexShrink: 0 }}>
          {taskName}
          {progressSuffix}
        </Text>
        {target ? (
          <Text
            size="xs"
            c="dimmed"
            title={target}
            style={{
              maxWidth: 180,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            · {target}
          </Text>
        ) : null}
        <Text size="xs" c="dimmed" style={{ flexShrink: 0 }}>
          · {elapsed}
        </Text>
      </Group>
    );
  };

  const sortedRunningTasks = [...runningTasks].sort((a, b) => {
    const nameA = formatTaskType(a.taskType);
    const nameB = formatTaskType(b.taskType);
    return nameA.localeCompare(nameB);
  });

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
          {sortedRunningTasks.map(renderRunningTask)}
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
