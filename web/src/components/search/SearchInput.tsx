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
import {
	forwardRef,
	useCallback,
	useImperativeHandle,
	useMemo,
	useRef,
	useState,
} from "react";
import { useNavigate } from "react-router-dom";
import { useSearch } from "@/hooks/useSearch";
import type { Book, Series } from "@/types";
import classes from "./SearchInput.module.css";

interface SearchInputProps {
	placeholder?: string;
	width?: number;
}

export interface SearchInputHandle {
	focus: () => void;
}

export const SearchInput = forwardRef<SearchInputHandle, SearchInputProps>(
	function SearchInput({ placeholder = "Search...", width = 300 }, ref) {
		const inputRef = useRef<HTMLInputElement>(null);

		useImperativeHandle(ref, () => ({
			focus: () => {
				inputRef.current?.focus();
			},
		}));
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

		// Create a map of option values to their navigation targets
		const optionMap = useMemo(() => {
			const map = new Map<string, { type: "series" | "book"; id: string }>();
			for (const s of series.slice(0, 5)) {
				map.set(`series-${s.id}`, { type: "series", id: s.id });
			}
			for (const b of books.slice(0, 5)) {
				map.set(`book-${b.id}`, { type: "book", id: b.id });
			}
			return map;
		}, [series, books]);

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
				// Handle Enter key - navigate to selected option or search page
				if (event.key === "Enter" && query.trim().length >= 2) {
					event.preventDefault();
					const selectedOption = combobox.getSelectedOptionIndex();

					// If an option is selected, trigger submission
					if (selectedOption !== -1) {
						combobox.clickSelectedOption();
						return;
					}

					// No option selected, go to search results page
					combobox.closeDropdown();
					navigate(`/search?q=${encodeURIComponent(query.trim())}`);
				}

				// Handle Escape key
				if (event.key === "Escape") {
					combobox.closeDropdown();
				}
			},
			[query, navigate, combobox],
		);

		const handleOptionSubmit = useCallback(
			(value: string) => {
				const target = optionMap.get(value);
				if (target) {
					combobox.closeDropdown();
					setQuery("");
					if (target.type === "series") {
						navigate(`/series/${target.id}`);
					} else {
						navigate(`/books/${target.id}`);
					}
				}
			},
			[navigate, combobox, optionMap],
		);

		const renderSeriesOption = (series: Series) => (
			<Combobox.Option
				value={`series-${series.id}`}
				key={series.id}
				className={classes.option}
			>
				<Group gap="sm" wrap="nowrap">
					<Image
						src={`/api/v1/series/${series.id}/thumbnail`}
						alt={series.title}
						w={40}
						h={56}
						fit="cover"
						radius="sm"
						fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='56'%3E%3Crect fill='%23333' width='40' height='56'/%3E%3C/svg%3E"
					/>
					<Stack gap={2} style={{ flex: 1, minWidth: 0 }}>
						<Text size="sm" fw={500} lineClamp={1}>
							{series.title}
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
			<Combobox
				store={combobox}
				withinPortal={false}
				onOptionSubmit={handleOptionSubmit}
			>
				<Combobox.EventsTarget>
					<TextInput
						ref={inputRef}
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
				</Combobox.EventsTarget>

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
								<Combobox.Options>
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
								</Combobox.Options>
							)}
						</ScrollArea.Autosize>
					</Combobox.Dropdown>
				)}
			</Combobox>
		);
	},
);
