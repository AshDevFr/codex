import {
	Box,
	Divider,
	Group,
	Modal,
	SegmentedControl,
	Select,
	Slider,
	Stack,
	Switch,
	Text,
	Title,
} from "@mantine/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import {
	type BackgroundColor,
	type FitMode,
	type PageLayout,
	type PageTransition,
	type ReadingDirection,
	selectEffectiveReadingDirection,
	useReaderStore,
} from "@/store/readerStore";

interface ReaderSettingsProps {
	/** Whether the modal is open */
	opened: boolean;
	/** Callback when modal is closed */
	onClose: () => void;
	/** Series ID for updating reading direction (optional) */
	seriesId?: string | null;
}

/**
 * Settings modal for the comic reader.
 *
 * Organized into sections:
 * - General: Reading mode, transitions, gestures
 * - Display: Background color
 * - Mode-specific options: Different for Webtoon vs Paginated modes
 */
export function ReaderSettings({ opened, onClose, seriesId }: ReaderSettingsProps) {
	const queryClient = useQueryClient();
	const settings = useReaderStore((state) => state.settings);
	const effectiveReadingDirection = useReaderStore(selectEffectiveReadingDirection);
	const setFitMode = useReaderStore((state) => state.setFitMode);
	const setPageLayout = useReaderStore((state) => state.setPageLayout);
	const setReadingDirectionOverride = useReaderStore(
		(state) => state.setReadingDirectionOverride,
	);
	const setBackgroundColor = useReaderStore(
		(state) => state.setBackgroundColor,
	);
	const setAutoHideToolbar = useReaderStore(
		(state) => state.setAutoHideToolbar,
	);
	const setPreloadPages = useReaderStore((state) => state.setPreloadPages);
	const setDoublePageShowWideAlone = useReaderStore(
		(state) => state.setDoublePageShowWideAlone,
	);
	const setDoublePageStartOnOdd = useReaderStore(
		(state) => state.setDoublePageStartOnOdd,
	);
	const setPageTransition = useReaderStore(
		(state) => state.setPageTransition,
	);
	const setTransitionDuration = useReaderStore(
		(state) => state.setTransitionDuration,
	);
	const setWebtoonSidePadding = useReaderStore(
		(state) => state.setWebtoonSidePadding,
	);
	const setWebtoonPageGap = useReaderStore(
		(state) => state.setWebtoonPageGap,
	);

	// Mutation to update series reading direction
	const updateSeriesReadingDirection = useMutation({
		mutationFn: async (direction: ReadingDirection) => {
			if (!seriesId) {
				return;
			}
			return seriesMetadataApi.patchMetadata(seriesId, {
				readingDirection: direction,
			});
		},
		onSuccess: () => {
			// Invalidate series metadata cache to reflect the change
			if (seriesId) {
				queryClient.invalidateQueries({ queryKey: ["seriesMetadata", seriesId] });
			}
		},
	});

	// Handle reading mode change - update session state and series metadata
	const handleReadingModeChange = (direction: ReadingDirection) => {
		// Update session state (not persisted preference)
		setReadingDirectionOverride(direction);

		// When switching to webtoon mode, ensure fitMode is compatible with continuous scroll
		// (only "width" and "original" are supported in continuous mode)
		if (direction === "webtoon" && !["width", "original"].includes(settings.fitMode)) {
			setFitMode("width");
		}

		// Update series metadata via API
		if (seriesId) {
			updateSeriesReadingDirection.mutate(direction);
		}
	};

	// Determine if we're in continuous scroll mode (either by page layout or webtoon reading direction)
	const isContinuousMode = settings.pageLayout === "continuous" || effectiveReadingDirection === "webtoon";

	return (
		<Modal opened={opened} onClose={onClose} title="Reader Settings" size="md">
			<Stack gap="lg">
				{/* ============================================================ */}
				{/* General Section */}
				{/* ============================================================ */}
				<Title order={5} c="dimmed">General</Title>

				{/* Reading Mode - combines direction and layout concept */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Reading mode
					</Text>
					<Select
						value={effectiveReadingDirection}
						onChange={(value) => value && handleReadingModeChange(value as ReadingDirection)}
						data={[
							{ label: "Left to Right", value: "ltr" },
							{ label: "Right to Left", value: "rtl" },
							{ label: "Vertical", value: "ttb" },
							{ label: "Webtoon", value: "webtoon" },
						]}
					/>
					<Text size="xs" c="dimmed" mt="xs">
						{seriesId ? "Saved to series metadata" : "Navigation direction for this session"}
					</Text>
				</Box>

				{/* Animate Page Transitions - only for paginated modes */}
				{!isContinuousMode && (
					<Group justify="space-between">
						<Box>
							<Text size="sm" fw={500}>
								Animate page transitions
							</Text>
							<Text size="xs" c="dimmed">
								Smooth animation when changing pages
							</Text>
						</Box>
						<Switch
							checked={settings.pageTransition !== "none"}
							onChange={(e) =>
								setPageTransition(e.currentTarget.checked ? "slide" : "none")
							}
						/>
					</Group>
				)}

				{/* Always Full Screen */}
				<Group justify="space-between">
					<Box>
						<Text size="sm" fw={500}>
							Auto-hide toolbar
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

				<Divider />

				{/* ============================================================ */}
				{/* Display Section */}
				{/* ============================================================ */}
				<Title order={5} c="dimmed">Display</Title>

				{/* Background Color */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Background color
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

				<Divider />

				{/* ============================================================ */}
				{/* Mode-specific Options */}
				{/* ============================================================ */}
				{isContinuousMode ? (
					<>
						<Title order={5} c="dimmed">Continuous Scroll Options</Title>

						{/* Scale Type for Continuous Scroll */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Scale type
							</Text>
							<SegmentedControl
								fullWidth
								value={settings.fitMode === "width" || settings.fitMode === "original" ? settings.fitMode : "width"}
								onChange={(value) => setFitMode(value as FitMode)}
								data={[
									{ label: "Fit width", value: "width" },
									{ label: "Original size", value: "original" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="xs">
								How images are scaled in the vertical scroll
							</Text>
						</Box>

						{/* Side Padding for Continuous Scroll */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Side padding
							</Text>
							<Slider
								value={settings.webtoonSidePadding}
								onChange={setWebtoonSidePadding}
								min={0}
								max={40}
								step={5}
								marks={[
									{ value: 0, label: "0%" },
									{ value: 20, label: "20%" },
									{ value: 40, label: "40%" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="md">
								{settings.webtoonSidePadding === 0 ? "No side padding" : `${settings.webtoonSidePadding}% padding on each side`}
							</Text>
						</Box>

						{/* Page Gap for Continuous Scroll */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Page gap
							</Text>
							<Slider
								value={settings.webtoonPageGap}
								onChange={setWebtoonPageGap}
								min={0}
								max={20}
								step={5}
								marks={[
									{ value: 0, label: "0" },
									{ value: 10, label: "10px" },
									{ value: 20, label: "20px" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="md">
								{settings.webtoonPageGap === 0 ? "No gap between pages" : `${settings.webtoonPageGap}px gap between pages`}
							</Text>
						</Box>

						{/* Preload Buffer for Continuous Scroll */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Preload buffer
							</Text>
							<Slider
								value={settings.preloadPages}
								onChange={setPreloadPages}
								min={0}
								max={5}
								step={1}
								marks={[
									{ value: 0, label: "0" },
									{ value: 1, label: "1" },
									{ value: 2, label: "2" },
									{ value: 3, label: "3" },
									{ value: 4, label: "4" },
									{ value: 5, label: "5" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="md">
								Pages to preload above and below viewport
							</Text>
						</Box>
					</>
				) : (
					<>
						<Title order={5} c="dimmed">Paginated Reader Options</Title>

						{/* Scale Type for Paginated */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Scale type
							</Text>
							<Select
								value={settings.fitMode}
								onChange={(value) => value && setFitMode(value as FitMode)}
								data={[
									{ label: "Fit screen", value: "screen" },
									{ label: "Fit width", value: "width" },
									{ label: "Fit width (shrink only)", value: "width-shrink" },
									{ label: "Fit height", value: "height" },
									{ label: "Original size", value: "original" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="xs">
								{settings.fitMode === "screen" && "Fit entire page within viewport (no scrolling)"}
								{settings.fitMode === "width" && "Scale to viewport width (may need vertical scroll)"}
								{settings.fitMode === "width-shrink" && "Fit to width, but only shrink larger images"}
								{settings.fitMode === "height" && "Scale to viewport height (may need horizontal scroll)"}
								{settings.fitMode === "original" && "Display at native resolution (1:1 pixels)"}
							</Text>
						</Box>

						{/* Page Layout for Paginated */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Page layout
							</Text>
							<SegmentedControl
								fullWidth
								value={settings.pageLayout === "continuous" ? "single" : settings.pageLayout}
								onChange={(value) => setPageLayout(value as PageLayout)}
								data={[
									{ label: "Single page", value: "single" },
									{ label: "Double pages", value: "double" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="xs">
								Single page or two-page spread view
							</Text>
						</Box>

						{/* Double Page Options - only show when double layout is selected */}
						{settings.pageLayout === "double" && (
							<Box
								p="sm"
								style={{
									backgroundColor: "var(--mantine-color-dark-7)",
									borderRadius: "var(--mantine-radius-sm)",
								}}
							>
								<Stack gap="sm">
									<Group justify="space-between">
										<Box>
											<Text size="sm">Show wide pages alone</Text>
											<Text size="xs" c="dimmed">
												Display landscape pages as single pages
											</Text>
										</Box>
										<Switch
											checked={settings.doublePageShowWideAlone}
											onChange={(e) =>
												setDoublePageShowWideAlone(e.currentTarget.checked)
											}
										/>
									</Group>
									<Group justify="space-between">
										<Box>
											<Text size="sm">Start on odd page</Text>
											<Text size="xs" c="dimmed">
												Page 1 alone, then 2-3, 4-5, etc. (manga covers)
											</Text>
										</Box>
										<Switch
											checked={settings.doublePageStartOnOdd}
											onChange={(e) =>
												setDoublePageStartOnOdd(e.currentTarget.checked)
											}
										/>
									</Group>
								</Stack>
							</Box>
						)}

						{/* Page Transition Type - only when transitions are enabled */}
						{settings.pageTransition !== "none" && (
							<Box>
								<Text size="sm" fw={500} mb="xs">
									Transition style
								</Text>
								<SegmentedControl
									fullWidth
									value={settings.pageTransition}
									onChange={(value) => setPageTransition(value as PageTransition)}
									data={[
										{ label: "Fade", value: "fade" },
										{ label: "Slide", value: "slide" },
									]}
								/>
							</Box>
						)}

						{/* Transition Duration - only show when transition is enabled */}
						{settings.pageTransition !== "none" && (
							<Box>
								<Text size="sm" fw={500} mb="xs">
									Transition speed
								</Text>
								<Slider
									value={settings.transitionDuration}
									onChange={setTransitionDuration}
									min={50}
									max={500}
									step={50}
									marks={[
										{ value: 50, label: "Fast" },
										{ value: 200, label: "" },
										{ value: 350, label: "" },
										{ value: 500, label: "Slow" },
									]}
								/>
								<Text size="xs" c="dimmed" mt="md">
									{settings.transitionDuration}ms
								</Text>
							</Box>
						)}

						{/* Preload Pages for Paginated */}
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Preload pages
							</Text>
							<Slider
								value={settings.preloadPages}
								onChange={setPreloadPages}
								min={0}
								max={5}
								step={1}
								marks={[
									{ value: 0, label: "0" },
									{ value: 1, label: "1" },
									{ value: 2, label: "2" },
									{ value: 3, label: "3" },
									{ value: 4, label: "4" },
									{ value: 5, label: "5" },
								]}
							/>
							<Text size="xs" c="dimmed" mt="md">
								Pages to preload ahead and behind
							</Text>
						</Box>
					</>
				)}

				<Divider />

				{/* ============================================================ */}
				{/* Keyboard shortcuts info */}
				{/* ============================================================ */}
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
								{isContinuousMode ? "Scroll up/down" : "Previous/Next page"}
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
								Toggle toolbar
							</Text>
							<Text size="xs">T</Text>
						</Group>
						<Group justify="space-between">
							<Text size="xs" c="dimmed">
								Cycle fit mode
							</Text>
							<Text size="xs">M</Text>
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
