import {
	Box,
	Group,
	Modal,
	SegmentedControl,
	Slider,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import { type EpubTheme, useReaderStore } from "@/store/readerStore";

interface EpubReaderSettingsProps {
	/** Whether the modal is open */
	opened: boolean;
	/** Callback when modal is closed */
	onClose: () => void;
}

/**
 * Settings modal for the EPUB reader.
 *
 * Allows configuring:
 * - Theme (light, sepia, dark, mint, slate)
 * - Font size
 * - Toolbar auto-hide
 */
export function EpubReaderSettings({
	opened,
	onClose,
}: EpubReaderSettingsProps) {
	const settings = useReaderStore((state) => state.settings);
	const setEpubTheme = useReaderStore((state) => state.setEpubTheme);
	const setEpubFontSize = useReaderStore((state) => state.setEpubFontSize);
	const setAutoHideToolbar = useReaderStore(
		(state) => state.setAutoHideToolbar,
	);

	return (
		<Modal opened={opened} onClose={onClose} title="Reader Settings" size="md">
			<Stack gap="lg">
				{/* Theme */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Theme
					</Text>
					<SegmentedControl
						fullWidth
						value={settings.epubTheme}
						onChange={(value) => setEpubTheme(value as EpubTheme)}
						data={[
							{ label: "Light", value: "light" },
							{ label: "Sepia", value: "sepia" },
							{ label: "Dark", value: "dark" },
						]}
					/>
					<Group gap="xs" mt="xs">
						<SegmentedControl
							fullWidth
							value={settings.epubTheme}
							onChange={(value) => setEpubTheme(value as EpubTheme)}
							data={[
								{ label: "Mint", value: "mint" },
								{ label: "Slate", value: "slate" },
							]}
						/>
					</Group>
					<Text size="xs" c="dimmed" mt="xs">
						Background and text color theme
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
