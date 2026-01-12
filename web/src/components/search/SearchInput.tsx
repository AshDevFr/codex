import {
	Combobox,
	Group,
	Image,
	Loader,
	ScrollArea,
	Stack,
	Text,
	TextInput,
	useCombobox,
} from "@mantine/core";
import { IconSearch } from "@tabler/icons-react";
import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSearch } from "@/hooks/useSearch";
import type { Book, Series } from "@/types";
import classes from "./SearchInput.module.css";

interface SearchInputProps {
	placeholder?: string;
	width?: number;
}

export function SearchInput({
	placeholder = "Search...",
	width = 300,
}: SearchInputProps) {
	const navigate = useNavigate();
	const combobox = useCombobox({
		onDropdownClose: () => combobox.resetSelectedOption(),
	});

	const [query, setQuery] = useState("");
	const { results, isLoading } = useSearch(query);

	// Defensive checks for undefined results
	const series = results?.series ?? [];
	const books = results?.books ?? [];
	const hasResults = series.length > 0 || books.length > 0;
	const showDropdown = query.trim().length >= 2;

	const handleInputChange = useCallback(
		(event: React.ChangeEvent<HTMLInputElement>) => {
			const value = event.currentTarget.value;
			setQuery(value);
			if (value.trim().length >= 2) {
				combobox.openDropdown();
			} else {
				combobox.closeDropdown();
			}
		},
		[combobox],
	);

	const handleKeyDown = useCallback(
		(event: React.KeyboardEvent<HTMLInputElement>) => {
			if (event.key === "Enter" && query.trim().length >= 2) {
				event.preventDefault();
				combobox.closeDropdown();
				navigate(`/search?q=${encodeURIComponent(query.trim())}`);
			}
		},
		[query, navigate, combobox],
	);

	const handleSeriesSelect = useCallback(
		(series: Series) => {
			combobox.closeDropdown();
			setQuery("");
			navigate(`/series/${series.id}`);
		},
		[navigate, combobox],
	);

	const handleBookSelect = useCallback(
		(book: Book) => {
			combobox.closeDropdown();
			setQuery("");
			navigate(`/books/${book.id}`);
		},
		[navigate, combobox],
	);

	const renderSeriesOption = (series: Series) => (
		<Combobox.Option
			value={`series-${series.id}`}
			key={series.id}
			className={classes.option}
			onClick={() => handleSeriesSelect(series)}
		>
			<Group gap="sm" wrap="nowrap">
				<Image
					src={`/api/v1/series/${series.id}/thumbnail`}
					alt={series.name}
					w={40}
					h={56}
					fit="cover"
					radius="sm"
					fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='56'%3E%3Crect fill='%23333' width='40' height='56'/%3E%3C/svg%3E"
				/>
				<Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
					<Text size="sm" fw={500} lineClamp={1}>
						{series.name}
					</Text>
					<Text size="xs" c="dimmed">
						{series.bookCount} book{series.bookCount !== 1 ? "s" : ""}
					</Text>
				</Stack>
			</Group>
		</Combobox.Option>
	);

	const renderBookOption = (book: Book) => (
		<Combobox.Option
			value={`book-${book.id}`}
			key={book.id}
			className={classes.option}
			onClick={() => handleBookSelect(book)}
		>
			<Group gap="sm" wrap="nowrap">
				<Image
					src={`/api/v1/books/${book.id}/thumbnail`}
					alt={book.title}
					w={40}
					h={56}
					fit="cover"
					radius="sm"
					fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='56'%3E%3Crect fill='%23333' width='40' height='56'/%3E%3C/svg%3E"
				/>
				<Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
					<Text size="sm" fw={500} lineClamp={1}>
						{book.number !== undefined && book.number !== null
							? `${book.number} - ${book.title}`
							: book.title}
					</Text>
					{book.seriesName && (
						<Text size="xs" c="dimmed" lineClamp={1}>
							{book.seriesName}
						</Text>
					)}
				</Stack>
			</Group>
		</Combobox.Option>
	);

	return (
		<Combobox store={combobox} withinPortal={false}>
			<Combobox.Target>
				<TextInput
					placeholder={placeholder}
					leftSection={
						isLoading ? <Loader size={16} /> : <IconSearch size={16} />
					}
					value={query}
					onChange={handleInputChange}
					onKeyDown={handleKeyDown}
					onFocus={() => {
						if (query.trim().length >= 2) {
							combobox.openDropdown();
						}
					}}
					onBlur={() => combobox.closeDropdown()}
					visibleFrom="sm"
					w={width}
				/>
			</Combobox.Target>

			{showDropdown && (
				<Combobox.Dropdown className={classes.dropdown}>
					<ScrollArea.Autosize mah={400} type="scroll">
						{isLoading ? (
							<Combobox.Empty>
								<Group justify="center" p="md">
									<Loader size="sm" />
									<Text size="sm" c="dimmed">
										Searching...
									</Text>
								</Group>
							</Combobox.Empty>
						) : !hasResults ? (
							<Combobox.Empty>No results found</Combobox.Empty>
						) : (
							<>
								{series.length > 0 && (
									<Combobox.Group label="Series">
										{series.slice(0, 5).map(renderSeriesOption)}
									</Combobox.Group>
								)}
								{books.length > 0 && (
									<Combobox.Group label="Books">
										{books.slice(0, 5).map(renderBookOption)}
									</Combobox.Group>
								)}
								{(series.length > 5 || books.length > 5) && (
									<Combobox.Footer className={classes.footer}>
										<Text
											size="xs"
											c="dimmed"
											ta="center"
											style={{ cursor: "pointer" }}
											onClick={() => {
												combobox.closeDropdown();
												navigate(
													`/search?q=${encodeURIComponent(query.trim())}`,
												);
											}}
										>
											Press Enter to see all results
										</Text>
									</Combobox.Footer>
								)}
							</>
						)}
					</ScrollArea.Autosize>
				</Combobox.Dropdown>
			)}
		</Combobox>
	);
}
