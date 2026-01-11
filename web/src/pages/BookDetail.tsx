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
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAnalyze,
	IconArrowLeft,
	IconBook,
	IconBookOff,
	IconCheck,
	IconChevronLeft,
	IconChevronRight,
	IconDotsVertical,
	IconDownload,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import {
	BookFileInfo,
	BookMetadataDisplay,
	BookProgress,
} from "@/components/book";

export function BookDetail() {
	const { bookId } = useParams<{ bookId: string }>();
	const navigate = useNavigate();
	const queryClient = useQueryClient();

	// Fetch book details
	const {
		data: bookDetail,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["book-detail", bookId],
		queryFn: () => booksApi.getDetail(bookId!),
		enabled: !!bookId,
	});

	// Fetch adjacent books for series navigation
	const { data: adjacentBooks } = useQuery({
		queryKey: ["adjacent-books", bookId],
		queryFn: () => booksApi.getAdjacent(bookId!),
		enabled: !!bookId,
	});

	const book = bookDetail?.book;
	const metadata = bookDetail?.metadata;
	const prevBook = adjacentBooks?.prev;
	const nextBook = adjacentBooks?.next;

	// Mark as read mutation
	const markAsReadMutation = useMutation({
		mutationFn: () => booksApi.markAsRead(bookId!),
		onSuccess: () => {
			notifications.show({
				title: "Marked as read",
				message: "Book marked as read",
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["book-detail", bookId] });
			queryClient.invalidateQueries({ queryKey: ["books"] });
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
		mutationFn: () => booksApi.markAsUnread(bookId!),
		onSuccess: () => {
			notifications.show({
				title: "Marked as unread",
				message: "Book marked as unread",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["book-detail", bookId] });
			queryClient.invalidateQueries({ queryKey: ["books"] });
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
		mutationFn: () => booksApi.analyze(bookId!),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Book queued for analysis",
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

	if (isLoading) {
		return (
			<Center h={400}>
				<Loader size="lg" />
			</Center>
		);
	}

	if (error || !book) {
		return (
			<Center h={400}>
				<Stack align="center" gap="md">
					<Text size="xl" fw={600}>
						Book Not Found
					</Text>
					<Text c="dimmed">The requested book could not be found.</Text>
					<Button onClick={() => navigate(-1)}>Go Back</Button>
				</Stack>
			</Center>
		);
	}

	const coverUrl = `/api/v1/books/${book.id}/thumbnail`;
	const downloadUrl = `/api/v1/books/${book.id}/file`;
	const hasProgress = !!book.readProgress;
	const isCompleted = book.readProgress?.completed ?? false;

	// Build display title
	const displayTitle =
		book.number !== undefined && book.number !== null
			? `${book.number} - ${book.title}`
			: book.title;

	// Build breadcrumbs
	const breadcrumbItems = [
		{ title: "Home", href: "/" },
		{ title: book.seriesName, href: `/series/${book.seriesId}` },
		{ title: displayTitle, href: `/books/${book.id}` },
	];

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
								alt={book.title}
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
										<Title order={1}>{displayTitle}</Title>
									</Group>
									<Text
										component={Link}
										to={`/series/${book.seriesId}`}
										size="lg"
										c="dimmed"
										style={{ textDecoration: "none" }}
									>
										in {book.seriesName}
									</Text>
								</Stack>

								<Menu shadow="md" width={200} position="bottom-end">
									<Menu.Target>
										<ActionIcon variant="subtle" size="lg">
											<IconDotsVertical size={20} />
										</ActionIcon>
									</Menu.Target>
									<Menu.Dropdown>
										{!isCompleted && (
											<Menu.Item
												leftSection={<IconCheck size={14} />}
												onClick={() => markAsReadMutation.mutate()}
												disabled={markAsReadMutation.isPending}
											>
												Mark as Read
											</Menu.Item>
										)}
										{hasProgress && (
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
											Analyze Book
										</Menu.Item>
									</Menu.Dropdown>
								</Menu>
							</Group>

							{/* Reading progress */}
							<BookProgress
								progress={book.readProgress}
								pageCount={book.pageCount}
							/>

							{/* Action buttons */}
							<Group gap="sm">
								<Button
									leftSection={<IconBook size={16} />}
									onClick={() => {
										// Navigate to reader (placeholder - reader not implemented yet)
										const page = book.readProgress?.current_page ?? 0;
										navigate(`/reader/${book.id}?page=${page}`);
									}}
								>
									{hasProgress && !isCompleted ? "Continue Reading" : "Read"}
								</Button>
								<Button
									variant="outline"
									component="a"
									href={downloadUrl}
									leftSection={<IconDownload size={16} />}
								>
									Download
								</Button>
							</Group>

							{/* Analysis error */}
							{book.analysisError && (
								<Box
									p="sm"
									style={{
										backgroundColor: "var(--mantine-color-red-light)",
										borderRadius: "var(--mantine-radius-sm)",
									}}
								>
									<Text size="sm" c="red">
										Analysis Error: {book.analysisError}
									</Text>
								</Box>
							)}
						</Stack>
					</Grid.Col>
				</Grid>

				<Divider />

				{/* File information */}
				<BookFileInfo book={book} />

				{/* Metadata */}
				{metadata && (
					<>
						<Divider />
						<BookMetadataDisplay metadata={metadata} />
					</>
				)}

				{/* Series navigation */}
				<Divider />
				<Group justify="space-between">
					{prevBook ? (
						<Tooltip label={`Previous: ${prevBook.title}`} position="top">
							<Button
								variant="subtle"
								leftSection={<IconChevronLeft size={16} />}
								onClick={() => navigate(`/books/${prevBook.id}`)}
							>
								Previous Book
							</Button>
						</Tooltip>
					) : (
						<Button
							variant="subtle"
							leftSection={<IconArrowLeft size={16} />}
							onClick={() => navigate(`/series/${book.seriesId}`)}
						>
							Back to Series
						</Button>
					)}

					{nextBook && (
						<Tooltip label={`Next: ${nextBook.title}`} position="top">
							<Button
								variant="subtle"
								rightSection={<IconChevronRight size={16} />}
								onClick={() => navigate(`/books/${nextBook.id}`)}
							>
								Next Book
							</Button>
						</Tooltip>
					)}
				</Group>
			</Stack>
		</Box>
	);
}
