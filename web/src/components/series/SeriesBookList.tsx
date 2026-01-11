import {
	Box,
	Center,
	Group,
	Loader,
	Menu,
	Pagination,
	Select,
	SimpleGrid,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { IconSortAscending } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import { MediaCard } from "@/components/library/MediaCard";

interface SeriesBookListProps {
	seriesId: string;
	seriesName: string;
	bookCount: number;
}

type SortOption = {
	value: string;
	label: string;
};

const SORT_OPTIONS: SortOption[] = [
	{ value: "number,asc", label: "Number (Ascending)" },
	{ value: "number,desc", label: "Number (Descending)" },
	{ value: "title,asc", label: "Title (A-Z)" },
	{ value: "title,desc", label: "Title (Z-A)" },
	{ value: "created_at,desc", label: "Recently Added" },
	{ value: "release_date,desc", label: "Release Date (Newest)" },
	{ value: "release_date,asc", label: "Release Date (Oldest)" },
];

const PAGE_SIZES = [20, 50, 100];

export function SeriesBookList({
	seriesId,
	seriesName,
	bookCount,
}: SeriesBookListProps) {
	const navigate = useNavigate();
	const [page, setPage] = useState(1);
	const [pageSize, setPageSize] = useState(20);
	const [sort, setSort] = useState("number,asc");

	// Fetch books for this series
	const { data, isLoading, error } = useQuery({
		queryKey: ["series-books", seriesId, page, pageSize, sort],
		queryFn: () =>
			booksApi.getByLibrary("all", {
				series_id: seriesId,
				page,
				pageSize,
				sort,
			}),
	});

	const totalPages = data ? Math.ceil(data.total / pageSize) : 1;

	const handleBookClick = (bookId: string) => {
		navigate(`/books/${bookId}`);
	};

	const currentSortLabel =
		SORT_OPTIONS.find((opt) => opt.value === sort)?.label || "Sort";

	if (error) {
		return (
			<Stack gap="md">
				<Title order={4}>Books ({bookCount})</Title>
				<Text c="red">Failed to load books</Text>
			</Stack>
		);
	}

	return (
		<Stack gap="md">
			<Group justify="space-between" align="center">
				<Title order={4}>Books ({bookCount})</Title>

				<Group gap="sm">
					<Menu shadow="md" width={200}>
						<Menu.Target>
							<Box
								style={{ cursor: "pointer" }}
								component="button"
								bg="transparent"
								bd="none"
							>
								<Group gap="xs">
									<IconSortAscending size={16} />
									<Text size="sm">{currentSortLabel}</Text>
								</Group>
							</Box>
						</Menu.Target>
						<Menu.Dropdown>
							{SORT_OPTIONS.map((option) => (
								<Menu.Item
									key={option.value}
									onClick={() => {
										setSort(option.value);
										setPage(1);
									}}
									style={{
										fontWeight: sort === option.value ? 600 : 400,
									}}
								>
									{option.label}
								</Menu.Item>
							))}
						</Menu.Dropdown>
					</Menu>

					<Select
						size="xs"
						w={80}
						value={pageSize.toString()}
						onChange={(value) => {
							if (value) {
								setPageSize(parseInt(value, 10));
								setPage(1);
							}
						}}
						data={PAGE_SIZES.map((size) => ({
							value: size.toString(),
							label: size.toString(),
						}))}
					/>
				</Group>
			</Group>

			{isLoading ? (
				<Center py="xl">
					<Loader size="lg" />
				</Center>
			) : data?.data.length === 0 ? (
				<Text c="dimmed">No books in this series</Text>
			) : (
				<>
					<SimpleGrid
						cols={{ base: 2, xs: 3, sm: 4, md: 5, lg: 6, xl: 7 }}
						spacing="md"
						style={{
							gridTemplateColumns:
								"repeat(auto-fill, minmax(150px, 1fr))",
						}}
					>
						{data?.data.map((book) => (
							<Box
								key={book.id}
								onClick={() => handleBookClick(book.id)}
								style={{ cursor: "pointer" }}
							>
								<MediaCard type="book" data={book} />
							</Box>
						))}
					</SimpleGrid>

					{totalPages > 1 && (
						<Center mt="md">
							<Pagination
								value={page}
								onChange={setPage}
								total={totalPages}
								siblings={1}
								boundaries={1}
							/>
						</Center>
					)}
				</>
			)}
		</Stack>
	);
}
