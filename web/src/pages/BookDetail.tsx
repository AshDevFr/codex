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
	Progress,
	Stack,
	Text,
	Title,
	Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
	IconAnalyze,
	IconBook,
	IconBookOff,
	IconCheck,
	IconChevronDown,
	IconChevronLeft,
	IconChevronRight,
	IconChevronUp,
	IconDotsVertical,
	IconDownload,
	IconEdit,
	IconPhoto,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import { BookMetadataEditModal } from "@/components/books/BookMetadataEditModal";

// Language code mapping
const LANGUAGE_DISPLAY: Record<string, string> = {
	en: "English",
	ja: "Japanese",
	ko: "Korean",
	zh: "Chinese",
	fr: "French",
	de: "German",
	es: "Spanish",
	it: "Italian",
	pt: "Portuguese",
	ru: "Russian",
};

function formatFileSize(bytes: number): string {
	if (bytes >= 1073741824) {
		return `${(bytes / 1073741824).toFixed(2)} GB`;
	}
	if (bytes >= 1048576) {
		return `${(bytes / 1048576).toFixed(2)} MB`;
	}
	if (bytes >= 1024) {
		return `${(bytes / 1024).toFixed(2)} KB`;
	}
	return `${bytes} B`;
}

export function BookDetail() {
	const { bookId } = useParams<{ bookId: string }>();
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const [summaryOpened, { toggle: toggleSummary }] = useDisclosure(false);
	const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
		useDisclosure(false);

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

	// Generate thumbnail mutation
	const generateThumbnailMutation = useMutation({
		mutationFn: () => booksApi.generateThumbnail(bookId ?? ""),
		onSuccess: () => {
			notifications.show({
				title: "Thumbnail generation started",
				message: "Book queued for thumbnail generation",
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
	const baseTitle =
		book.number !== undefined && book.number !== null
			? `${book.number} - ${book.title}`
			: book.title;
	const displayTitle = book.deleted ? `(Deleted) ${baseTitle}` : baseTitle;

	// Build breadcrumbs
	const breadcrumbItems = [
		{ title: "Home", href: "/" },
		{ title: book.seriesName, href: `/series/${book.seriesId}` },
		{ title: displayTitle, href: `/books/${book.id}` },
	];

	// Calculate reading progress (current_page is 1-indexed)
	const currentPage = book.readProgress ? book.readProgress.current_page : 0;
	const percentage =
		book.pageCount > 0 ? (currentPage / book.pageCount) * 100 : 0;

	// Extract metadata values
	const languageDisplay = metadata?.languageIso
		? LANGUAGE_DISPLAY[metadata.languageIso] || metadata.languageIso
		: null;
	const releaseYear = metadata?.releaseDate
		? new Date(metadata.releaseDate).getFullYear()
		: null;

	// Collect all creators
	const creators: { role: string; names: string[] }[] = [
		{ role: "WRITERS", names: metadata?.writers || [] },
		{ role: "PENCILLERS", names: metadata?.pencillers || [] },
		{ role: "INKERS", names: metadata?.inkers || [] },
		{ role: "COLORISTS", names: metadata?.colorists || [] },
		{ role: "LETTERERS", names: metadata?.letterers || [] },
		{ role: "COVER ARTISTS", names: metadata?.coverArtists || [] },
		{ role: "EDITORS", names: metadata?.editors || [] },
	].filter((c) => c.names.length > 0);

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
						<Box pos="relative">
							{book.deleted ? (
								<Box
									style={{
										aspectRatio: "150/212.125",
										display: "flex",
										flexDirection: "column",
										alignItems: "center",
										justifyContent: "center",
										backgroundColor: "var(--mantine-color-dark-6)",
										borderRadius: "var(--mantine-radius-sm)",
										border: "2px dashed var(--mantine-color-red-6)",
									}}
								>
									<IconTrash
										size={48}
										style={{
											color: "var(--mantine-color-red-6)",
											opacity: 0.7,
										}}
									/>
									<Text size="sm" fw={500} c="red" mt="xs">
										Deleted
									</Text>
								</Box>
							) : (
								<Image
									src={coverUrl}
									alt={book.title}
									radius="sm"
									fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='150' height='212'%3E%3Crect fill='%23333' width='150' height='212'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='12' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
									style={{ aspectRatio: "150/212.125" }}
								/>
							)}
						</Box>
					</Grid.Col>

					{/* Info */}
					<Grid.Col span={{ base: 8, xs: 9, sm: 10 }}>
						<Stack gap="xs">
							{/* Title row with badges and menu */}
							<Group justify="space-between" align="flex-start" wrap="nowrap">
								<Box style={{ flex: 1, minWidth: 0 }}>
									<Group gap="sm" align="center" wrap="wrap">
										<Title order={2} style={{ wordBreak: "break-word" }}>
											{displayTitle}
										</Title>
									</Group>
									<Group gap="xs" mt={4}>
										{book.deleted && (
											<Badge
												size="sm"
												variant="filled"
												color="red"
												leftSection={<IconTrash size={12} />}
											>
												Deleted
											</Badge>
										)}
										<Badge size="sm" variant="filled">
											{book.fileFormat.toUpperCase()}
										</Badge>
										{isCompleted && (
											<Badge size="sm" variant="filled" color="green">
												Completed
											</Badge>
										)}
										{hasProgress && !isCompleted && (
											<Badge size="sm" variant="outline" color="blue">
												In Progress
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
											Force Analyze
										</Menu.Item>
										<Menu.Item
											leftSection={<IconPhoto size={14} />}
											onClick={() => generateThumbnailMutation.mutate()}
											disabled={generateThumbnailMutation.isPending}
										>
											Generate Thumbnail
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

							{/* Series link */}
							<Text
								component={Link}
								to={`/series/${book.seriesId}`}
								size="sm"
								c="dimmed"
								className="hover-underline"
								style={{ textDecoration: "none", width: "fit-content" }}
							>
								in {book.seriesName}
							</Text>

							{/* Reading progress */}
							{hasProgress && !isCompleted && (
								<Group gap="sm" align="center">
									{book.fileFormat !== "epub" && (
										<Text size="sm">
											Page {currentPage} of {book.pageCount}
										</Text>
									)}
									<Progress
										value={percentage}
										size="sm"
										style={{ flex: 1, maxWidth: 200 }}
									/>
									<Text size="sm" c="dimmed">
										{Math.round(percentage)}%
									</Text>
								</Group>
							)}

							{/* Action buttons */}
							<Group gap="sm" mt="xs">
								<Button
									size="xs"
									variant="filled"
									leftSection={<IconBook size={14} />}
									onClick={() => {
										const page = book.readProgress?.current_page ?? 1;
										navigate(`/reader/${book.id}?page=${page}`);
									}}
								>
									{hasProgress && !isCompleted ? "Continue" : "Read"}
								</Button>
								<Button
									size="xs"
									variant="outline"
									component="a"
									href={downloadUrl}
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

							{/* Analysis error */}
							{book.analysisError && (
								<Box
									p="xs"
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

				{/* Metadata rows - Komga style */}
				<Stack gap="xs">
					{/* File Info */}
					<Group gap="md" align="center">
						<Text size="sm" c="dimmed" w={100}>
							SIZE
						</Text>
						<Text size="sm">{formatFileSize(book.fileSize)}</Text>
					</Group>

					<Group gap="md" align="center">
						<Text size="sm" c="dimmed" w={100}>
							PAGES
						</Text>
						<Text size="sm">{book.pageCount}</Text>
					</Group>

					{/* Publisher */}
					{metadata?.publisher && (
						<Group gap="md" align="center">
							<Text size="sm" c="dimmed" w={100}>
								PUBLISHER
							</Text>
							<Badge variant="outline" size="sm">
								{metadata.publisher}
							</Badge>
						</Group>
					)}

					{/* Imprint */}
					{metadata?.imprint && (
						<Group gap="md" align="center">
							<Text size="sm" c="dimmed" w={100}>
								IMPRINT
							</Text>
							<Badge variant="outline" size="sm">
								{metadata.imprint}
							</Badge>
						</Group>
					)}

					{/* Release Year */}
					{releaseYear && (
						<Group gap="md" align="center">
							<Text size="sm" c="dimmed" w={100}>
								YEAR
							</Text>
							<Text size="sm">{releaseYear}</Text>
						</Group>
					)}

					{/* Language */}
					{languageDisplay && (
						<Group gap="md" align="center">
							<Text size="sm" c="dimmed" w={100}>
								LANGUAGE
							</Text>
							<Text size="sm">{languageDisplay}</Text>
						</Group>
					)}

					{/* Genre */}
					{metadata?.genre && (
						<Group gap="md" align="center">
							<Text size="sm" c="dimmed" w={100}>
								GENRE
							</Text>
							<Badge variant="light" size="sm">
								{metadata.genre}
							</Badge>
						</Group>
					)}

					{/* Creators */}
					{creators.map(({ role, names }) => (
						<Group key={role} gap="md" align="flex-start">
							<Text size="sm" c="dimmed" w={100}>
								{role}
							</Text>
							<Group gap="xs">
								{names.map((name) => (
									<Badge key={`${role}-${name}`} variant="light" size="sm">
										{name}
									</Badge>
								))}
							</Group>
						</Group>
					))}

					{/* File Path */}
					<Group gap="md" align="center">
						<Text size="sm" c="dimmed" w={100}>
							FILE
						</Text>
						<Tooltip label={book.filePath} position="top" multiline maw={400}>
							<Text size="sm" style={{ cursor: "help" }}>
								{book.filePath.split("/").pop() || book.filePath}
							</Text>
						</Tooltip>
					</Group>

					{/* Hash */}
					<Group gap="md" align="center">
						<Text size="sm" c="dimmed" w={100}>
							HASH
						</Text>
						<Tooltip label={book.fileHash} position="top">
							<Text size="sm" style={{ cursor: "help" }}>
								{book.fileHash.substring(0, 16)}...
							</Text>
						</Tooltip>
					</Group>
				</Stack>

				{/* Series navigation */}
				<Group justify="space-between" mt="md">
					{prevBook ? (
						<Tooltip label={prevBook.title} position="top">
							<Button
								component={Link}
								to={`/books/${prevBook.id}`}
								variant="subtle"
								size="xs"
								leftSection={<IconChevronLeft size={14} />}
							>
								Previous
							</Button>
						</Tooltip>
					) : (
						<Button
							component={Link}
							to={`/series/${book.seriesId}`}
							variant="subtle"
							size="xs"
							leftSection={<IconChevronLeft size={14} />}
						>
							Back to Series
						</Button>
					)}

					{nextBook && (
						<Tooltip label={nextBook.title} position="top">
							<Button
								component={Link}
								to={`/books/${nextBook.id}`}
								variant="subtle"
								size="xs"
								rightSection={<IconChevronRight size={14} />}
							>
								Next
							</Button>
						</Tooltip>
					)}
				</Group>
			</Stack>

			{/* Edit Metadata Modal */}
			<BookMetadataEditModal
				opened={editModalOpened}
				onClose={closeEditModal}
				bookId={book.id}
				bookTitle={book.title}
			/>
		</Box>
	);
}
