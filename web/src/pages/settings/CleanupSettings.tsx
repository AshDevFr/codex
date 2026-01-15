import {
	Alert,
	Badge,
	Box,
	Button,
	Card,
	Group,
	Loader,
	Modal,
	SimpleGrid,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconFileX,
	IconPhoto,
	IconRefresh,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { cleanupApi } from "@/api/cleanup";
import type { CleanupResultDto, OrphanStatsDto } from "@/api/cleanup";

// Utility to format bytes as human-readable
function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
}

// Stat card component
function StatCard({
	title,
	value,
	subtitle,
	color,
	icon,
}: {
	title: string;
	value: string | number;
	subtitle?: string;
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
						{typeof value === "number" ? value.toLocaleString() : value}
					</Text>
					{subtitle && (
						<Text size="xs" c="dimmed">
							{subtitle}
						</Text>
					)}
				</div>
				<Box c={color}>{icon}</Box>
			</Group>
		</Card>
	);
}

export function CleanupSettings() {
	const queryClient = useQueryClient();
	const [cleanupModalOpened, setCleanupModalOpened] = useState(false);
	const [asyncCleanupModalOpened, setAsyncCleanupModalOpened] = useState(false);

	// Fetch orphan stats
	const {
		data: stats,
		isLoading: statsLoading,
		refetch: refetchStats,
	} = useQuery<OrphanStatsDto>({
		queryKey: ["orphan-stats"],
		queryFn: () => cleanupApi.getOrphanStats(),
		refetchInterval: 30000, // Refresh every 30 seconds
	});

	// Trigger async cleanup (background task)
	const triggerCleanupMutation = useMutation({
		mutationFn: cleanupApi.triggerCleanup,
		onSuccess: (data) => {
			setAsyncCleanupModalOpened(false);
			notifications.show({
				title: "Cleanup Task Queued",
				message: `Task ${data.task_id.slice(0, 8)}... has been queued. Check the Tasks page for progress.`,
				color: "blue",
			});
			// Invalidate task queries so the new task shows up
			queryClient.invalidateQueries({ queryKey: ["tasks"] });
			queryClient.invalidateQueries({ queryKey: ["task-stats"] });
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to queue cleanup task",
				color: "red",
			});
		},
	});

	// Delete orphans immediately (sync)
	const deleteOrphansMutation = useMutation<CleanupResultDto>({
		mutationFn: cleanupApi.deleteOrphans,
		onSuccess: (data) => {
			setCleanupModalOpened(false);
			queryClient.invalidateQueries({ queryKey: ["orphan-stats"] });

			const totalDeleted = data.thumbnails_deleted + data.covers_deleted;
			if (totalDeleted > 0) {
				notifications.show({
					title: "Cleanup Complete",
					message: `Deleted ${totalDeleted} orphaned files, freed ${formatBytes(data.bytes_freed)}`,
					color: "green",
				});
			} else {
				notifications.show({
					title: "Cleanup Complete",
					message: "No orphaned files found to delete",
					color: "blue",
				});
			}

			// Show failures if any
			if (data.failures > 0) {
				notifications.show({
					title: "Cleanup Warnings",
					message: `${data.failures} files could not be deleted`,
					color: "yellow",
				});
			}
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to run cleanup",
				color: "red",
			});
		},
	});

	const totalOrphaned =
		(stats?.orphaned_thumbnails || 0) + (stats?.orphaned_covers || 0);
	const hasOrphanedFiles = totalOrphaned > 0;

	if (statsLoading) {
		return (
			<Box py="xl" px="md">
				<Stack gap="xl" align="center">
					<Loader size="lg" />
					<Text c="dimmed">Scanning for orphaned files...</Text>
				</Stack>
			</Box>
		);
	}

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<div>
						<Title order={1}>File Cleanup</Title>
						<Text c="dimmed" size="sm">
							Manage orphaned thumbnail and cover files
						</Text>
					</div>
					<Group gap="xs">
						<Button
							variant="light"
							leftSection={<IconRefresh size={16} />}
							onClick={() => refetchStats()}
						>
							Refresh
						</Button>
						{hasOrphanedFiles && (
							<>
								<Button
									variant="light"
									color="blue"
									leftSection={<IconTrash size={16} />}
									onClick={() => setAsyncCleanupModalOpened(true)}
								>
									Queue Cleanup
								</Button>
								<Button
									variant="filled"
									color="orange"
									leftSection={<IconTrash size={16} />}
									onClick={() => setCleanupModalOpened(true)}
								>
									Clean Now
								</Button>
							</>
						)}
					</Group>
				</Group>

				{/* Info Alert */}
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="blue"
					title="About Orphaned Files"
				>
					Orphaned files are thumbnails and covers that remain on disk after
					their associated books or series have been deleted. These files
					accumulate over time and can consume disk space unnecessarily. Cleanup
					is automatically triggered when entities are deleted, but you can also
					run a manual scan to find any missed files.
				</Alert>

				{/* Stats Cards */}
				<SimpleGrid cols={{ base: 1, sm: 2, md: 3 }} spacing="md">
					<StatCard
						title="Orphaned Thumbnails"
						value={stats?.orphaned_thumbnails || 0}
						subtitle="Book thumbnails without matching database records"
						color={stats?.orphaned_thumbnails ? "orange" : "green"}
						icon={<IconPhoto size={32} />}
					/>
					<StatCard
						title="Orphaned Covers"
						value={stats?.orphaned_covers || 0}
						subtitle="Series covers without matching database records"
						color={stats?.orphaned_covers ? "orange" : "green"}
						icon={<IconFileX size={32} />}
					/>
					<StatCard
						title="Total Space"
						value={formatBytes(stats?.total_size_bytes || 0)}
						subtitle="Disk space used by orphaned files"
						color={hasOrphanedFiles ? "orange" : "green"}
						icon={<IconTrash size={32} />}
					/>
				</SimpleGrid>

				{/* Status Card */}
				<Card withBorder>
					<Stack gap="md">
						<Group justify="space-between">
							<Title order={4}>Status</Title>
							{hasOrphanedFiles ? (
								<Badge color="orange" size="lg">
									{totalOrphaned} orphaned files found
								</Badge>
							) : (
								<Badge color="green" size="lg">
									No orphaned files
								</Badge>
							)}
						</Group>
						{hasOrphanedFiles ? (
							<Text c="dimmed">
								Found {stats?.orphaned_thumbnails || 0} orphaned thumbnails and{" "}
								{stats?.orphaned_covers || 0} orphaned covers. You can either
								queue a background cleanup task or clean them immediately.
							</Text>
						) : (
							<Text c="dimmed">
								All files on disk have matching database records. No cleanup
								needed at this time.
							</Text>
						)}
					</Stack>
				</Card>

				{/* Async Cleanup Modal */}
				<Modal
					opened={asyncCleanupModalOpened}
					onClose={() => setAsyncCleanupModalOpened(false)}
					title="Queue Cleanup Task"
					centered
				>
					<Stack gap="md">
						<Text>
							This will queue a background task to scan for and delete orphaned
							files. The task will run with low priority and won&apos;t
							interfere with other operations.
						</Text>
						<Text size="sm" c="dimmed">
							Found: {stats?.orphaned_thumbnails || 0} thumbnails,{" "}
							{stats?.orphaned_covers || 0} covers (
							{formatBytes(stats?.total_size_bytes || 0)})
						</Text>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setAsyncCleanupModalOpened(false)}
							>
								Cancel
							</Button>
							<Button
								color="blue"
								loading={triggerCleanupMutation.isPending}
								onClick={() => triggerCleanupMutation.mutate()}
							>
								Queue Task
							</Button>
						</Group>
					</Stack>
				</Modal>

				{/* Immediate Cleanup Modal */}
				<Modal
					opened={cleanupModalOpened}
					onClose={() => setCleanupModalOpened(false)}
					title="Clean Orphaned Files"
					centered
				>
					<Stack gap="md">
						<Alert icon={<IconAlertCircle size={16} />} color="yellow">
							This will immediately delete all orphaned files. This action
							cannot be undone.
						</Alert>
						<Text>
							Are you sure you want to delete {totalOrphaned} orphaned files?
						</Text>
						<Text size="sm" c="dimmed">
							This will free approximately{" "}
							{formatBytes(stats?.total_size_bytes || 0)} of disk space.
						</Text>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setCleanupModalOpened(false)}
							>
								Cancel
							</Button>
							<Button
								color="orange"
								loading={deleteOrphansMutation.isPending}
								onClick={() => deleteOrphansMutation.mutate()}
							>
								Delete Files
							</Button>
						</Group>
					</Stack>
				</Modal>
			</Stack>
		</Box>
	);
}
