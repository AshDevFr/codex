import { Badge, Tooltip, Stack, Text, Group } from "@mantine/core";
import { IconLoader2 } from "@tabler/icons-react";
import { useTaskProgress } from "@/hooks/useTaskProgress";

/**
 * Discreet task notification badge that appears near the app logo
 * Shows count of active tasks with a tooltip containing task details
 */
export function TaskNotificationBadge() {
	const { activeTasks, pendingCounts } = useTaskProgress();

	// Filter to only running/pending tasks
	const runningTasks = activeTasks.filter(
		(task) => task.status === "running" || task.status === "pending",
	);

	// Calculate total pending count
	const totalPendingCount = Object.values(pendingCounts).reduce((sum, count) => sum + count, 0);

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

	const getTaskSummary = (task: typeof runningTasks[0]) => {
		const taskName = formatTaskType(task.task_type);
		const progress = task.progress
			? ` (${task.progress.current}/${task.progress.total})`
			: "";
		return `${taskName}${progress}`;
	};

	const tooltipContent = (
		<Stack gap={8}>
			{runningTasks.length > 0 && (
				<Stack gap={4}>
					<Text size="xs" fw={600}>
						Running Tasks
					</Text>
					{runningTasks.map((task) => (
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

			{totalPendingCount > 0 && (
				<Stack gap={4}>
					<Text size="xs" fw={600}>
						Pending Tasks ({totalPendingCount})
					</Text>
					{Object.entries(pendingCounts).map(([taskType, count]) => (
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
		<Tooltip label={tooltipContent} withArrow position="bottom-start">
			<Badge
				color="blue"
				variant="filled"
				size="sm"
				style={{
					cursor: "pointer",
					animation: "pulse 2s ease-in-out infinite",
				}}
			>
				{totalTasks} pending task
				{totalTasks !== 1 ? "s" : ""}
			</Badge>
		</Tooltip>
	);
}
