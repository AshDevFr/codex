import {
	ActionIcon,
	Card,
	Group,
	Image,
	Menu,
	Stack,
	Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAnalyze, IconDotsVertical } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { useAuthenticatedImage } from "@/hooks/useAuthenticatedImage";
import type { Book, Series } from "@/types/api";

interface MediaCardProps {
	type: "book" | "series";
	data: Book | Series;
	showProgress?: boolean;
}

export function MediaCard({ type, data, showProgress }: MediaCardProps) {
	const queryClient = useQueryClient();
	const coverUrl =
		type === "book"
			? `/books/${(data as Book).id}/thumbnail`
			: `/series/${(data as Series).id}/thumbnail`;
	const authenticatedImageUrl = useAuthenticatedImage(coverUrl);

	const book = type === "book" ? (data as Book) : null;
	const series = type === "series" ? (data as Series) : null;

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

	const title = book
		? `${book.number !== undefined && book.number !== null ? `${book.number} - ` : ""}${book.title}`
		: series?.name || "";
	const altText = book ? book.title : series?.name || "";

	return (
		<Card
			shadow="sm"
			padding={0}
			radius="md"
			withBorder
			style={{
				height: "100%",
				display: "flex",
				flexDirection: "column",
				minHeight: 0,
				width: "100%", // Ensure full width of grid cell
				boxSizing: "border-box", // Include border in width calculation
			}}
		>
			<Stack gap={0} style={{ height: "100%", minHeight: 0 }}>
				{/* Cover Image - Fixed height section (Komga ratio: 150px width, 212.125px height = 1.414) */}
				<div
					style={{
						position: "relative",
						width: "100%",
						aspectRatio: "150/212.125",
						flexShrink: 0,
						overflow: "hidden",
					}}
				>
					<Image
						src={authenticatedImageUrl || undefined}
						alt={altText}
						fit="cover"
						style={{ width: "100%", height: "100%", objectFit: "cover" }}
						fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='300'%3E%3Crect fill='%23ddd' width='200' height='300'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='14' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
					/>
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
						style={{
							position: "absolute",
							bottom: 8,
							right: 8,
							zIndex: 3,
						}}
					>
						<Menu position="top-end" shadow="md" withinPortal>
							<Menu.Target>
								<ActionIcon variant="filled" color="dark" size="sm" style={{ opacity: 0.8 }}>
									<IconDotsVertical size={16} />
								</ActionIcon>
							</Menu.Target>
							<Menu.Dropdown>
								{type === "book" ? (
									<Menu.Item
										leftSection={<IconAnalyze size={14} />}
										onClick={() => bookAnalyzeMutation.mutate()}
										disabled={bookAnalyzeMutation.isPending}
									>
										{bookAnalyzeMutation.isPending ? "Analyzing..." : "Analyze"}
									</Menu.Item>
								) : (
									<>
										<Menu.Item
											leftSection={<IconAnalyze size={14} />}
											onClick={() => seriesAnalyzeMutation.mutate()}
											disabled={seriesAnalyzeMutation.isPending}
										>
											{seriesAnalyzeMutation.isPending ? "Analyzing..." : "Analyze All"}
										</Menu.Item>
										<Menu.Item
											leftSection={<IconAnalyze size={14} />}
											onClick={() => seriesAnalyzeUnanalyzedMutation.mutate()}
											disabled={seriesAnalyzeUnanalyzedMutation.isPending}
										>
											{seriesAnalyzeUnanalyzedMutation.isPending
												? "Analyzing..."
												: "Analyze Unanalyzed"}
										</Menu.Item>
									</>
								)}
							</Menu.Dropdown>
						</Menu>
					</div>
				</div>
				{/* Card Content - Fixed height section (Komga: 94px = 5.875rem at 16px base) */}
				<Stack gap={4} p="sm" style={{ flexShrink: 0, height: "5.875rem", minHeight: "5.875rem", overflow: "visible" }}>
					{type === "book" && book?.seriesName && book.seriesName.trim() !== "" && book.seriesName.trim() !== "-" && (
						<Text
							fw={500}
							lineClamp={1}
							c="dimmed"
							size="xs"
							style={{
								overflow: "hidden",
								textOverflow: "ellipsis",
								whiteSpace: "nowrap",
								display: "block",
							}}
						>
							{book.seriesName}
						</Text>
					)}
					<Text fw={600} lineClamp={1} size="sm" style={{ overflow: "hidden" }}>
						{title}
					</Text>
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
					{showProgress && type === "book" && (
						<Text size="xs" c="blue">
							Continue reading
						</Text>
					)}
				</Stack>
			</Stack>
		</Card>
	);
}
