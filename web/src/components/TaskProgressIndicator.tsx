import { Box, Group, Paper, Progress, Stack, Text } from "@mantine/core";
import { IconCircleCheck, IconCircleX, IconLoader2 } from "@tabler/icons-react";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { TaskProgressEvent } from "@/types";

interface TaskProgressItemProps {
	task: TaskProgressEvent;
}

function TaskProgressItem({ task }: TaskProgressItemProps) {
	const isRunning = task.status === "running";
	const isCompleted = task.status === "completed";
	const isFailed = task.status === "failed";

	const progress = task.progress
		? (task.progress.current / task.progress.total) * 100
		: isCompleted
			? 100
			: 0;

	const getStatusIcon = () => {
		if (isCompleted)
			return (
				<IconCircleCheck
					size={16}
					style={{ color: "var(--mantine-color-green-6)" }}
				/>
			);
		if (isFailed)
			return (
				<IconCircleX
					size={16}
					style={{ color: "var(--mantine-color-red-6)" }}
				/>
			);
		return (
			<IconLoader2
				size={16}
				style={{ color: "var(--mantine-color-blue-6)" }}
				className="rotating-icon"
			/>
		);
	};

	const getStatusColor = () => {
		if (isCompleted) return "green";
		if (isFailed) return "red";
		return "blue";
	};

	const formatTaskType = (type: string) => {
		return type
			.replace(/([A-Z])/g, " $1")
			.replace(/_/g, " ")
			.trim()
			.split(" ")
			.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
			.join(" ");
	};

	return (
		<Paper
			shadow="sm"
			p="md"
			withBorder
			style={{
				borderLeft: `4px solid var(--mantine-color-${getStatusColor()}-6)`,
				marginBottom: "0.5rem",
			}}
		>
			<Group align="flex-start" gap="sm">
				<Box mt={2}>{getStatusIcon()}</Box>

				<Stack gap="xs" style={{ flex: 1 }}>
					<Group justify="space-between" gap="xs">
						<Text size="sm" fw={500} c={getStatusColor()}>
							{formatTaskType(task.task_type)}
						</Text>
						{task.status !== "pending" && (
							<Text size="xs" c="dimmed" tt="capitalize">
								{task.status}
							</Text>
						)}
					</Group>

					{task.progress?.message && (
						<Text size="xs" c="dimmed">
							{task.progress.message}
						</Text>
					)}

					{task.error && (
						<Text size="xs" c="red">
							{task.error}
						</Text>
					)}

					{isRunning && task.progress && (
						<Stack gap={4}>
							<Progress value={progress} size="sm" color={getStatusColor()} />
							<Text size="xs" c="dimmed">
								{task.progress.current} / {task.progress.total}
							</Text>
						</Stack>
					)}
				</Stack>
			</Group>
		</Paper>
	);
}

export function TaskProgressIndicator() {
	const { activeTasks, connectionState } = useTaskProgress();

	// Show only running tasks (processing tasks are converted to running in the hook)
	// Note: pending tasks are not shown here as they're represented by pendingCounts in the badge
	const visibleTasks = activeTasks.filter((task) => task.status === "running");

	if (visibleTasks.length === 0) {
		return null;
	}

	return (
		<Box
			style={{
				maxHeight: "300px",
				overflowY: "auto",
			}}
		>
			{visibleTasks.map((task) => (
				<TaskProgressItem key={task.task_id} task={task} />
			))}

			{connectionState === "connecting" && (
				<Text size="xs" c="dimmed" ta="center" mt="xs">
					Connecting to task updates...
				</Text>
			)}
			{connectionState === "failed" && (
				<Text size="xs" c="red" ta="center" mt="xs">
					Failed to connect to task updates
				</Text>
			)}

			<style>
				{`
					@keyframes rotate {
						from { transform: rotate(0deg); }
						to { transform: rotate(360deg); }
					}
					.rotating-icon {
						animation: rotate 1s linear infinite;
					}
				`}
			</style>
		</Box>
	);
}
