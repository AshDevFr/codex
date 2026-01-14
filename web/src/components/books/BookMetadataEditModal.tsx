import {
	Button,
	Center,
	Group,
	Loader,
	Modal,
	Stack,
	Switch,
	Tabs,
	Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconEdit,
	IconLink,
	IconList,
	IconPhoto,
	IconTag,
	IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import {
	booksApi,
	type BookDetailResponse,
	type BookMetadataLocks,
} from "@/api/books";
import {
	ImageUploader,
	LockableInput,
	LockableSelect,
	LockableTextarea,
	type ImageInfo,
} from "@/components/forms/lockable";

export interface BookMetadataEditModalProps {
	opened: boolean;
	onClose: () => void;
	bookId: string;
	bookTitle?: string;
}

interface FormState {
	// General
	title: string;
	number: string;
	sortNumber: string;
	summary: string;
	releaseYear: string;
	releaseMonth: string;
	releaseDay: string;
	isbn: string;
	volume: string;
	count: string;
	// Authors
	writer: string;
	penciller: string;
	inker: string;
	colorist: string;
	letterer: string;
	coverArtist: string;
	editor: string;
	// Publishing
	publisher: string;
	imprint: string;
	genre: string;
	languageIso: string;
	formatDetail: string;
	// Flags
	blackAndWhite: boolean | null;
	manga: boolean | null;
	// Link
	web: string;
}

interface LocksState {
	summary: boolean;
	writer: boolean;
	penciller: boolean;
	inker: boolean;
	colorist: boolean;
	letterer: boolean;
	coverArtist: boolean;
	editor: boolean;
	publisher: boolean;
	imprint: boolean;
	genre: boolean;
	web: boolean;
	languageIso: boolean;
	formatDetail: boolean;
	blackAndWhite: boolean;
	manga: boolean;
	year: boolean;
	month: boolean;
	day: boolean;
	volume: boolean;
	count: boolean;
	isbns: boolean;
}

function initializeFormState(detail: BookDetailResponse | undefined): FormState {
	const metadata = detail?.metadata;
	return {
		title: detail?.book.title || "",
		number: detail?.book.number?.toString() || "",
		sortNumber: detail?.book.sortNumber?.toString() || "",
		summary: metadata?.summary || "",
		releaseYear: "",
		releaseMonth: "",
		releaseDay: "",
		isbn: "",
		volume: "",
		count: "",
		writer: metadata?.writers?.join(", ") || "",
		penciller: metadata?.pencillers?.join(", ") || "",
		inker: metadata?.inkers?.join(", ") || "",
		colorist: metadata?.colorists?.join(", ") || "",
		letterer: metadata?.letterers?.join(", ") || "",
		coverArtist: metadata?.coverArtists?.join(", ") || "",
		editor: metadata?.editors?.join(", ") || "",
		publisher: metadata?.publisher || "",
		imprint: metadata?.imprint || "",
		genre: metadata?.genre || "",
		languageIso: metadata?.languageIso || "",
		formatDetail: "",
		blackAndWhite: null,
		manga: null,
		web: "",
	};
}

function initializeLocksState(locks: BookMetadataLocks | undefined): LocksState {
	return {
		summary: locks?.summaryLock || false,
		writer: locks?.writerLock || false,
		penciller: locks?.pencillerLock || false,
		inker: locks?.inkerLock || false,
		colorist: locks?.coloristLock || false,
		letterer: locks?.lettererLock || false,
		coverArtist: locks?.coverArtistLock || false,
		editor: locks?.editorLock || false,
		publisher: locks?.publisherLock || false,
		imprint: locks?.imprintLock || false,
		genre: locks?.genreLock || false,
		web: locks?.webLock || false,
		languageIso: locks?.languageIsoLock || false,
		formatDetail: locks?.formatDetailLock || false,
		blackAndWhite: locks?.blackAndWhiteLock || false,
		manga: locks?.mangaLock || false,
		year: locks?.yearLock || false,
		month: locks?.monthLock || false,
		day: locks?.dayLock || false,
		volume: locks?.volumeLock || false,
		count: locks?.countLock || false,
		isbns: locks?.isbnsLock || false,
	};
}

export function BookMetadataEditModal({
	opened,
	onClose,
	bookId,
	bookTitle,
}: BookMetadataEditModalProps) {
	const queryClient = useQueryClient();
	const [activeTab, setActiveTab] = useState<string | null>("general");
	const [formState, setFormState] = useState<FormState>(initializeFormState(undefined));
	const [locksState, setLocksState] = useState<LocksState>(initializeLocksState(undefined));
	const [originalFormState, setOriginalFormState] = useState<FormState | null>(null);
	const [posterImage, setPosterImage] = useState<ImageInfo | null>(null);

	// Fetch book detail
	const { data: bookDetail, isLoading: isLoadingBook } = useQuery({
		queryKey: ["books", bookId, "detail"],
		queryFn: () => booksApi.getDetail(bookId),
		enabled: opened,
	});

	// Fetch locks
	const { data: locks, isLoading: isLoadingLocks } = useQuery({
		queryKey: ["books", bookId, "metadata", "locks"],
		queryFn: () => booksApi.getMetadataLocks(bookId),
		enabled: opened,
	});

	const isLoading = isLoadingBook || isLoadingLocks;

	// Initialize form state when data loads
	useEffect(() => {
		if (bookDetail) {
			const newFormState = initializeFormState(bookDetail);
			setFormState(newFormState);
			setOriginalFormState(newFormState);
		}
	}, [bookDetail]);

	useEffect(() => {
		if (locks) {
			setLocksState(initializeLocksState(locks));
		}
	}, [locks]);

	// Update field helper
	const updateField = useCallback(<K extends keyof FormState>(
		field: K,
		value: FormState[K],
	) => {
		setFormState((prev) => ({ ...prev, [field]: value }));
	}, []);

	// Update lock helper
	const updateLock = useCallback(<K extends keyof LocksState>(
		field: K,
		value: boolean,
	) => {
		setLocksState((prev) => ({ ...prev, [field]: value }));
	}, []);

	// Save mutation
	const saveMutation = useMutation({
		mutationFn: async () => {
			// Update book core fields (title, number) if changed
			const titleChanged = formState.title !== originalFormState?.title;
			const numberChanged = formState.number !== originalFormState?.number;

			if (titleChanged || numberChanged) {
				const patchData: { title?: string | null; number?: number | null } = {};
				if (titleChanged) {
					patchData.title = formState.title || null;
				}
				if (numberChanged) {
					patchData.number = formState.number ? Number.parseFloat(formState.number) : null;
				}
				await booksApi.patch(bookId, patchData);
			}

			// Update metadata
			await booksApi.patchMetadata(bookId, {
				summary: formState.summary || null,
				writer: formState.writer || null,
				penciller: formState.penciller || null,
				inker: formState.inker || null,
				colorist: formState.colorist || null,
				letterer: formState.letterer || null,
				coverArtist: formState.coverArtist || null,
				editor: formState.editor || null,
				publisher: formState.publisher || null,
				imprint: formState.imprint || null,
				genre: formState.genre || null,
				languageIso: formState.languageIso || null,
				formatDetail: formState.formatDetail || null,
				blackAndWhite: formState.blackAndWhite,
				manga: formState.manga,
				year: formState.releaseYear ? Number.parseInt(formState.releaseYear, 10) : null,
				month: formState.releaseMonth ? Number.parseInt(formState.releaseMonth, 10) : null,
				day: formState.releaseDay ? Number.parseInt(formState.releaseDay, 10) : null,
				volume: formState.volume ? Number.parseInt(formState.volume, 10) : null,
				count: formState.count ? Number.parseInt(formState.count, 10) : null,
				isbns: formState.isbn || null,
			});

			// Update locks
			await booksApi.updateMetadataLocks(bookId, {
				summaryLock: locksState.summary,
				writerLock: locksState.writer,
				pencillerLock: locksState.penciller,
				inkerLock: locksState.inker,
				coloristLock: locksState.colorist,
				lettererLock: locksState.letterer,
				coverArtistLock: locksState.coverArtist,
				editorLock: locksState.editor,
				publisherLock: locksState.publisher,
				imprintLock: locksState.imprint,
				genreLock: locksState.genre,
				webLock: locksState.web,
				languageIsoLock: locksState.languageIso,
				formatDetailLock: locksState.formatDetail,
				blackAndWhiteLock: locksState.blackAndWhite,
				mangaLock: locksState.manga,
				yearLock: locksState.year,
				monthLock: locksState.month,
				dayLock: locksState.day,
				volumeLock: locksState.volume,
				countLock: locksState.count,
				isbnsLock: locksState.isbns,
			});

			// Upload cover image if selected
			if (posterImage?.file) {
				await booksApi.uploadCover(bookId, posterImage.file);
			}
		},
		onSuccess: () => {
			notifications.show({
				title: "Success",
				message: "Book metadata updated successfully",
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["books", bookId] });
			onClose();
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update book metadata",
				color: "red",
			});
		},
	});

	const handleSave = () => {
		saveMutation.mutate();
	};

	// General tab
	const renderGeneralTab = () => (
		<Stack gap="md">
			<LockableInput
				label="Title"
				value={formState.title}
				onChange={(v) => updateField("title", v)}
				locked={false}
				onLockChange={() => {}}
				originalValue={originalFormState?.title}
				placeholder="Book title"
				description="Display name for this book"
			/>

			<LockableInput
				label="Number"
				value={formState.number}
				onChange={(v) => updateField("number", v)}
				locked={false}
				onLockChange={() => {}}
				originalValue={originalFormState?.number}
				placeholder="e.g., 1, 2.5, 10"
				description="Book number in series (decimals allowed for sorting)"
			/>

			<LockableTextarea
				label="Summary"
				value={formState.summary}
				onChange={(v) => updateField("summary", v)}
				locked={locksState.summary}
				onLockChange={(v) => updateLock("summary", v)}
				originalValue={originalFormState?.summary}
				placeholder="Enter book summary..."
				minRows={4}
				autosize
			/>

			<Group grow>
				<LockableInput
					label="Year"
					value={formState.releaseYear}
					onChange={(v) => updateField("releaseYear", v)}
					locked={locksState.year}
					onLockChange={(v) => updateLock("year", v)}
					originalValue={originalFormState?.releaseYear}
					placeholder="YYYY"
					type="number"
				/>
				<LockableInput
					label="Month"
					value={formState.releaseMonth}
					onChange={(v) => updateField("releaseMonth", v)}
					locked={locksState.month}
					onLockChange={(v) => updateLock("month", v)}
					originalValue={originalFormState?.releaseMonth}
					placeholder="1-12"
					type="number"
				/>
				<LockableInput
					label="Day"
					value={formState.releaseDay}
					onChange={(v) => updateField("releaseDay", v)}
					locked={locksState.day}
					onLockChange={(v) => updateLock("day", v)}
					originalValue={originalFormState?.releaseDay}
					placeholder="1-31"
					type="number"
				/>
			</Group>

			<Group grow>
				<LockableInput
					label="Volume"
					value={formState.volume}
					onChange={(v) => updateField("volume", v)}
					locked={locksState.volume}
					onLockChange={(v) => updateLock("volume", v)}
					originalValue={originalFormState?.volume}
					placeholder="Volume number"
					type="number"
				/>
				<LockableInput
					label="Count"
					value={formState.count}
					onChange={(v) => updateField("count", v)}
					locked={locksState.count}
					onLockChange={(v) => updateLock("count", v)}
					originalValue={originalFormState?.count}
					placeholder="Total in series"
					type="number"
				/>
			</Group>

			<LockableInput
				label="ISBN"
				value={formState.isbn}
				onChange={(v) => updateField("isbn", v)}
				locked={locksState.isbns}
				onLockChange={(v) => updateLock("isbns", v)}
				originalValue={originalFormState?.isbn}
				placeholder="978-..."
			/>
		</Stack>
	);

	// Authors tab
	const renderAuthorsTab = () => (
		<Stack gap="md">
			<LockableInput
				label="Writer"
				value={formState.writer}
				onChange={(v) => updateField("writer", v)}
				locked={locksState.writer}
				onLockChange={(v) => updateLock("writer", v)}
				originalValue={originalFormState?.writer}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Penciller"
				value={formState.penciller}
				onChange={(v) => updateField("penciller", v)}
				locked={locksState.penciller}
				onLockChange={(v) => updateLock("penciller", v)}
				originalValue={originalFormState?.penciller}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Inker"
				value={formState.inker}
				onChange={(v) => updateField("inker", v)}
				locked={locksState.inker}
				onLockChange={(v) => updateLock("inker", v)}
				originalValue={originalFormState?.inker}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Colorist"
				value={formState.colorist}
				onChange={(v) => updateField("colorist", v)}
				locked={locksState.colorist}
				onLockChange={(v) => updateLock("colorist", v)}
				originalValue={originalFormState?.colorist}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Letterer"
				value={formState.letterer}
				onChange={(v) => updateField("letterer", v)}
				locked={locksState.letterer}
				onLockChange={(v) => updateLock("letterer", v)}
				originalValue={originalFormState?.letterer}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Cover Artist"
				value={formState.coverArtist}
				onChange={(v) => updateField("coverArtist", v)}
				locked={locksState.coverArtist}
				onLockChange={(v) => updateLock("coverArtist", v)}
				originalValue={originalFormState?.coverArtist}
				placeholder="Comma-separated if multiple"
			/>

			<LockableInput
				label="Editor"
				value={formState.editor}
				onChange={(v) => updateField("editor", v)}
				locked={locksState.editor}
				onLockChange={(v) => updateLock("editor", v)}
				originalValue={originalFormState?.editor}
				placeholder="Comma-separated if multiple"
			/>
		</Stack>
	);

	// Tags tab
	const renderTagsTab = () => (
		<Stack gap="md">
			<LockableInput
				label="Genre"
				value={formState.genre}
				onChange={(v) => updateField("genre", v)}
				locked={locksState.genre}
				onLockChange={(v) => updateLock("genre", v)}
				originalValue={originalFormState?.genre}
				placeholder="e.g., Superhero, Action"
			/>

			<LockableInput
				label="Publisher"
				value={formState.publisher}
				onChange={(v) => updateField("publisher", v)}
				locked={locksState.publisher}
				onLockChange={(v) => updateLock("publisher", v)}
				originalValue={originalFormState?.publisher}
				placeholder="Publisher name"
			/>

			<LockableInput
				label="Imprint"
				value={formState.imprint}
				onChange={(v) => updateField("imprint", v)}
				locked={locksState.imprint}
				onLockChange={(v) => updateLock("imprint", v)}
				originalValue={originalFormState?.imprint}
				placeholder="Imprint (sub-publisher)"
			/>

			<LockableInput
				label="Language"
				value={formState.languageIso}
				onChange={(v) => updateField("languageIso", v)}
				locked={locksState.languageIso}
				onLockChange={(v) => updateLock("languageIso", v)}
				originalValue={originalFormState?.languageIso}
				placeholder="e.g., en, ja, ko"
			/>

			<LockableInput
				label="Format"
				value={formState.formatDetail}
				onChange={(v) => updateField("formatDetail", v)}
				locked={locksState.formatDetail}
				onLockChange={(v) => updateLock("formatDetail", v)}
				originalValue={originalFormState?.formatDetail}
				placeholder="e.g., Trade Paperback, Hardcover"
			/>

			<Group>
				<Switch
					label="Black and White"
					checked={formState.blackAndWhite ?? false}
					onChange={(e) => updateField("blackAndWhite", e.currentTarget.checked)}
				/>
				<Switch
					label="Manga"
					checked={formState.manga ?? false}
					onChange={(e) => updateField("manga", e.currentTarget.checked)}
				/>
			</Group>
		</Stack>
	);

	// Links tab
	const renderLinksTab = () => (
		<Stack gap="md">
			<LockableInput
				label="Web URL"
				value={formState.web}
				onChange={(v) => updateField("web", v)}
				locked={locksState.web}
				onLockChange={(v) => updateLock("web", v)}
				originalValue={originalFormState?.web}
				placeholder="https://..."
			/>
		</Stack>
	);

	// Poster tab
	const renderPosterTab = () => (
		<Stack gap="md">
			<Text size="sm" c="dimmed">
				Upload a custom cover image for this book.
			</Text>

			<ImageUploader
				value={posterImage}
				onChange={setPosterImage}
				label="Upload cover image - drag and drop"
				maxSize={10 * 1024 * 1024}
			/>
		</Stack>
	);

	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title={
				<Group gap="xs">
					<IconEdit size={20} />
					<Text fw={500}>Edit {bookTitle || "Book"}</Text>
				</Group>
			}
			size="lg"
			centered
			zIndex={1000}
			overlayProps={{
				backgroundOpacity: 0.55,
				blur: 3,
			}}
		>
			{isLoading ? (
				<Center h={300}>
					<Loader />
				</Center>
			) : (
				<Stack gap="md">
					<Tabs value={activeTab} onChange={setActiveTab}>
						<Tabs.List>
							<Tabs.Tab value="general" leftSection={<IconList size={16} />}>
								General
							</Tabs.Tab>
							<Tabs.Tab value="authors" leftSection={<IconUsers size={16} />}>
								Authors
							</Tabs.Tab>
							<Tabs.Tab value="tags" leftSection={<IconTag size={16} />}>
								Tags
							</Tabs.Tab>
							<Tabs.Tab value="links" leftSection={<IconLink size={16} />}>
								Links
							</Tabs.Tab>
							<Tabs.Tab value="poster" leftSection={<IconPhoto size={16} />}>
								Cover
							</Tabs.Tab>
						</Tabs.List>

						<Tabs.Panel value="general" pt="md">
							{renderGeneralTab()}
						</Tabs.Panel>

						<Tabs.Panel value="authors" pt="md">
							{renderAuthorsTab()}
						</Tabs.Panel>

						<Tabs.Panel value="tags" pt="md">
							{renderTagsTab()}
						</Tabs.Panel>

						<Tabs.Panel value="links" pt="md">
							{renderLinksTab()}
						</Tabs.Panel>

						<Tabs.Panel value="poster" pt="md">
							{renderPosterTab()}
						</Tabs.Panel>
					</Tabs>

					<Group justify="flex-end" mt="md">
						<Button variant="subtle" onClick={onClose}>
							Cancel
						</Button>
						<Button onClick={handleSave} loading={saveMutation.isPending}>
							Save Changes
						</Button>
					</Group>
				</Stack>
			)}
		</Modal>
	);
}
