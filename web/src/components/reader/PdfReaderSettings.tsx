import {
	Box,
	Divider,
	Grid,
	Group,
	Modal,
	SegmentedControl,
	Stack,
	Switch,
	Text,
	Title,
} from "@mantine/core";
import {
	type BackgroundColor,
	type PdfMode,
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
}

/**
 * Settings modal for the native PDF reader.
 *
 * Two-column layout on desktop, single column on mobile.
 * Left: Display settings (zoom, background, spread mode)
 * Right: PDF-specific options (continuous scroll, shortcuts)
 *
 * Includes PDF rendering mode toggle to switch back to streaming mode.
 */
export function PdfReaderSettings({
	opened,
	onClose,
	zoomLevel,
	onZoomChange,
}: PdfReaderSettingsProps) {
	const settings = useReaderStore((state) => state.settings);
	const pdfMode = useReaderStore((state) => state.settings.pdfMode);
	const setPdfMode = useReaderStore((state) => state.setPdfMode);
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
		<Modal opened={opened} onClose={onClose} title="Reader Settings" size="lg">
			<Stack gap="md">
				{/* PDF Rendering Mode - full width at top */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						PDF Rendering Mode
					</Text>
					<SegmentedControl
						fullWidth
						value={pdfMode}
						onChange={(value) => setPdfMode(value as PdfMode)}
						data={[
							{ label: "Auto", value: "auto" },
							{ label: "Streaming", value: "streaming" },
							{ label: "Native", value: "native" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						{pdfMode === "auto"
							? "Automatically selects based on file size (>100MB uses streaming)"
							: pdfMode === "streaming"
								? "Server renders pages as images (lower bandwidth)"
								: "Downloads full PDF for text selection and search"}
					</Text>
					<Text size="xs" c="yellow" mt={4}>
						Re-open the book after changing to apply
					</Text>
				</Box>

				<Group justify="space-between">
					<Text size="sm" fw={500}>
						Auto-hide toolbar
					</Text>
					<Switch
						size="sm"
						checked={settings.autoHideToolbar}
						onChange={(e) => setAutoHideToolbar(e.currentTarget.checked)}
					/>
				</Group>

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
						size="sm"
						checked={settings.pdfContinuousScroll}
						onChange={(e) => setPdfContinuousScroll(e.currentTarget.checked)}
					/>
				</Group>

				<Divider />

				{/* Two-column layout */}
				<Grid gutter="xl">
					{/* Left Column: Display */}
					<Grid.Col span={{ base: 12, sm: 6 }}>
						<Stack gap="md">
							<Title order={6} c="dimmed">
								Display
							</Title>

							{/* Zoom Level */}
							<Box>
								<Text size="sm" fw={500} mb="xs">
									Zoom
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
							</Box>

							{/* More Zoom Options */}
							<Box>
								<Text size="sm" fw={500} mb="xs">
									More Zoom
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
									]}
								/>
							</Box>

							{/* Background */}
							<Box>
								<Text size="sm" fw={500} mb="xs">
									Background
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

							{/* Page Spread - only show when not in continuous scroll mode */}
							{!settings.pdfContinuousScroll && (
								<Box>
									<Text size="sm" fw={500} mb="xs">
										Page layout
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
							)}
						</Stack>
					</Grid.Col>

					{/* Right Column: Native PDF Features */}
					<Grid.Col span={{ base: 12, sm: 6 }}>
						<Stack gap="md">
							<Title order={6} c="dimmed">
								Native PDF Features
							</Title>

							{/* Features info box */}
							<Box
								p="sm"
								style={{
									backgroundColor: "var(--mantine-color-blue-light)",
									borderRadius: "var(--mantine-radius-sm)",
								}}
							>
								<Stack gap={4}>
									<Text size="xs">Text selection and copy</Text>
									<Text size="xs">Search within document (Ctrl+F)</Text>
									<Text size="xs">Clickable links and bookmarks</Text>
									<Text size="xs">Vector rendering (sharp at any zoom)</Text>
								</Stack>
							</Box>

							{/* Keyboard shortcuts */}
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
										<Text size="xs">← → ↑ ↓</Text>
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
						</Stack>
					</Grid.Col>
				</Grid>

				{/* Bottom keyboard shortcuts - compact for desktop */}
				<Divider visibleFrom="sm" />
				<Group justify="space-between" gap="xl" visibleFrom="sm">
					<Group gap="lg">
						<Text size="xs" c="dimmed">
							<Text span fw={500}>
								← → ↑ ↓
							</Text>{" "}
							Navigate
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>
								Home/End
							</Text>{" "}
							First/Last
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>
								F
							</Text>{" "}
							Fullscreen
						</Text>
					</Group>
					<Group gap="lg">
						<Text size="xs" c="dimmed">
							<Text span fw={500}>
								Ctrl+F
							</Text>{" "}
							Search
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>
								Esc
							</Text>{" "}
							Close
						</Text>
					</Group>
				</Group>
			</Stack>
		</Modal>
	);
}
