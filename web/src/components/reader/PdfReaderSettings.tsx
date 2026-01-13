import {
	Box,
	Group,
	Modal,
	SegmentedControl,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import {
	type BackgroundColor,
	useReaderStore,
} from "@/store/readerStore";
import type { PdfZoomLevel } from "./PdfReader";

interface PdfReaderSettingsProps {
	/** Whether the modal is open */
	opened: boolean;
	/** Callback when modal is closed */
	onClose: () => void;
	/** Current zoom level */
	zoomLevel: PdfZoomLevel;
	/** Callback when zoom level changes */
	onZoomChange: (level: PdfZoomLevel) => void;
}

/**
 * Settings modal for the PDF reader.
 *
 * Allows configuring:
 * - Zoom level (fit page, fit width, percentage)
 * - Background color
 * - Auto-hide toolbar
 */
export function PdfReaderSettings({
	opened,
	onClose,
	zoomLevel,
	onZoomChange,
}: PdfReaderSettingsProps) {
	const settings = useReaderStore((state) => state.settings);
	const setBackgroundColor = useReaderStore(
		(state) => state.setBackgroundColor,
	);
	const setAutoHideToolbar = useReaderStore(
		(state) => state.setAutoHideToolbar,
	);

	return (
		<Modal opened={opened} onClose={onClose} title="PDF Reader Settings" size="md">
			<Stack gap="lg">
				{/* Zoom Level */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Zoom Level
					</Text>
					<SegmentedControl
						fullWidth
						value={zoomLevel}
						onChange={(value) => onZoomChange(value as PdfZoomLevel)}
						data={[
							{ label: "Fit Page", value: "fit-page" },
							{ label: "Fit Width", value: "fit-width" },
							{ label: "100%", value: "100%" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						{zoomLevel === "fit-page" && "Fit entire page within viewport"}
						{zoomLevel === "fit-width" && "Scale to viewport width"}
						{zoomLevel === "100%" && "Display at 100% zoom"}
					</Text>
				</Box>

				{/* Additional Zoom Levels */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						More Zoom Options
					</Text>
					<SegmentedControl
						fullWidth
						value={zoomLevel}
						onChange={(value) => onZoomChange(value as PdfZoomLevel)}
						data={[
							{ label: "50%", value: "50%" },
							{ label: "75%", value: "75%" },
							{ label: "125%", value: "125%" },
							{ label: "150%", value: "150%" },
							{ label: "200%", value: "200%" },
						]}
					/>
				</Box>

				{/* Background Color */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Background Color
					</Text>
					<SegmentedControl
						fullWidth
						value={settings.backgroundColor}
						onChange={(value) => setBackgroundColor(value as BackgroundColor)}
						data={[
							{ label: "Black", value: "black" },
							{ label: "Gray", value: "gray" },
							{ label: "White", value: "white" },
						]}
					/>
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
							<Text size="xs">Arrow keys, Space</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								First/Last page
							</Text>
							<Text size="xs">Home / End</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Toggle fullscreen
							</Text>
							<Text size="xs">F</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Search
							</Text>
							<Text size="xs">Ctrl+F / Cmd+F</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Close reader
							</Text>
							<Text size="xs">Esc</Text>
						</Group>
					</Stack>
				</Box>

				{/* PDF-specific info */}
				<Box
					p="sm"
					style={{
						backgroundColor: "var(--mantine-color-blue-9)",
						borderRadius: "var(--mantine-radius-sm)",
					}}
				>
					<Text size="sm" fw={500} mb="xs">
						Native PDF Features
					</Text>
					<Stack gap={4}>
						<Text size="xs" c="dimmed">
							Text selection and copy
						</Text>
						<Text size="xs" c="dimmed">
							Search within document (Ctrl+F)
						</Text>
						<Text size="xs" c="dimmed">
							Clickable links and bookmarks
						</Text>
						<Text size="xs" c="dimmed">
							Vector rendering (sharp at any zoom)
						</Text>
					</Stack>
				</Box>
			</Stack>
		</Modal>
	);
}
