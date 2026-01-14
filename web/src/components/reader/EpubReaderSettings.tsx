import {
	Box,
	Divider,
	Group,
	Modal,
	Select,
	Slider,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import {
	type EpubFontFamily,
	type EpubTheme,
	useReaderStore,
} from "@/store/readerStore";

interface EpubReaderSettingsProps {
	/** Whether the modal is open */
	opened: boolean;
	/** Callback when modal is closed */
	onClose: () => void;
}

/** Theme options for display in select - organized by light/dark */
const THEME_OPTIONS = [
	// Light themes
	{ value: "light", label: "Light" },
	{ value: "paper", label: "Paper (Warm)" },
	{ value: "sepia", label: "Sepia" },
	{ value: "rose", label: "Rose" },
	{ value: "mint", label: "Mint" },
	// Dark themes
	{ value: "dark", label: "Dark" },
	{ value: "slate", label: "Slate" },
	{ value: "night", label: "Night (OLED)" },
	{ value: "ocean", label: "Ocean" },
	{ value: "forest", label: "Forest" },
];

/** Font family options for display in select */
const FONT_FAMILY_OPTIONS = [
	{ value: "default", label: "Default" },
	{ value: "serif", label: "Serif (Georgia)" },
	{ value: "sans-serif", label: "Sans-serif (Helvetica)" },
	{ value: "monospace", label: "Monospace (Courier)" },
	{ value: "dyslexic", label: "Dyslexic-friendly" },
];

/**
 * Settings modal for the EPUB reader.
 *
 * Allows configuring:
 * - Theme (10 options: light, paper, sepia, rose, mint, dark, slate, night, ocean, forest)
 * - Font size
 * - Font family
 * - Line height
 * - Margins
 * - Toolbar auto-hide
 */
export function EpubReaderSettings({
	opened,
	onClose,
}: EpubReaderSettingsProps) {
	const settings = useReaderStore((state) => state.settings);
	const setEpubTheme = useReaderStore((state) => state.setEpubTheme);
	const setEpubFontSize = useReaderStore((state) => state.setEpubFontSize);
	const setEpubFontFamily = useReaderStore((state) => state.setEpubFontFamily);
	const setEpubLineHeight = useReaderStore((state) => state.setEpubLineHeight);
	const setEpubMargin = useReaderStore((state) => state.setEpubMargin);
	const setAutoHideToolbar = useReaderStore(
		(state) => state.setAutoHideToolbar,
	);
	const setAutoAdvanceToNextBook = useReaderStore(
		(state) => state.setAutoAdvanceToNextBook,
	);

	return (
		<Modal opened={opened} onClose={onClose} title="Reader Settings" size="md">
			<Stack gap="lg">
				{/* Theme */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Theme
					</Text>
					<Select
						value={settings.epubTheme}
						onChange={(value) => value && setEpubTheme(value as EpubTheme)}
						data={THEME_OPTIONS}
						allowDeselect={false}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						Background and text color theme
					</Text>
				</Box>

				{/* Font Family */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Font Family
					</Text>
					<Select
						value={settings.epubFontFamily}
						onChange={(value) =>
							value && setEpubFontFamily(value as EpubFontFamily)
						}
						data={FONT_FAMILY_OPTIONS}
						allowDeselect={false}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						Choose a typeface for reading
					</Text>
				</Box>

				{/* Font Size */}
				<Box>
					<Group justify="space-between" mb="xs">
						<Text size="sm" fw={500}>
							Font Size
						</Text>
						<Text size="sm" c="dimmed">
							{settings.epubFontSize}%
						</Text>
					</Group>
					<Slider
						value={settings.epubFontSize}
						onChange={setEpubFontSize}
						min={50}
						max={200}
						step={10}
						marks={[
							{ value: 50, label: "50%" },
							{ value: 100, label: "100%" },
							{ value: 150, label: "150%" },
							{ value: 200, label: "200%" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="lg">
						Adjust text size for comfortable reading
					</Text>
				</Box>

				{/* Line Height */}
				<Box>
					<Group justify="space-between" mb="xs">
						<Text size="sm" fw={500}>
							Line Spacing
						</Text>
						<Text size="sm" c="dimmed">
							{settings.epubLineHeight}%
						</Text>
					</Group>
					<Slider
						value={settings.epubLineHeight}
						onChange={setEpubLineHeight}
						min={100}
						max={250}
						step={10}
						marks={[
							{ value: 100, label: "Tight" },
							{ value: 140, label: "Normal" },
							{ value: 200, label: "Relaxed" },
							{ value: 250, label: "Loose" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="lg">
						Space between lines of text
					</Text>
				</Box>

				{/* Margins */}
				<Box>
					<Group justify="space-between" mb="xs">
						<Text size="sm" fw={500}>
							Margins
						</Text>
						<Text size="sm" c="dimmed">
							{settings.epubMargin}%
						</Text>
					</Group>
					<Slider
						value={settings.epubMargin}
						onChange={setEpubMargin}
						min={0}
						max={30}
						step={5}
						marks={[
							{ value: 0, label: "None" },
							{ value: 10, label: "Normal" },
							{ value: 20, label: "Wide" },
							{ value: 30, label: "Max" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="lg">
						Horizontal padding around text
					</Text>
				</Box>

				{/* Auto-hide Toolbar */}
				<Group justify="space-between">
					<Box>
						<Text size="sm" fw={500}>
							Auto-hide Toolbar
						</Text>
						<Text size="xs" c="dimmed">
							Hide toolbar after inactivity
						</Text>
					</Box>
					<Switch
						checked={settings.autoHideToolbar}
						onChange={(e) => setAutoHideToolbar(e.currentTarget.checked)}
					/>
				</Group>

				{/* Auto-advance to next book */}
				<Group justify="space-between">
					<Box>
						<Text size="sm" fw={500}>
							Auto-advance to next book
						</Text>
						<Text size="xs" c="dimmed">
							Automatically continue to next book in series
						</Text>
					</Box>
					<Switch
						checked={settings.autoAdvanceToNextBook}
						onChange={(e) => setAutoAdvanceToNextBook(e.currentTarget.checked)}
					/>
				</Group>

				<Divider />

				{/* Keyboard shortcuts info */}
				<Box
					p="sm"
					style={{
						backgroundColor: "var(--mantine-color-dark-6)",
						borderRadius: "var(--mantine-radius-sm)",
					}}
				>
					<Text size="sm" fw={500} mb="xs">
						Keyboard Shortcuts
					</Text>
					<Stack gap={4}>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Previous/Next page
							</Text>
							<Text size="xs">Arrow keys</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Table of contents
							</Text>
							<Text size="xs">T</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Toggle fullscreen
							</Text>
							<Text size="xs">F</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Toggle toolbar
							</Text>
							<Text size="xs">Space</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Close reader
							</Text>
							<Text size="xs">Esc</Text>
						</Group>
					</Stack>
				</Box>
			</Stack>
		</Modal>
	);
}
