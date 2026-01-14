import {
	Button,
	Group,
	Loader,
	Modal,
	Stack,
	Tabs,
	Text,
	Center,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconEdit,
	IconLink,
	IconList,
	IconPhoto,
	IconTag,
	IconTypography,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import {
	seriesMetadataApi,
	type FullSeriesMetadata,
	type MetadataLocks,
	type AlternateTitle,
	type ExternalLink,
} from "@/api/seriesMetadata";
import {
	LockableInput,
	LockableTextarea,
	LockableSelect,
	LockableChipInput,
	LockableListEditor,
	ImageUploader,
	type ListItem,
	type ImageInfo,
} from "@/components/forms/lockable";

export interface SeriesMetadataEditModalProps {
	opened: boolean;
	onClose: () => void;
	seriesId: string;
	seriesTitle?: string;
}

interface FormState {
	title: string;
	titleSort: string;
	summary: string;
	status: string | null;
	language: string;
	readingDirection: string | null;
	publisher: string;
	ageRating: string;
	imprint: string;
	year: string;
	genres: string[];
	tags: string[];
	alternateTitles: ListItem[];
	externalLinks: ListItem[];
}

interface LocksState {
	title: boolean;
	titleSort: boolean;
	summary: boolean;
	status: boolean;
	language: boolean;
	readingDirection: boolean;
	publisher: boolean;
	ageRating: boolean;
	imprint: boolean;
	year: boolean;
	genres: boolean;
	tags: boolean;
}

const STATUS_OPTIONS = [
	{ value: "ongoing", label: "Ongoing" },
	{ value: "ended", label: "Ended" },
	{ value: "hiatus", label: "Hiatus" },
	{ value: "abandoned", label: "Abandoned" },
	{ value: "unknown", label: "Unknown" },
];

const READING_DIRECTION_OPTIONS = [
	{ value: "ltr", label: "Left to Right (Comics)" },
	{ value: "rtl", label: "Right to Left (Manga)" },
	{ value: "ttb", label: "Top to Bottom" },
	{ value: "webtoon", label: "Webtoon" },
];

const ALTERNATE_TITLE_LABELS = [
	{ value: "native", label: "Native" },
	{ value: "roman", label: "Roman" },
	{ value: "english", label: "English" },
	{ value: "japanese", label: "Japanese" },
	{ value: "korean", label: "Korean" },
	{ value: "chinese", label: "Chinese" },
	{ value: "other", label: "Other" },
];

function initializeFormState(metadata: FullSeriesMetadata | undefined): FormState {
	return {
		title: metadata?.title || "",
		titleSort: metadata?.sortName || "",
		summary: metadata?.summary || "",
		status: metadata?.status || null,
		language: metadata?.language || "",
		readingDirection: metadata?.readingDirection || null,
		publisher: metadata?.publisher || "",
		ageRating: metadata?.ageRating?.toString() || "",
		imprint: metadata?.imprint || "",
		year: metadata?.year?.toString() || "",
		genres: metadata?.genres.map((g) => g.name) || [],
		tags: metadata?.tags?.map((t) => t.name) || [],
		alternateTitles:
			metadata?.alternateTitles.map((t) => ({
				id: t.id,
				values: { label: t.label || "other", title: t.title },
				locked: false,
			})) || [],
		externalLinks:
			metadata?.externalLinks.map((l) => ({
				id: l.id,
				values: { label: l.sourceName, url: l.url },
				locked: false,
			})) || [],
	};
}

function initializeLocksState(locks: MetadataLocks | undefined): LocksState {
	return {
		title: locks?.title || false,
		titleSort: locks?.titleSort || false,
		summary: locks?.summary || false,
		status: locks?.status || false,
		language: locks?.language || false,
		readingDirection: locks?.readingDirection || false,
		publisher: locks?.publisher || false,
		ageRating: locks?.ageRating || false,
		imprint: locks?.imprint || false,
		year: locks?.year || false,
		genres: locks?.genres || false,
		tags: locks?.tags || false,
	};
}

export function SeriesMetadataEditModal({
	opened,
	onClose,
	seriesId,
	seriesTitle,
}: SeriesMetadataEditModalProps) {
	const queryClient = useQueryClient();
	const [activeTab, setActiveTab] = useState<string | null>("general");
	const [formState, setFormState] = useState<FormState>(initializeFormState(undefined));
	const [locksState, setLocksState] = useState<LocksState>(initializeLocksState(undefined));
	const [originalFormState, setOriginalFormState] = useState<FormState | null>(null);
	const [posterImage, setPosterImage] = useState<ImageInfo | null>(null);

	// Fetch full metadata
	const { data: metadata, isLoading } = useQuery({
		queryKey: ["series", seriesId, "metadata", "full"],
		queryFn: () => seriesMetadataApi.getFullMetadata(seriesId),
		enabled: opened,
	});

	// Initialize form state when metadata loads
	useEffect(() => {
		if (metadata) {
			const newFormState = initializeFormState(metadata);
			setFormState(newFormState);
			setOriginalFormState(newFormState);
			setLocksState(initializeLocksState(metadata.locks));
		}
	}, [metadata]);

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
			// Update metadata
			await seriesMetadataApi.patchMetadata(seriesId, {
				sortName: formState.titleSort || null,
				summary: formState.summary || null,
				status: formState.status || undefined,
				language: formState.language || null,
				readingDirection: formState.readingDirection || undefined,
				publisher: formState.publisher || null,
				year: formState.year ? Number.parseInt(formState.year, 10) : null,
			});

			// Update locks
			await seriesMetadataApi.updateLocks(seriesId, locksState);

			// Upload poster image if selected
			if (posterImage?.file) {
				await seriesMetadataApi.uploadCover(seriesId, posterImage.file);
			}

			// Handle alternate titles changes
			const originalTitleIds = new Set(
				originalFormState?.alternateTitles.map((t) => t.id) || [],
			);
			const currentTitleIds = new Set(
				formState.alternateTitles.filter((t) => !t.id.startsWith("new-")).map((t) => t.id),
			);

			// Delete removed titles
			for (const title of originalFormState?.alternateTitles || []) {
				if (!currentTitleIds.has(title.id)) {
					await seriesMetadataApi.deleteAlternateTitle(seriesId, title.id);
				}
			}

			// Create new titles and update existing ones
			for (const title of formState.alternateTitles) {
				if (title.id.startsWith("new-")) {
					// Create new title
					await seriesMetadataApi.createAlternateTitle(
						seriesId,
						title.values.title,
						title.values.label,
					);
				} else if (originalTitleIds.has(title.id)) {
					// Update existing title
					const original = originalFormState?.alternateTitles.find(
						(t) => t.id === title.id,
					);
					if (
						original &&
						(original.values.title !== title.values.title ||
							original.values.label !== title.values.label)
					) {
						await seriesMetadataApi.updateAlternateTitle(
							seriesId,
							title.id,
							title.values.title,
							title.values.label,
						);
					}
				}
			}

			// Handle external links changes
			const originalLinkIds = new Set(
				originalFormState?.externalLinks.map((l) => l.id) || [],
			);
			const currentLinkIds = new Set(
				formState.externalLinks.filter((l) => !l.id.startsWith("new-")).map((l) => l.id),
			);

			// Delete removed links
			for (const link of originalFormState?.externalLinks || []) {
				if (!currentLinkIds.has(link.id)) {
					await seriesMetadataApi.deleteExternalLink(seriesId, link.id);
				}
			}

			// Create new links
			for (const link of formState.externalLinks) {
				if (link.id.startsWith("new-")) {
					await seriesMetadataApi.createExternalLink(
						seriesId,
						link.values.label,
						link.values.url,
					);
				}
			}
		},
		onSuccess: () => {
			notifications.show({
				title: "Success",
				message: "Series metadata updated successfully",
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
			onClose();
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update series metadata",
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
				locked={locksState.title}
				onLockChange={(v) => updateLock("title", v)}
				originalValue={originalFormState?.title}
				disabled
				description="Title cannot be changed from this modal"
			/>

			<LockableInput
				label="Sort Title"
				value={formState.titleSort}
				onChange={(v) => updateField("titleSort", v)}
				locked={locksState.titleSort}
				onLockChange={(v) => updateLock("titleSort", v)}
				originalValue={originalFormState?.titleSort}
				placeholder="Sort title (e.g., 'Avengers, The')"
			/>

			<LockableTextarea
				label="Summary"
				value={formState.summary}
				onChange={(v) => updateField("summary", v)}
				locked={locksState.summary}
				onLockChange={(v) => updateLock("summary", v)}
				originalValue={originalFormState?.summary}
				placeholder="Enter series summary..."
				minRows={4}
				autosize
			/>

			<LockableSelect
				label="Status"
				value={formState.status}
				onChange={(v) => updateField("status", v)}
				locked={locksState.status}
				onLockChange={(v) => updateLock("status", v)}
				originalValue={originalFormState?.status}
				data={STATUS_OPTIONS}
				placeholder="Select status"
				clearable
			/>

			<LockableInput
				label="Language"
				value={formState.language}
				onChange={(v) => updateField("language", v)}
				locked={locksState.language}
				onLockChange={(v) => updateLock("language", v)}
				originalValue={originalFormState?.language}
				placeholder="e.g., en, ja, ko"
			/>

			<LockableSelect
				label="Reading Direction"
				value={formState.readingDirection}
				onChange={(v) => updateField("readingDirection", v)}
				locked={locksState.readingDirection}
				onLockChange={(v) => updateLock("readingDirection", v)}
				originalValue={originalFormState?.readingDirection}
				data={READING_DIRECTION_OPTIONS}
				placeholder="Select reading direction"
				clearable
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
				label="Year"
				value={formState.year}
				onChange={(v) => updateField("year", v)}
				locked={locksState.year}
				onLockChange={(v) => updateLock("year", v)}
				originalValue={originalFormState?.year}
				placeholder="Publication year"
				type="number"
			/>

			<LockableInput
				label="Age Rating"
				value={formState.ageRating}
				onChange={(v) => updateField("ageRating", v)}
				locked={locksState.ageRating}
				onLockChange={(v) => updateLock("ageRating", v)}
				originalValue={originalFormState?.ageRating}
				placeholder="e.g., 13, 16, 18"
				type="number"
			/>
		</Stack>
	);

	// Alternate titles tab
	const renderAlternateTitlesTab = () => (
		<Stack gap="md">
			<Text size="sm" c="dimmed">
				Add alternate titles for this series (e.g., native title, romanized title).
			</Text>

			<LockableListEditor
				items={formState.alternateTitles}
				onChange={(items) => updateField("alternateTitles", items)}
				fields={[
					{
						key: "label",
						label: "Type",
						placeholder: "Select type",
						flex: 1,
					},
					{
						key: "title",
						label: "Title",
						placeholder: "Alternate title",
						flex: 2,
					},
				]}
				originalItems={originalFormState?.alternateTitles}
				addButtonLabel="Add Alternate Title"
				generateId={() => `new-${crypto.randomUUID()}`}
			/>
		</Stack>
	);

	// Tags tab
	const renderTagsTab = () => (
		<Stack gap="md">
			<LockableChipInput
				label="Genres"
				value={formState.genres}
				onChange={(v) => updateField("genres", v)}
				locked={locksState.genres}
				onLockChange={(v) => updateLock("genres", v)}
				originalValue={originalFormState?.genres}
				placeholder="Add genres..."
				description="Press Enter to add a genre"
			/>

			<LockableChipInput
				label="Tags"
				value={formState.tags}
				onChange={(v) => updateField("tags", v)}
				locked={locksState.tags}
				onLockChange={(v) => updateLock("tags", v)}
				originalValue={originalFormState?.tags}
				placeholder="Add tags..."
				description="Press Enter to add a tag"
			/>
		</Stack>
	);

	// Links tab
	const renderLinksTab = () => (
		<Stack gap="md">
			<Text size="sm" c="dimmed">
				Add external links to other sites (e.g., MyAnimeList, AniList).
			</Text>

			<LockableListEditor
				items={formState.externalLinks}
				onChange={(items) => updateField("externalLinks", items)}
				fields={[
					{
						key: "label",
						label: "Site Name",
						placeholder: "e.g., MyAnimeList",
						flex: 1,
					},
					{
						key: "url",
						label: "URL",
						placeholder: "https://...",
						flex: 2,
					},
				]}
				originalItems={originalFormState?.externalLinks}
				addButtonLabel="Add Link"
				generateId={() => `new-${crypto.randomUUID()}`}
			/>
		</Stack>
	);

	// Poster tab
	const renderPosterTab = () => (
		<Stack gap="md">
			<Text size="sm" c="dimmed">
				Upload a custom poster image for this series.
			</Text>

			<ImageUploader
				value={posterImage}
				onChange={setPosterImage}
				label="Upload poster image - drag and drop"
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
					<Text fw={500}>Edit {seriesTitle || "Series"}</Text>
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
							<Tabs.Tab
								value="alternateTitles"
								leftSection={<IconTypography size={16} />}
							>
								Titles
							</Tabs.Tab>
							<Tabs.Tab value="tags" leftSection={<IconTag size={16} />}>
								Tags
							</Tabs.Tab>
							<Tabs.Tab value="links" leftSection={<IconLink size={16} />}>
								Links
							</Tabs.Tab>
							<Tabs.Tab value="poster" leftSection={<IconPhoto size={16} />}>
								Poster
							</Tabs.Tab>
						</Tabs.List>

						<Tabs.Panel value="general" pt="md">
							{renderGeneralTab()}
						</Tabs.Panel>

						<Tabs.Panel value="alternateTitles" pt="md">
							{renderAlternateTitlesTab()}
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
