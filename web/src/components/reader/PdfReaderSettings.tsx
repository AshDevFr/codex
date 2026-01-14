import {
	Box,
	Button,
	Group,
	Modal,
	SegmentedControl,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import {
	type BackgroundColor,
	type PdfSpreadMode,
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
	/** Whether this book has a per-book PDF mode preference saved */
	hasPerBookPdfMode?: boolean;
	/** Callback to save per-book PDF mode preference */
	onSavePerBookPdfMode?: (mode: "streaming" | "native") => void;
	/** Callback to clear per-book PDF mode preference */
	onClearPerBookPdfMode?: () => void;
}

/**
 * Settings modal for the PDF reader.
 *
 * Allows configuring:
 * - Zoom level (fit page, fit width, percentage)
 * - Background color
 * - Auto-hide toolbar
 * - Per-book PDF mode preference
 */
export function PdfReaderSettings({
	opened,
	onClose,
	zoomLevel,
	onZoomChange,
	hasPerBookPdfMode,
	onSavePerBookPdfMode,
	onClearPerBookPdfMode,
}: PdfReaderSettingsProps) {
	const settings = useReaderStore((state) => state.settings);
	const setBackgroundColor = useReaderStore(
		(state) => state.setBackgroundColor,
	);
	const setPdfSpreadMode = useReaderStore((state) => state.setPdfSpreadMode);
	const setPdfContinuousScroll = useReaderStore(
		(state) => state.setPdfContinuousScroll,
	);
	const setAutoHideToolbar = useReaderStore(
		(state) => state.setAutoHideToolbar,
	);

	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title="PDF Reader Settings"
			size="md"
		>
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

				{/* Page Spread Mode */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Page Spread
					</Text>
					<SegmentedControl
						fullWidth
						value={settings.pdfSpreadMode}
						onChange={(value) => setPdfSpreadMode(value as PdfSpreadMode)}
						data={[
							{ label: "Single", value: "single" },
							{ label: "Double", value: "double" },
							{ label: "Double (Odd)", value: "double-odd" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						{settings.pdfSpreadMode === "single" &&
							"Display one page at a time"}
						{settings.pdfSpreadMode === "double" &&
							"Display two pages side by side"}
						{settings.pdfSpreadMode === "double-odd" &&
							"Two pages, starting spreads on odd pages"}
					</Text>
				</Box>

				{/* Continuous Scroll */}
				<Group justify="space-between">
					<Box>
						<Text size="sm" fw={500}>
							Continuous Scroll
						</Text>
						<Text size="xs" c="dimmed">
							Scroll through all pages vertically
						</Text>
					</Box>
					<Switch
						checked={settings.pdfContinuousScroll}
						onChange={(e) => setPdfContinuousScroll(e.currentTarget.checked)}
					/>
				</Group>

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

				{/* Per-book PDF mode preference */}
				{onSavePerBookPdfMode && onClearPerBookPdfMode && (
					<Box>
						<Text size="sm" fw={500} mb="xs">
							Remember for this Book
						</Text>
						<Group gap="sm">
							{hasPerBookPdfMode ? (
								<>
									<Text size="xs" c="dimmed" style={{ flex: 1 }}>
										Native mode saved for this book
									</Text>
									<Button
										size="xs"
										variant="subtle"
										color="gray"
										onClick={onClearPerBookPdfMode}
									>
										Clear
									</Button>
								</>
							) : (
								<>
									<Text size="xs" c="dimmed" style={{ flex: 1 }}>
										Use global setting
									</Text>
									<Button
										size="xs"
										variant="light"
										onClick={() => onSavePerBookPdfMode("native")}
									>
										Save for this book
									</Button>
								</>
							)}
						</Group>
						<Text size="xs" c="dimmed" mt="xs">
							Override global PDF mode preference for this specific book
						</Text>
					</Box>
				)}

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
