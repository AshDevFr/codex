import {
	ActionIcon,
	Box,
	Breadcrumbs,
	Button,
	Center,
	Divider,
	Grid,
	Group,
	Image,
	Loader,
	Menu,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAnalyze,
	IconArrowLeft,
	IconBook,
	IconBookOff,
	IconCheck,
	IconChevronRight,
	IconDotsVertical,
	IconDownload,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { seriesApi } from "@/api/series";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import {
	AlternateTitles,
	ExternalLinks,
	ExternalRatings,
	GenreTagChips,
	SeriesBookList,
	SeriesMetadata,
	SeriesRating,
} from "@/components/series";

export function SeriesDetail() {
	const { seriesId } = useParams<{ seriesId: string }>();
	const navigate = useNavigate();
	const queryClient = useQueryClient();

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

	// Build breadcrumbs
	const breadcrumbItems = [
		{ title: "Home", href: "/" },
		{ title: "Libraries", href: "/libraries/all/series" },
	];

	if (series.libraryId) {
		breadcrumbItems.push({
			title: series.libraryName || "Library",
			href: `/libraries/${series.libraryId}/series`,
		});
	}

	breadcrumbItems.push({
		title: series.name,
		href: `/series/${series.id}`,
	});

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
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

				{/* Header section with cover, title, and actions */}
				<Grid gutter="xl">
					{/* Cover image */}
					<Grid.Col span={{ base: 12, sm: 4, md: 3 }}>
						<Box
							style={{
								position: "relative",
								width: "100%",
								maxWidth: 300,
								margin: "0 auto",
							}}
						>
							<Image
								src={coverUrl}
								alt={series.name}
								radius="md"
								fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='300' height='425'%3E%3Crect fill='%23ddd' width='300' height='425'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='14' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
								style={{ aspectRatio: "150/212.125" }}
							/>
						</Box>
					</Grid.Col>

					{/* Title, metadata, and actions */}
					<Grid.Col span={{ base: 12, sm: 8, md: 9 }}>
						<Stack gap="md">
							<Group justify="space-between" align="flex-start">
								<Stack gap="xs">
									<Group gap="xs">
										<ActionIcon
											variant="subtle"
											onClick={() => navigate(-1)}
											title="Go back"
										>
											<IconArrowLeft size={20} />
										</ActionIcon>
										<Title order={1}>{series.name}</Title>
									</Group>
									{metadata?.publisher && (
										<Text size="lg" c="dimmed">
											{metadata.publisher}
										</Text>
									)}
								</Stack>

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
										<Menu.Divider />
										<Menu.Item
											component="a"
											href={`/api/v1/series/${series.id}/file`}
											leftSection={<IconDownload size={14} />}
										>
											Download Series
										</Menu.Item>
									</Menu.Dropdown>
								</Menu>
							</Group>

							{/* External ratings */}
							{metadata?.externalRatings &&
								metadata.externalRatings.length > 0 && (
									<ExternalRatings ratings={metadata.externalRatings} />
								)}

							{/* Action buttons */}
							<Group gap="sm">
								<Button
									leftSection={<IconBook size={16} />}
									onClick={() => {
										// Navigate to first unread book, or first book
										// This will be improved when we have the adjacent books endpoint
										navigate(`/libraries/${series.libraryId}/series`);
									}}
								>
									Read
								</Button>
								<Button
									variant="outline"
									component="a"
									href={`/api/v1/series/${series.id}/file`}
									leftSection={<IconDownload size={16} />}
								>
									Download
								</Button>
							</Group>

							{/* External links */}
							{metadata?.externalLinks &&
								metadata.externalLinks.length > 0 && (
									<Box>
										<Text size="sm" fw={500} mb="xs">
											External Links
										</Text>
										<ExternalLinks links={metadata.externalLinks} />
									</Box>
								)}
						</Stack>
					</Grid.Col>
				</Grid>

				<Divider />

				{/* Summary */}
				{metadata?.summary && (
					<>
						<Stack gap="xs">
							<Title order={4}>Summary</Title>
							<Text style={{ whiteSpace: "pre-wrap" }}>{metadata.summary}</Text>
						</Stack>
						<Divider />
					</>
				)}

				{/* Metadata grid */}
				{metadata && <SeriesMetadata metadata={metadata} />}

				{/* Genres and Tags */}
				{metadata &&
					(metadata.genres.length > 0 || metadata.tags.length > 0) && (
						<>
							<Divider />
							<Stack gap="md">
								{metadata.genres.length > 0 && (
									<Box>
										<Text size="sm" fw={500} mb="xs">
											Genres
										</Text>
										<GenreTagChips
											genres={metadata.genres}
											libraryId={series.libraryId}
										/>
									</Box>
								)}
								{metadata.tags.length > 0 && (
									<Box>
										<Text size="sm" fw={500} mb="xs">
											Tags
										</Text>
										<GenreTagChips
											tags={metadata.tags}
											libraryId={series.libraryId}
										/>
									</Box>
								)}
							</Stack>
						</>
					)}

				{/* Alternate titles */}
				{metadata?.alternateTitles && metadata.alternateTitles.length > 0 && (
					<>
						<Divider />
						<Stack gap="xs">
							<Title order={4}>Alternate Titles</Title>
							<AlternateTitles titles={metadata.alternateTitles} />
						</Stack>
					</>
				)}

				{/* User Rating */}
				<Divider />
				<SeriesRating seriesId={series.id} />

				{/* Books list */}
				<Divider />
				<SeriesBookList
					seriesId={series.id}
					seriesName={series.name}
					bookCount={series.bookCount ?? 0}
				/>
			</Stack>
		</Box>
	);
}
