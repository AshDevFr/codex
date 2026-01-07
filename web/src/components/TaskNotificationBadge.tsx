import { Badge, Tooltip, Stack, Text, Group } from "@mantine/core";
import { IconLoader2 } from "@tabler/icons-react";
import { useTaskProgress } from "@/hooks/useTaskProgress";

/**
 * Discreet task notification badge that appears near the app logo
 * Shows count of active tasks with a tooltip containing task details
 */
export function TaskNotificationBadge() {
	const { activeTasks } = useTaskProgress();

	const tasksArray = Array.from(activeTasks.values());
	const activeTasksFiltered = tasksArray.filter(
		(task) => task.status === "running" || task.status === "queued",
	);

	if (activeTasksFiltered.length === 0) {
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

	const getTaskSummary = (task: typeof activeTasksFiltered[0]) => {
		const taskName = formatTaskType(task.task_type);
		const progress = task.progress
			? ` (${task.progress.current}/${task.progress.total})`
			: "";
		return `${taskName}${progress}`;
	};

	const tooltipContent = (
		<Stack gap={4}>
			<Text size="xs" fw={600}>
				Active Tasks
			</Text>
			{activeTasksFiltered.map((task) => (
				<Group key={task.task_id} gap="xs">
					<IconLoader2
						size={12}
						style={{ color: "var(--mantine-color-blue-4)" }}
						className="rotating-icon-small"
					/>
					<Text size="xs">{getTaskSummary(task)}</Text>
				</Group>
			))}
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
				{activeTasksFiltered.length} pending task
				{activeTasksFiltered.length !== 1 ? "s" : ""}
			</Badge>
		</Tooltip>
	);
}
