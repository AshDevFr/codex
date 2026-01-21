import {
	ActionIcon,
	Card,
	Group,
	Image,
	Menu,
	Progress,
	Stack,
	Text,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAnalyze,
	IconBook,
	IconBookOff,
	IconCheck,
	IconDotsVertical,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { AppLink } from "@/components/common";
import { usePermissions } from "@/hooks/usePermissions";
import type { Book, Series } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

interface MediaCardProps {
	type: "book" | "series";
	data: Book | Series;
	hideSeriesName?: boolean;
}

export function MediaCard({
	type,
	data,
	hideSeriesName = false,
}: MediaCardProps) {
	const queryClient = useQueryClient();
	const navigate = useNavigate();
	const { hasPermission } = usePermissions();
	const canWriteBooks = hasPermission(PERMISSIONS.BOOKS_WRITE);
	const canWriteSeries = hasPermission(PERMISSIONS.SERIES_WRITE);

	// Handle card click navigation
	const handleCardClick = (e: React.MouseEvent) => {
		// Don't navigate if clicking the menu button or dropdown
		if ((e.target as HTMLElement).closest("[data-menu]")) return;

		if (type === "series") {
			navigate(`/series/${(data as Series).id}`);
		} else {
			navigate(`/books/${(data as Book).id}`);
		}
	};

	// Use API endpoint directly - browser will send auth cookie automatically
	// Include updatedAt as cache-busting parameter so images refresh when thumbnails are generated
	const coverUrl =
		type === "book"
			? `/api/v1/books/${(data as Book).id}/thumbnail?v=${encodeURIComponent(data.updatedAt)}`
			: `/api/v1/series/${(data as Series).id}/thumbnail?v=${encodeURIComponent(data.updatedAt)}`;

	const book = type === "book" ? (data as Book) : null;
	const series = type === "series" ? (data as Series) : null;

	// Track if item is newly created (for animation)
	const [isNew, setIsNew] = useState(false);

	useEffect(() => {
		// Check if item was created recently (within last 5 seconds)
		const createdAt = new Date(data.createdAt);
		const now = new Date();
		const diffMs = now.getTime() - createdAt.getTime();

		if (diffMs < 5000) {
			setIsNew(true);
			// Remove animation after 3 seconds
			const timer = setTimeout(() => setIsNew(false), 3000);
			return () => clearTimeout(timer);
		}
	}, [data.createdAt]);

	// Handle read button click - navigate directly to reader
	const handleReadClick = (e: React.MouseEvent) => {
		e.stopPropagation();
		if (type === "book" && book) {
			// Start from current page if there's progress, otherwise page 1
			const page = book.readProgress?.current_page || 1;
			navigate(`/reader/${book.id}?page=${page}`);
		}
	};

	// Calculate progress percentage for books
	const progressPercentage =
		book?.readProgress && book.pageCount
			? (book.readProgress.current_page / book.pageCount) * 100
			: 0;

	// Book analysis mutation
	const bookAnalyzeMutation = useMutation({
		mutationFn: () => {
			if (!book) throw new Error("Book not available");
			return booksApi.analyze(book.id);
		},
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Book analysis has been queued",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["books"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start book analysis",
				color: "red",
			});
		},
	});

	// Series analysis mutations
	const seriesAnalyzeMutation = useMutation({
		mutationFn: () => {
			if (!series) throw new Error("Series not available");
			return seriesApi.analyze(series.id);
		},
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "All books in series queued for analysis",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start series analysis",
				color: "red",
			});
		},
	});

	const seriesAnalyzeUnanalyzedMutation = useMutation({
		mutationFn: () => {
			if (!series) throw new Error("Series not available");
			return seriesApi.analyzeUnanalyzed(series.id);
		},
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Unanalyzed books queued for analysis",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start analysis",
				color: "red",
			});
		},
	});

	// Book mark as read/unread mutations
	const bookMarkAsReadMutation = useMutation({
		mutationFn: () => {
			if (!book) throw new Error("Book not available");
			return booksApi.markAsRead(book.id);
		},
		onSuccess: () => {
			notifications.show({
				title: "Marked as read",
				message: "Book marked as read",
				color: "green",
			});
			// Refetch all book and series related queries to update UI
			queryClient.refetchQueries({
				predicate: (query) => {
					const key = query.queryKey[0] as string;
					return (
						key === "books" ||
						key === "series" ||
						key === "series-books" ||
						key === "book-detail"
					);
				},
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to mark as read",
				message: error.message || "Failed to mark book as read",
				color: "red",
			});
		},
	});

	const bookMarkAsUnreadMutation = useMutation({
		mutationFn: () => {
			if (!book) throw new Error("Book not available");
			return booksApi.markAsUnread(book.id);
		},
		onSuccess: () => {
			notifications.show({
				title: "Marked as unread",
				message: "Book marked as unread",
				color: "blue",
			});
			// Refetch all book and series related queries to update UI
			queryClient.refetchQueries({
				predicate: (query) => {
					const key = query.queryKey[0] as string;
					return (
						key === "books" ||
						key === "series" ||
						key === "series-books" ||
						key === "book-detail"
					);
				},
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to mark as unread",
				message: error.message || "Failed to mark book as unread",
				color: "red",
			});
		},
	});

	// Series mark as read/unread mutations
	const seriesMarkAsReadMutation = useMutation({
		mutationFn: () => {
			if (!series) throw new Error("Series not available");
			return seriesApi.markAsRead(series.id);
		},
		onSuccess: (data) => {
			notifications.show({
				title: "Marked as read",
				message: data.message,
				color: "green",
			});
			// Refetch all book and series related queries to update UI
			queryClient.refetchQueries({
				predicate: (query) => {
					const key = query.queryKey[0] as string;
					return (
						key === "books" ||
						key === "series" ||
						key === "series-books" ||
						key === "book-detail"
					);
				},
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to mark as read",
				message: error.message || "Failed to mark series as read",
				color: "red",
			});
		},
	});

	const seriesMarkAsUnreadMutation = useMutation({
		mutationFn: () => {
			if (!series) throw new Error("Series not available");
			return seriesApi.markAsUnread(series.id);
		},
		onSuccess: (data) => {
			notifications.show({
				title: "Marked as unread",
				message: data.message,
				color: "blue",
			});
			// Refetch all book and series related queries to update UI
			queryClient.refetchQueries({
				predicate: (query) => {
					const key = query.queryKey[0] as string;
					return (
						key === "books" ||
						key === "series" ||
						key === "series-books" ||
						key === "book-detail"
					);
				},
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to mark as unread",
				message: error.message || "Failed to mark series as unread",
				color: "red",
			});
		},
	});

	const title = book
		? `${book.number !== undefined && book.number !== null ? `${book.number} - ` : ""}${book.title}`
		: series?.title || "";
	const altText = book ? book.title : series?.title || "";

	return (
		<Card
			shadow="sm"
			padding={0}
			radius="md"
			withBorder
			onClick={handleCardClick}
			style={{
				height: "100%",
				display: "flex",
				flexDirection: "column",
				minHeight: 0,
				width: "100%", // Ensure full width of grid cell
				boxSizing: "border-box", // Include border in width calculation
				animation: isNew ? "fadeIn 0.5s ease-in" : undefined,
				border: isNew ? "2px solid var(--mantine-color-blue-6)" : undefined,
				cursor: "pointer",
			}}
		>
			<Stack gap={0} style={{ height: "100%", minHeight: 0 }}>
				{/* Cover Image - Fixed height section (Komga ratio: 150px width, 212.125px height = 1.414) */}
				<div
					className="media-card-cover"
					style={{
						position: "relative",
						width: "100%",
						aspectRatio: "150/212.125",
						flexShrink: 0,
						overflow: "hidden",
					}}
				>
					{book?.deleted ? (
						<div
							style={{
								width: "100%",
								height: "100%",
								backgroundColor: "var(--mantine-color-dark-6)",
								display: "flex",
								flexDirection: "column",
								alignItems: "center",
								justifyContent: "center",
								gap: "8px",
							}}
						>
							<IconTrash
								size={48}
								style={{ color: "var(--mantine-color-red-6)", opacity: 0.7 }}
							/>
							<Text size="sm" fw={500} c="dimmed">
								Deleted
							</Text>
						</div>
					) : (
						<Image
							src={coverUrl}
							alt={altText}
							fit="cover"
							style={{ width: "100%", height: "100%", objectFit: "cover" }}
							fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='300'%3E%3Crect fill='%23ddd' width='200' height='300'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='14' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
						/>
					)}
					{/* Unread indicator - Triangle for books, Square for series */}
					{type === "book" && book && !book.readProgress && (
						<div
							style={{
								position: "absolute",
								top: 0,
								right: 0,
								width: 0,
								height: 0,
								borderTop: "24px solid #ff6b35",
								borderLeft: "24px solid transparent",
								zIndex: 2,
							}}
						/>
					)}
					{type === "series" && series && (series.unreadCount ?? 0) > 0 && (
						<div
							style={{
								position: "absolute",
								top: 0,
								right: 0,
								width: "28px",
								height: "28px",
								backgroundColor: "#ff6b35",
								display: "flex",
								alignItems: "center",
								justifyContent: "center",
								zIndex: 2,
								borderBottomLeftRadius: "4px",
							}}
						>
							<Text
								size="xs"
								fw={700}
								c="white"
								style={{
									fontSize: "12px",
									lineHeight: 1,
								}}
							>
								{(series.unreadCount ?? 0) > 99 ? "99+" : series.unreadCount}
							</Text>
						</div>
					)}
					{/* Menu overlay */}
					<div
						data-menu
						style={{
							position: "absolute",
							bottom: 8,
							right: 8,
							zIndex: 3,
						}}
					>
						<Menu position="top-end" shadow="md" withinPortal>
							<Menu.Target>
								<ActionIcon
									variant="filled"
									color="dark"
									size="sm"
									style={{ opacity: 0.8 }}
									onClick={(e: React.MouseEvent) => e.stopPropagation()}
								>
									<IconDotsVertical size={16} />
								</ActionIcon>
							</Menu.Target>
							<Menu.Dropdown>
								{type === "book" ? (
									<>
										{/* Show Mark as Read if book is unread (no progress or not completed) */}
										{(!book?.readProgress || !book.readProgress.completed) && (
											<Menu.Item
												leftSection={<IconCheck size={14} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													bookMarkAsReadMutation.mutate();
												}}
												disabled={bookMarkAsReadMutation.isPending}
											>
												{bookMarkAsReadMutation.isPending
													? "Marking..."
													: "Mark as Read"}
											</Menu.Item>
										)}
										{/* Show Mark as Unread if book has progress */}
										{book?.readProgress && (
											<Menu.Item
												leftSection={<IconBookOff size={14} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													bookMarkAsUnreadMutation.mutate();
												}}
												disabled={bookMarkAsUnreadMutation.isPending}
											>
												{bookMarkAsUnreadMutation.isPending
													? "Marking..."
													: "Mark as Unread"}
											</Menu.Item>
										)}
										{canWriteBooks && (
											<Menu.Item
												leftSection={<IconAnalyze size={14} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													bookAnalyzeMutation.mutate();
												}}
												disabled={bookAnalyzeMutation.isPending}
											>
												{bookAnalyzeMutation.isPending
													? "Analyzing..."
													: "Force Analyze"}
											</Menu.Item>
										)}
									</>
								) : (
									<>
										{/* Show Mark as Read if series has any unread books */}
										{series && (series.unreadCount ?? 0) > 0 && (
											<Menu.Item
												leftSection={<IconCheck size={14} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													seriesMarkAsReadMutation.mutate();
												}}
												disabled={seriesMarkAsReadMutation.isPending}
											>
												{seriesMarkAsReadMutation.isPending
													? "Marking..."
													: "Mark as Read"}
											</Menu.Item>
										)}
										{/* Show Mark as Unread if series has any read books */}
										{series &&
											(series.bookCount ?? 0) > (series.unreadCount ?? 0) && (
												<Menu.Item
													leftSection={<IconBookOff size={14} />}
													onClick={(e: React.MouseEvent) => {
														e.stopPropagation();
														seriesMarkAsUnreadMutation.mutate();
													}}
													disabled={seriesMarkAsUnreadMutation.isPending}
												>
													{seriesMarkAsUnreadMutation.isPending
														? "Marking..."
														: "Mark as Unread"}
												</Menu.Item>
											)}
										{canWriteSeries && (
											<>
												<Menu.Item
													leftSection={<IconAnalyze size={14} />}
													onClick={(e: React.MouseEvent) => {
														e.stopPropagation();
														seriesAnalyzeMutation.mutate();
													}}
													disabled={seriesAnalyzeMutation.isPending}
												>
													{seriesAnalyzeMutation.isPending
														? "Analyzing..."
														: "Force Analyze All"}
												</Menu.Item>
												<Menu.Item
													leftSection={<IconAnalyze size={14} />}
													onClick={(e: React.MouseEvent) => {
														e.stopPropagation();
														seriesAnalyzeUnanalyzedMutation.mutate();
													}}
													disabled={seriesAnalyzeUnanalyzedMutation.isPending}
												>
													{seriesAnalyzeUnanalyzedMutation.isPending
														? "Analyzing..."
														: "Analyze Unanalyzed"}
												</Menu.Item>
											</>
										)}
									</>
								)}
							</Menu.Dropdown>
						</Menu>
					</div>
					{/* Read button overlay - shows on hover for books only */}
					{type === "book" && !book?.deleted && (
						<div
							className="media-card-read-overlay"
							style={{
								position: "absolute",
								top: 0,
								left: 0,
								right: 0,
								bottom: 0,
								display: "flex",
								alignItems: "center",
								justifyContent: "center",
								backgroundColor: "rgba(0, 0, 0, 0.5)",
								transition: "opacity 0.2s ease",
								zIndex: 2,
							}}
						>
							<ActionIcon
								variant="filled"
								color="red"
								size={56}
								radius="xl"
								onClick={handleReadClick}
								aria-label="Read book"
							>
								<IconBook size={28} />
							</ActionIcon>
						</div>
					)}
					{/* Progress bar - shows at bottom of cover for books with progress */}
					{type === "book" &&
						book?.readProgress &&
						!book.readProgress.completed &&
						progressPercentage > 0 && (
							<Progress
								value={progressPercentage}
								size="sm"
								color="red"
								style={{
									position: "absolute",
									bottom: 0,
									left: 0,
									right: 0,
									zIndex: 4,
									borderRadius: 0,
								}}
							/>
						)}
				</div>
				{/* Card Content - Fixed height section (Komga: 94px = 5.875rem at 16px base) */}
				<Stack
					gap={4}
					p="sm"
					style={{
						flexShrink: 0,
						height: "5.875rem",
						minHeight: "5.875rem",
						overflow: "visible",
					}}
				>
					{!hideSeriesName &&
						type === "book" &&
						book?.seriesName &&
						book.seriesName.trim() !== "" &&
						book.seriesName.trim() !== "-" && (
							<Tooltip
								label={book.seriesName}
								openDelay={500}
								multiline
								maw={300}
							>
								<AppLink
									to={`/series/${book.seriesId}`}
									stopPropagation
									style={{
										overflow: "hidden",
										textOverflow: "ellipsis",
										whiteSpace: "nowrap",
										display: "block",
									}}
									className="hover-underline"
								>
									<Text fw={500} lineClamp={1} c="dimmed" size="xs">
										{book.seriesName}
									</Text>
								</AppLink>
							</Tooltip>
						)}
					<Tooltip label={title} openDelay={500} multiline maw={300}>
						<div style={{ minWidth: 0, width: "100%" }}>
							<AppLink
								to={
									type === "series"
										? `/series/${(data as Series).id}`
										: `/books/${(data as Book).id}`
								}
								stopPropagation
								className="hover-underline"
							>
								<Text
									fw={600}
									size="sm"
									style={{
										display: "-webkit-box",
										WebkitLineClamp: hideSeriesName ? 2 : 1,
										WebkitBoxOrient: "vertical",
										overflow: "hidden",
										wordBreak: "break-all",
									}}
								>
									{title}
								</Text>
							</AppLink>
						</div>
					</Tooltip>
					<Group gap="xs" mt="auto" style={{ flexShrink: 0 }}>
						{book && (
							<>
								{book.pageCount && (
									<Text size="xs" c="dimmed">
										{book.pageCount} pages
									</Text>
								)}
								<Text size="xs" c="dimmed">
									{book.fileFormat.toUpperCase()}
								</Text>
							</>
						)}
						{series && (
							<>
								{series.bookCount !== undefined && (
									<Text size="xs" c="dimmed">
										{series.bookCount} book{series.bookCount !== 1 ? "s" : ""}
									</Text>
								)}
								{series.year && (
									<Text size="xs" c="dimmed">
										{series.year}
									</Text>
								)}
							</>
						)}
					</Group>
				</Stack>
			</Stack>
		</Card>
	);
}
