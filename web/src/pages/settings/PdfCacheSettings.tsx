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
	IconBook,
	IconClock,
	IconFile,
	IconFolder,
	IconRefresh,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type {
	PdfCacheCleanupResultDto,
	PdfCacheStatsDto,
} from "@/api/pdfCache";
import { pdfCacheApi } from "@/api/pdfCache";
import { useTaskProgress } from "@/hooks/useTaskProgress";

// Cleanup task types that should trigger a stats refresh
const PDF_CACHE_TASK_TYPES = ["cleanup_pdf_cache"];

// Throttle duration for stats refresh (30 seconds)
const REFRESH_THROTTLE_MS = 30000;

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

export function PdfCacheSettings() {
	const queryClient = useQueryClient();
	const [clearModalOpened, setClearModalOpened] = useState(false);
	const [cleanupModalOpened, setCleanupModalOpened] = useState(false);

	// Track completed cleanup tasks to trigger refresh
	const { activeTasks } = useTaskProgress();
	const lastRefreshTime = useRef<number>(0);
	const processedTaskIds = useRef<Set<string>>(new Set());

	// Fetch cache stats
	const {
		data: stats,
		isLoading: statsLoading,
		refetch: refetchStats,
	} = useQuery<PdfCacheStatsDto>({
		queryKey: ["pdf-cache-stats"],
		queryFn: () => pdfCacheApi.getStats(),
	});

	// Watch for cleanup task completions and refresh stats (throttled to 30s)
	useEffect(() => {
		const completedCleanupTasks = activeTasks.filter(
			(task) =>
				PDF_CACHE_TASK_TYPES.includes(task.task_type) &&
				task.status === "completed" &&
				!processedTaskIds.current.has(task.task_id),
		);

		if (completedCleanupTasks.length > 0) {
			// Mark these tasks as processed
			for (const task of completedCleanupTasks) {
				processedTaskIds.current.add(task.task_id);
			}

			// Throttle refresh to avoid hammering the API
			const now = Date.now();
			if (now - lastRefreshTime.current >= REFRESH_THROTTLE_MS) {
				lastRefreshTime.current = now;
				refetchStats();
			}
		}
	}, [activeTasks, refetchStats]);

	// Trigger async cleanup (background task)
	const triggerCleanupMutation = useMutation({
		mutationFn: pdfCacheApi.triggerCleanup,
		onSuccess: (data) => {
			setCleanupModalOpened(false);
			notifications.show({
				title: "Cleanup Task Queued",
				message: `Task ${data.task_id.slice(0, 8)}... has been queued. Cleaning pages older than ${data.max_age_days} days.`,
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

	// Clear cache immediately (sync)
	const clearCacheMutation = useMutation<PdfCacheCleanupResultDto>({
		mutationFn: pdfCacheApi.clearCache,
		onSuccess: (data) => {
			setClearModalOpened(false);
			queryClient.invalidateQueries({ queryKey: ["pdf-cache-stats"] });

			if (data.files_deleted > 0) {
				notifications.show({
					title: "Cache Cleared",
					message: `Deleted ${data.files_deleted.toLocaleString()} cached pages, freed ${data.bytes_reclaimed_human}`,
					color: "green",
				});
			} else {
				notifications.show({
					title: "Cache Cleared",
					message: "Cache was already empty",
					color: "blue",
				});
			}
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to clear cache",
				color: "red",
			});
		},
	});

	const hasCachedFiles = (stats?.total_files || 0) > 0;
	const cacheEnabled = stats?.cache_enabled ?? false;

	if (statsLoading) {
		return (
			<Box py="xl" px="md">
				<Stack gap="xl" align="center">
					<Loader size="lg" />
					<Text c="dimmed">Loading cache statistics...</Text>
				</Stack>
			</Box>
		);
	}

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<div>
						<Title order={1}>PDF Page Cache</Title>
						<Text c="dimmed" size="sm">
							Manage cached PDF page renders
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
						{hasCachedFiles && (
							<>
								<Button
									variant="light"
									color="blue"
									leftSection={<IconTrash size={16} />}
									onClick={() => setCleanupModalOpened(true)}
								>
									Cleanup Old
								</Button>
								<Button
									variant="filled"
									color="orange"
									leftSection={<IconTrash size={16} />}
									onClick={() => setClearModalOpened(true)}
								>
									Clear All
								</Button>
							</>
						)}
					</Group>
				</Group>

				{/* Info Alert */}
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="blue"
					title="About PDF Page Cache"
				>
					When using streaming mode for PDF files, pages are rendered on the
					server and cached to improve subsequent load times. The cache is
					automatically cleaned up based on the configured max age (default 30
					days). You can manually trigger cleanup or clear the entire cache
					here.
				</Alert>

				{/* Cache Status */}
				{!cacheEnabled && (
					<Alert icon={<IconAlertCircle size={16} />} color="yellow">
						PDF page caching is currently disabled. Enable it in your server
						configuration to improve PDF streaming performance.
					</Alert>
				)}

				{/* Stats Cards */}
				<SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="md">
					<StatCard
						title="Cached Pages"
						value={stats?.total_files || 0}
						subtitle="Total rendered page images"
						color={hasCachedFiles ? "blue" : "gray"}
						icon={<IconFile size={32} />}
					/>
					<StatCard
						title="Cache Size"
						value={stats?.total_size_human || "0 B"}
						subtitle={`${(stats?.total_size_bytes || 0).toLocaleString()} bytes`}
						color={hasCachedFiles ? "blue" : "gray"}
						icon={<IconFolder size={32} />}
					/>
					<StatCard
						title="Books Cached"
						value={stats?.book_count || 0}
						subtitle="Unique books with cached pages"
						color={hasCachedFiles ? "blue" : "gray"}
						icon={<IconBook size={32} />}
					/>
					<StatCard
						title="Oldest Page"
						value={
							stats?.oldest_file_age_days !== undefined
								? `${stats.oldest_file_age_days} days`
								: "N/A"
						}
						subtitle="Age of oldest cached page"
						color={
							stats?.oldest_file_age_days && stats.oldest_file_age_days > 30
								? "orange"
								: "gray"
						}
						icon={<IconClock size={32} />}
					/>
				</SimpleGrid>

				{/* Status Card */}
				<Card withBorder>
					<Stack gap="md">
						<Group justify="space-between">
							<Title order={4}>Status</Title>
							{cacheEnabled ? (
								hasCachedFiles ? (
									<Badge color="blue" size="lg">
										{stats?.total_files.toLocaleString()} pages cached
									</Badge>
								) : (
									<Badge color="gray" size="lg">
										Cache empty
									</Badge>
								)
							) : (
								<Badge color="yellow" size="lg">
									Cache disabled
								</Badge>
							)}
						</Group>
						<Group gap="xs">
							<Text size="sm" c="dimmed">
								Cache directory:
							</Text>
							<Text size="sm" ff="monospace">
								{stats?.cache_dir || "N/A"}
							</Text>
						</Group>
						{hasCachedFiles ? (
							<Text c="dimmed">
								The cache contains {stats?.total_files.toLocaleString()}{" "}
								rendered pages from {stats?.book_count.toLocaleString()} books,
								using {stats?.total_size_human} of disk space.
								{stats?.oldest_file_age_days !== undefined && (
									<>
										{" "}
										The oldest cached page is {stats.oldest_file_age_days} days
										old.
									</>
								)}
							</Text>
						) : (
							<Text c="dimmed">
								No pages are currently cached. Pages will be cached as PDFs are
								viewed in streaming mode.
							</Text>
						)}
					</Stack>
				</Card>

				{/* Cleanup Modal */}
				<Modal
					opened={cleanupModalOpened}
					onClose={() => setCleanupModalOpened(false)}
					title="Cleanup Old Cache Entries"
					centered
				>
					<Stack gap="md">
						<Text>
							This will queue a background task to remove cached pages older
							than the configured max age (default 30 days). Recently accessed
							pages will be preserved.
						</Text>
						<Text size="sm" c="dimmed">
							Current cache: {stats?.total_files.toLocaleString() || 0} pages (
							{stats?.total_size_human || "0 B"})
						</Text>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setCleanupModalOpened(false)}
							>
								Cancel
							</Button>
							<Button
								color="blue"
								loading={triggerCleanupMutation.isPending}
								onClick={() => triggerCleanupMutation.mutate()}
							>
								Queue Cleanup
							</Button>
						</Group>
					</Stack>
				</Modal>

				{/* Clear All Modal */}
				<Modal
					opened={clearModalOpened}
					onClose={() => setClearModalOpened(false)}
					title="Clear Entire Cache"
					centered
				>
					<Stack gap="md">
						<Alert icon={<IconAlertCircle size={16} />} color="orange">
							This will immediately delete all cached PDF pages. PDFs will need
							to be re-rendered when next viewed in streaming mode.
						</Alert>
						<Text>
							Are you sure you want to clear the entire PDF page cache?
						</Text>
						<Text size="sm" c="dimmed">
							This will delete {stats?.total_files.toLocaleString() || 0} cached
							pages and free {stats?.total_size_human || "0 B"} of disk space.
						</Text>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setClearModalOpened(false)}
							>
								Cancel
							</Button>
							<Button
								color="orange"
								loading={clearCacheMutation.isPending}
								onClick={() => clearCacheMutation.mutate()}
							>
								Clear Cache
							</Button>
						</Group>
					</Stack>
				</Modal>
			</Stack>
		</Box>
	);
}
