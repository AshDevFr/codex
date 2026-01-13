import {
	Box,
	Divider,
	Grid,
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
 * Two-column layout on desktop, single column on mobile.
 * Left: Display settings (scale, background, layout)
 * Right: Mode-specific settings (transitions or scroll options)
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
			if (seriesId) {
				queryClient.invalidateQueries({ queryKey: ["seriesMetadata", seriesId] });
			}
		},
	});

	const handleReadingModeChange = (direction: ReadingDirection) => {
		setReadingDirectionOverride(direction);

		if (direction === "webtoon" && !["width", "original"].includes(settings.fitMode)) {
			setFitMode("width");
		}

		if (seriesId) {
			updateSeriesReadingDirection.mutate(direction);
		}
	};

	const isContinuousMode = settings.pageLayout === "continuous" || effectiveReadingDirection === "webtoon";

	return (
		<Modal opened={opened} onClose={onClose} title="Reader Settings" size="lg">
			<Stack gap="md">
				{/* General settings - full width at top */}
				<Box>
					<Text size="sm" fw={500} mb="xs">
						Reading mode
					</Text>
					<Select
						value={effectiveReadingDirection}
						onChange={(value) => value && handleReadingModeChange(value as ReadingDirection)}
						data={[
							{ label: "Left to Right", value: "ltr" },
							{ label: "Right to Left (Manga)", value: "rtl" },
							{ label: "Vertical", value: "ttb" },
							{ label: "Webtoon (Continuous Scroll)", value: "webtoon" },
						]}
					/>
					<Text size="xs" c="dimmed" mt={4}>
						{seriesId ? "Saved to series" : "Session only"}
					</Text>
				</Box>

				<Group justify="space-between">
					<Text size="sm" fw={500}>Auto-hide toolbar</Text>
					<Switch
						size="sm"
						checked={settings.autoHideToolbar}
						onChange={(e) => setAutoHideToolbar(e.currentTarget.checked)}
					/>
				</Group>

				<Divider />

				{/* Two-column layout */}
				<Grid gutter="xl">
					{/* Left Column: Display */}
					<Grid.Col span={{ base: 12, sm: 6 }}>
						<Stack gap="md">
							<Title order={6} c="dimmed">Display</Title>

							{/* Scale */}
							<Box>
								<Text size="sm" fw={500} mb="xs">
									Scale
								</Text>
								{isContinuousMode ? (
									<SegmentedControl
										fullWidth
										value={settings.fitMode === "width" || settings.fitMode === "original" ? settings.fitMode : "width"}
										onChange={(value) => setFitMode(value as FitMode)}
										data={[
											{ label: "Fit width", value: "width" },
											{ label: "Original", value: "original" },
										]}
									/>
								) : (
									<Select
										value={settings.fitMode}
										onChange={(value) => value && setFitMode(value as FitMode)}
										data={[
											{ label: "Fit screen", value: "screen" },
											{ label: "Fit width", value: "width" },
											{ label: "Fit width (shrink only)", value: "width-shrink" },
											{ label: "Fit height", value: "height" },
											{ label: "Original", value: "original" },
										]}
									/>
								)}
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

							{/* Page Layout - paginated only */}
							{!isContinuousMode && (
								<Box>
									<Text size="sm" fw={500} mb="xs">
										Page layout
									</Text>
									<SegmentedControl
										fullWidth
										value={settings.pageLayout === "continuous" ? "single" : settings.pageLayout}
										onChange={(value) => setPageLayout(value as PageLayout)}
										data={[
											{ label: "Single", value: "single" },
											{ label: "Double", value: "double" },
										]}
									/>
								</Box>
							)}

							{/* Double Page Options */}
							{!isContinuousMode && settings.pageLayout === "double" && (
								<Box
									p="sm"
									style={{
										backgroundColor: "var(--mantine-color-dark-7)",
										borderRadius: "var(--mantine-radius-sm)",
									}}
								>
									<Stack gap="xs">
										<Group justify="space-between">
											<Text size="sm">Wide pages alone</Text>
											<Switch
												size="sm"
												checked={settings.doublePageShowWideAlone}
												onChange={(e) => setDoublePageShowWideAlone(e.currentTarget.checked)}
											/>
										</Group>
										<Group justify="space-between">
											<Text size="sm">Start on odd page</Text>
											<Switch
												size="sm"
												checked={settings.doublePageStartOnOdd}
												onChange={(e) => setDoublePageStartOnOdd(e.currentTarget.checked)}
											/>
										</Group>
									</Stack>
								</Box>
							)}
						</Stack>
					</Grid.Col>

					{/* Right Column: Mode-specific options */}
					<Grid.Col span={{ base: 12, sm: 6 }}>
						<Stack gap="lg">
							{isContinuousMode ? (
								<>
									<Title order={6} c="dimmed">Scroll Options</Title>

									{/* Side Padding */}
									<Box pb="md">
										<Group justify="space-between" mb="xs">
											<Text size="sm" fw={500}>Side padding</Text>
											<Text size="xs" c="dimmed">{settings.webtoonSidePadding}%</Text>
										</Group>
										<Slider
											value={settings.webtoonSidePadding}
											onChange={setWebtoonSidePadding}
											min={0}
											max={40}
											step={5}
											marks={[
												{ value: 0, label: "0" },
												{ value: 20, label: "20" },
												{ value: 40, label: "40" },
											]}
										/>
									</Box>

									{/* Page Gap */}
									<Box pb="md">
										<Group justify="space-between" mb="xs">
											<Text size="sm" fw={500}>Page gap</Text>
											<Text size="xs" c="dimmed">{settings.webtoonPageGap}px</Text>
										</Group>
										<Slider
											value={settings.webtoonPageGap}
											onChange={setWebtoonPageGap}
											min={0}
											max={20}
											step={5}
											marks={[
												{ value: 0, label: "0" },
												{ value: 10, label: "10" },
												{ value: 20, label: "20" },
											]}
										/>
									</Box>
								</>
							) : (
								<>
									<Title order={6} c="dimmed">Transitions</Title>

									{/* Page Transitions */}
									<Box>
										<Text size="sm" fw={500} mb="xs">
											Page transitions
										</Text>
										<SegmentedControl
											fullWidth
											value={settings.pageTransition}
											onChange={(value) => setPageTransition(value as PageTransition)}
											data={[
												{ label: "None", value: "none" },
												{ label: "Fade", value: "fade" },
												{ label: "Slide", value: "slide" },
											]}
										/>
									</Box>

									{/* Transition Speed */}
									{settings.pageTransition !== "none" && (
										<Box pb="md">
											<Group justify="space-between" mb="xs">
												<Text size="sm" fw={500}>Speed</Text>
												<Text size="xs" c="dimmed">{settings.transitionDuration}ms</Text>
											</Group>
											<Slider
												value={settings.transitionDuration}
												onChange={setTransitionDuration}
												min={50}
												max={500}
												step={50}
												marks={[
													{ value: 50, label: "Fast" },
													{ value: 500, label: "Slow" },
												]}
											/>
										</Box>
									)}
								</>
							)}

							{/* Preload - common to both */}
							<Box pb="md">
								<Group justify="space-between" mb="xs">
									<Text size="sm" fw={500}>Preload pages</Text>
									<Text size="xs" c="dimmed" visibleFrom="sm">(Doubled for double-page layout)</Text>
									<Text size="xs" c="dimmed">{settings.preloadPages}</Text>
								</Group>
								<Slider
									value={settings.preloadPages}
									onChange={setPreloadPages}
									min={0}
									max={5}
									step={1}
									marks={[
										{ value: 0, label: "0" },
										{ value: 5, label: "5" },
									]}
								/>
							</Box>

						</Stack>
					</Grid.Col>
				</Grid>

				{/* Keyboard shortcuts - desktop only, compact */}
				<Divider visibleFrom="sm" />
				<Group justify="space-between" gap="xl" visibleFrom="sm">
					<Group gap="lg">
						<Text size="xs" c="dimmed">
							<Text span fw={500}>← → ↑ ↓</Text> {isContinuousMode ? "Scroll" : "Navigate"}
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>Home/End</Text> First/Last
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>F</Text> Fullscreen
						</Text>
					</Group>
					<Group gap="lg">
						<Text size="xs" c="dimmed">
							<Text span fw={500}>T</Text> Toolbar
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>M</Text> Cycle scale
						</Text>
						<Text size="xs" c="dimmed">
							<Text span fw={500}>Esc</Text> Close
						</Text>
					</Group>
				</Group>
			</Stack>
		</Modal>
	);
}
