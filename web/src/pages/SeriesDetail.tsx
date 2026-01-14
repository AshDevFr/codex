import {
	ActionIcon,
	Badge,
	Box,
	Breadcrumbs,
	Button,
	Center,
	Grid,
	Group,
	Image,
	Loader,
	Menu,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
	IconAnalyze,
	IconBookOff,
	IconCheck,
	IconChevronDown,
	IconChevronRight,
	IconChevronUp,
	IconDotsVertical,
	IconDownload,
	IconEdit,
	IconPhoto,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { seriesApi } from "@/api/series";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import {
	AlternateTitles,
	ExternalLinks,
	GenreTagChips,
	SeriesBookList,
	SeriesMetadataEditModal,
	SeriesRating,
} from "@/components/series";

// Helper to format reading direction
function formatReadingDirection(dir?: string | null): string | null {
	if (!dir) return null;
	const map: Record<string, string> = {
		ltr: "Left to Right",
		rtl: "Right to Left",
		ttb: "Vertical",
		webtoon: "Webtoon",
	};
	return map[dir] || dir;
}

// Helper to format status
function formatStatus(status?: string | null): string | null {
	if (!status) return null;
	return status.charAt(0).toUpperCase() + status.slice(1);
}

export function SeriesDetail() {
	const { seriesId } = useParams<{ seriesId: string }>();
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const [summaryOpened, { toggle: toggleSummary }] = useDisclosure(false);
	const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
		useDisclosure(false);

	// Fetch series basic info
	const {
		data: series,
		isLoading: seriesLoading,
		error: seriesError,
	} = useQuery({
		queryKey: ["series", seriesId],
		queryFn: () => seriesApi.getById(seriesId!),
		enabled: !!seriesId,
	});

	// Fetch full metadata
	const {
		data: metadata,
		isLoading: metadataLoading,
		error: metadataError,
	} = useQuery({
		queryKey: ["series-metadata", seriesId],
		queryFn: () => seriesMetadataApi.getFullMetadata(seriesId!),
		enabled: !!seriesId,
	});

	// Mark as read mutation
	const markAsReadMutation = useMutation({
		mutationFn: () => seriesApi.markAsRead(seriesId!),
		onSuccess: (data) => {
			notifications.show({
				title: "Marked as read",
				message: data.message,
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
			queryClient.invalidateQueries({ queryKey: ["series-books", seriesId] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed",
				message: error.message,
				color: "red",
			});
		},
	});

	// Mark as unread mutation
	const markAsUnreadMutation = useMutation({
		mutationFn: () => seriesApi.markAsUnread(seriesId!),
		onSuccess: (data) => {
			notifications.show({
				title: "Marked as unread",
				message: data.message,
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
			queryClient.invalidateQueries({ queryKey: ["series-books", seriesId] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed",
				message: error.message,
				color: "red",
			});
		},
	});

	// Analyze mutation
	const analyzeMutation = useMutation({
		mutationFn: () => seriesApi.analyze(seriesId!),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "All books in series queued for analysis",
				color: "blue",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed",
				message: error.message,
				color: "red",
			});
		},
	});

	// Analyze unanalyzed mutation
	const analyzeUnanalyzedMutation = useMutation({
		mutationFn: () => seriesApi.analyzeUnanalyzed(seriesId!),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Unanalyzed books queued for analysis",
				color: "blue",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed",
				message: error.message,
				color: "red",
			});
		},
	});

	// Generate thumbnails mutation
	const generateThumbnailsMutation = useMutation({
		mutationFn: () => seriesApi.generateThumbnails(seriesId!),
		onSuccess: () => {
			notifications.show({
				title: "Thumbnails generation started",
				message: "All books queued for thumbnail generation",
				color: "blue",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed",
				message: error.message,
				color: "red",
			});
		},
	});

	const isLoading = seriesLoading || metadataLoading;
	const error = seriesError || metadataError;

	if (isLoading) {
		return (
			<Center h={400}>
				<Loader size="lg" />
			</Center>
		);
	}

	if (error || !series) {
		return (
			<Center h={400}>
				<Stack align="center" gap="md">
					<Text size="xl" fw={600}>
						Series Not Found
					</Text>
					<Text c="dimmed">The requested series could not be found.</Text>
					<Button onClick={() => navigate(-1)}>Go Back</Button>
				</Stack>
			</Center>
		);
	}

	const coverUrl = `/api/v1/series/${series.id}/thumbnail`;
	const hasUnread = (series.unreadCount ?? 0) > 0;
	const hasRead = (series.bookCount ?? 0) > (series.unreadCount ?? 0);
	const readingDirection = formatReadingDirection(metadata?.readingDirection);
	const status = formatStatus(metadata?.status);

	// Build breadcrumbs
	const breadcrumbItems: { title: string; href: string }[] = [
		{ title: "Home", href: "/" },
	];

	if (series.libraryId) {
		breadcrumbItems.push({
			title: (series as { libraryName?: string }).libraryName || "Library",
			href: `/libraries/${series.libraryId}/series`,
		});
	}

	breadcrumbItems.push({
		title: series.name,
		href: `/series/${series.id}`,
	});

	return (
		<Box py="md" px="md">
			<Stack gap="md">
				{/* Breadcrumbs */}
				<Breadcrumbs separator={<IconChevronRight size={14} />}>
					{breadcrumbItems.map((item, index) =>
						index === breadcrumbItems.length - 1 ? (
							<Text key={item.href} size="sm">
								{item.title}
							</Text>
						) : (
							<Text
								key={item.href}
								component={Link}
								to={item.href}
								size="sm"
								c="dimmed"
								style={{ textDecoration: "none" }}
							>
								{item.title}
							</Text>
						),
					)}
				</Breadcrumbs>

				{/* Header: Cover + Info side by side */}
				<Grid gutter="md">
					{/* Cover - smaller */}
					<Grid.Col span={{ base: 4, xs: 3, sm: 2 }}>
						<Image
							src={coverUrl}
							alt={series.name}
							radius="sm"
							fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='150' height='212'%3E%3Crect fill='%23333' width='150' height='212'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='12' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
							style={{ aspectRatio: "150/212.125" }}
						/>
					</Grid.Col>

					{/* Info */}
					<Grid.Col span={{ base: 8, xs: 9, sm: 10 }}>
						<Stack gap="xs">
							{/* Title row with badges and menu */}
							<Group justify="space-between" align="flex-start" wrap="nowrap">
								<Box style={{ flex: 1, minWidth: 0 }}>
									<Group gap="sm" align="center" wrap="wrap">
										<Title order={2} style={{ wordBreak: "break-word" }}>
											{series.name}
										</Title>
										{metadata?.publisher && (
											<Text size="sm" c="dimmed">
												in{" "}
												{(series as { libraryName?: string }).libraryName ||
													"Library"}
											</Text>
										)}
									</Group>
									<Group gap="xs" mt={4}>
										{status && (
											<Badge
												size="sm"
												variant="filled"
												color={status === "Ended" ? "green" : "blue"}
											>
												{status}
											</Badge>
										)}
										{readingDirection && (
											<Badge size="sm" variant="outline">
												{readingDirection}
											</Badge>
										)}
									</Group>
								</Box>

								<Menu shadow="md" width={200} position="bottom-end">
									<Menu.Target>
										<ActionIcon variant="subtle" size="lg">
											<IconDotsVertical size={20} />
										</ActionIcon>
									</Menu.Target>
									<Menu.Dropdown>
										{hasUnread && (
											<Menu.Item
												leftSection={<IconCheck size={14} />}
												onClick={() => markAsReadMutation.mutate()}
												disabled={markAsReadMutation.isPending}
											>
												Mark as Read
											</Menu.Item>
										)}
										{hasRead && (
											<Menu.Item
												leftSection={<IconBookOff size={14} />}
												onClick={() => markAsUnreadMutation.mutate()}
												disabled={markAsUnreadMutation.isPending}
											>
												Mark as Unread
											</Menu.Item>
										)}
										<Menu.Divider />
										<Menu.Item
											leftSection={<IconAnalyze size={14} />}
											onClick={() => analyzeMutation.mutate()}
											disabled={analyzeMutation.isPending}
										>
											Analyze All
										</Menu.Item>
										<Menu.Item
											leftSection={<IconAnalyze size={14} />}
											onClick={() => analyzeUnanalyzedMutation.mutate()}
											disabled={analyzeUnanalyzedMutation.isPending}
										>
											Analyze Unanalyzed
										</Menu.Item>
										<Menu.Item
											leftSection={<IconPhoto size={14} />}
											onClick={() => generateThumbnailsMutation.mutate()}
											disabled={generateThumbnailsMutation.isPending}
										>
											Generate Thumbnails
										</Menu.Item>
										<Menu.Divider />
										<Menu.Item
											leftSection={<IconEdit size={14} />}
											onClick={openEditModal}
										>
											Edit Metadata
										</Menu.Item>
									</Menu.Dropdown>
								</Menu>
							</Group>

							{/* Book count */}
							<Text size="sm" c="dimmed">
								{series.bookCount ?? 0} / {series.bookCount ?? 0} books
							</Text>

							{/* Alternate titles inline */}
							{metadata?.alternateTitles &&
								metadata.alternateTitles.length > 0 && (
									<AlternateTitles titles={metadata.alternateTitles} compact />
								)}

							{/* Download button */}
							<Group gap="sm" mt="xs">
								<Button
									size="xs"
									variant="filled"
									component="a"
									href={`/api/v1/series/${series.id}/file`}
									leftSection={<IconDownload size={14} />}
								>
									Download
								</Button>
							</Group>

							{/* Summary - show preview with expand if long */}
							{metadata?.summary && (
								<Box mt="xs">
									<Text
										size="sm"
										style={{ whiteSpace: "pre-wrap" }}
										lineClamp={summaryOpened ? undefined : 2}
									>
										{metadata.summary}
									</Text>
									{/* Only show READ MORE if summary is long enough (roughly > 150 chars or has newlines) */}
									{(metadata.summary.length > 150 ||
										metadata.summary.includes("\n")) && (
										<Text
											size="sm"
											c="dimmed"
											style={{
												cursor: "pointer",
												display: "inline-flex",
												alignItems: "center",
												gap: 4,
											}}
											onClick={toggleSummary}
											mt={4}
										>
											{summaryOpened ? "READ LESS" : "READ MORE"}
											{summaryOpened ? (
												<IconChevronUp size={14} />
											) : (
												<IconChevronDown size={14} />
											)}
										</Text>
									)}
								</Box>
							)}
						</Stack>
					</Grid.Col>
				</Grid>

				{/* Metadata rows - Komga style */}
				<Stack gap="xs">
					{/* Publisher */}
					{metadata?.publisher && (
						<Group gap="md">
							<Text size="sm" c="dimmed" w={100}>
								PUBLISHER
							</Text>
							<Badge variant="outline" size="sm">
								{metadata.publisher}
							</Badge>
						</Group>
					)}

					{/* Genres */}
					{metadata && (metadata.genres?.length ?? 0) > 0 && (
						<Group gap="md" align="flex-start">
							<Text size="sm" c="dimmed" w={100}>
								GENRE
							</Text>
							<GenreTagChips
								genres={metadata.genres}
								libraryId={series.libraryId}
							/>
						</Group>
					)}

					{/* Tags */}
					{metadata && (metadata.tags?.length ?? 0) > 0 && (
						<Group gap="md" align="flex-start">
							<Text size="sm" c="dimmed" w={100}>
								TAGS
							</Text>
							<GenreTagChips
								tags={metadata.tags}
								libraryId={series.libraryId}
							/>
						</Group>
					)}

					{/* External Links */}
					{metadata?.externalLinks && metadata.externalLinks.length > 0 && (
						<Group gap="md" align="flex-start">
							<Text size="sm" c="dimmed" w={100}>
								LINKS
							</Text>
							<ExternalLinks links={metadata.externalLinks} />
						</Group>
					)}

					{/* User Rating - compact */}
					<Group gap="md" align="center">
						<Text size="sm" c="dimmed" w={100}>
							YOUR RATING
						</Text>
						<SeriesRating seriesId={series.id} />
					</Group>
				</Stack>

				{/* Books list */}
				<SeriesBookList
					seriesId={series.id}
					seriesName={series.name}
					bookCount={series.bookCount ?? 0}
				/>
			</Stack>

			{/* Edit Metadata Modal */}
			<SeriesMetadataEditModal
				opened={editModalOpened}
				onClose={closeEditModal}
				seriesId={series.id}
				seriesTitle={series.name}
			/>
		</Box>
	);
}
