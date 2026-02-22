import {
  Alert,
  Box,
  Button,
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
import { IconBookmark, IconRefresh } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { seriesMetadataApi } from "@/api/seriesMetadata";
import {
  type BackgroundColor,
  type FitMode,
  type PageLayout,
  type PageTransition,
  type PdfMode,
  type ReadingDirection,
  selectEffectiveReadingDirection,
  useReaderStore,
} from "@/store/readerStore";
import { useSeriesReaderSettings } from "./hooks/useSeriesReaderSettings";

interface ReaderSettingsProps {
  /** Whether the modal is open */
  opened: boolean;
  /** Callback when modal is closed */
  onClose: () => void;
  /** Series ID for updating reading direction (optional) */
  seriesId?: string | null;
  /** Book format (CBZ, CBR, PDF, EPUB) - enables format-specific settings */
  format?: string;
}

/**
 * Settings modal for the comic reader.
 *
 * Two-column layout on desktop, single column on mobile.
 * Left: Display settings (scale, background, layout)
 * Right: Mode-specific settings (transitions or scroll options)
 *
 * Supports per-series settings override. When viewing a series, users can
 * customize settings that will only apply to that series.
 */
export function ReaderSettings({
  opened,
  onClose,
  seriesId,
  format,
}: ReaderSettingsProps) {
  const queryClient = useQueryClient();
  const globalSettings = useReaderStore((state) => state.settings);
  const effectiveReadingDirection = useReaderStore(
    selectEffectiveReadingDirection,
  );
  const pdfMode = useReaderStore((state) => state.settings.pdfMode);
  const setPdfMode = useReaderStore((state) => state.setPdfMode);

  // Global-only settings (not forkable per-series)
  const setAutoHideToolbar = useReaderStore(
    (state) => state.setAutoHideToolbar,
  );
  const setPreloadPages = useReaderStore((state) => state.setPreloadPages);
  const setPageTransition = useReaderStore((state) => state.setPageTransition);
  const setTransitionDuration = useReaderStore(
    (state) => state.setTransitionDuration,
  );
  const setWebtoonSidePadding = useReaderStore(
    (state) => state.setWebtoonSidePadding,
  );
  const setWebtoonPageGap = useReaderStore((state) => state.setWebtoonPageGap);
  const setAutoAdvanceToNextBook = useReaderStore(
    (state) => state.setAutoAdvanceToNextBook,
  );

  // Global setters for reading direction override (used when updating series metadata)
  const setReadingDirectionOverride = useReaderStore(
    (state) => state.setReadingDirectionOverride,
  );
  const setGlobalFitMode = useReaderStore((state) => state.setFitMode);

  // Per-series settings hook
  const {
    hasSeriesOverride,
    effectiveSettings,
    forkToSeries,
    resetToGlobal,
    updateSetting,
  } = useSeriesReaderSettings(seriesId);

  // Mutation to update series reading direction in backend
  // Also locks the reading direction field to prevent scans/imports from overwriting it
  const updateSeriesReadingDirection = useMutation({
    mutationFn: async (direction: ReadingDirection) => {
      if (!seriesId) {
        return;
      }
      await seriesMetadataApi.patchMetadata(seriesId, {
        readingDirection: direction,
      });
      await seriesMetadataApi.updateLocks(seriesId, {
        readingDirection: true,
      });
    },
    onSuccess: () => {
      if (seriesId) {
        queryClient.invalidateQueries({
          queryKey: ["seriesMetadata", seriesId],
        });
      }
    },
  });

  const handleReadingModeChange = (direction: ReadingDirection) => {
    // Update the reading direction override in global store for immediate effect
    setReadingDirectionOverride(direction);

    // If we have series context with an existing override, save to series-specific settings
    if (seriesId && hasSeriesOverride) {
      updateSetting("readingDirection", direction);

      // Auto-adjust fit mode for webtoon
      if (
        direction === "webtoon" &&
        !["width", "original"].includes(effectiveSettings.fitMode)
      ) {
        updateSetting("fitMode", "width");
      }
    } else {
      // No series override - adjust fit mode in global store if needed
      if (
        direction === "webtoon" &&
        !["width", "original"].includes(globalSettings.fitMode)
      ) {
        setGlobalFitMode("width");
      }
    }

    // Persist to backend if we have series context (always, regardless of override)
    if (seriesId) {
      updateSeriesReadingDirection.mutate(direction);
    }
  };

  const handleFitModeChange = (fitMode: FitMode) => {
    // Only save to series override if one already exists (explicitly forked)
    if (seriesId && hasSeriesOverride) {
      updateSetting("fitMode", fitMode);
    } else {
      setGlobalFitMode(fitMode);
    }
  };

  const handleBackgroundChange = (bg: BackgroundColor) => {
    // Only save to series override if one already exists (explicitly forked)
    if (seriesId && hasSeriesOverride) {
      updateSetting("backgroundColor", bg);
    } else {
      useReaderStore.getState().setBackgroundColor(bg);
    }
  };

  const handlePageLayoutChange = (layout: PageLayout) => {
    // Only save to series override if one already exists (explicitly forked)
    if (seriesId && hasSeriesOverride) {
      updateSetting("pageLayout", layout);
    } else {
      useReaderStore.getState().setPageLayout(layout);
    }
  };

  const handleDoublePageWideAloneChange = (checked: boolean) => {
    // Only save to series override if one already exists (explicitly forked)
    if (seriesId && hasSeriesOverride) {
      updateSetting("doublePageShowWideAlone", checked);
    } else {
      useReaderStore.getState().setDoublePageShowWideAlone(checked);
    }
  };

  const handleDoublePageStartOnOddChange = (checked: boolean) => {
    // Only save to series override if one already exists (explicitly forked)
    if (seriesId && hasSeriesOverride) {
      updateSetting("doublePageStartOnOdd", checked);
    } else {
      useReaderStore.getState().setDoublePageStartOnOdd(checked);
    }
  };

  // Determine which settings to use for display
  const displaySettings = seriesId ? effectiveSettings : globalSettings;

  const isContinuousMode =
    displaySettings.pageLayout === "continuous" ||
    effectiveReadingDirection === "webtoon";

  return (
    <Modal opened={opened} onClose={onClose} title="Reader Settings" size="lg">
      <Stack gap="md">
        {/* Series-specific settings banner */}
        {seriesId && hasSeriesOverride && (
          <Alert
            variant="light"
            color="blue"
            icon={<IconBookmark size={16} />}
            title="Using series-specific settings"
            styles={{
              root: { paddingBlock: "var(--mantine-spacing-xs)" },
              title: { marginBottom: 0 },
            }}
          >
            <Group justify="space-between" align="center" mt="xs">
              <Text size="sm" c="dimmed">
                Display settings are customized for this series
              </Text>
              <Button
                size="xs"
                variant="subtle"
                color="gray"
                leftSection={<IconRefresh size={14} />}
                onClick={resetToGlobal}
              >
                Reset to global
              </Button>
            </Group>
          </Alert>
        )}

        {/* General settings - full width at top */}
        <Box>
          <Text size="sm" fw={500} mb="xs">
            Reading mode
          </Text>
          <Select
            value={effectiveReadingDirection}
            onChange={(value) =>
              value && handleReadingModeChange(value as ReadingDirection)
            }
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

        {/* PDF rendering mode toggle - only shown for PDF format */}
        {format?.toUpperCase() === "PDF" && (
          <>
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
            <Divider />
          </>
        )}

        <Group justify="space-between">
          <Text size="sm" fw={500}>
            Auto-hide toolbar
          </Text>
          <Switch
            size="sm"
            checked={globalSettings.autoHideToolbar}
            onChange={(e) => setAutoHideToolbar(e.currentTarget.checked)}
          />
        </Group>

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
            size="sm"
            checked={globalSettings.autoAdvanceToNextBook}
            onChange={(e) => setAutoAdvanceToNextBook(e.currentTarget.checked)}
          />
        </Group>

        <Divider />

        {/* Two-column layout */}
        <Grid gutter="xl">
          {/* Left Column: Display (Forkable settings) */}
          <Grid.Col span={{ base: 12, sm: 6 }}>
            <Box
              p="sm"
              style={{
                backgroundColor:
                  seriesId && hasSeriesOverride
                    ? "var(--mantine-color-blue-light)"
                    : undefined,
                borderRadius: "var(--mantine-radius-sm)",
                marginInline: "calc(-1 * var(--mantine-spacing-sm))",
              }}
            >
              <Stack gap="md">
                <Group justify="space-between" align="center">
                  <Title order={6} c="dimmed">
                    Display
                  </Title>
                  {seriesId && hasSeriesOverride && (
                    <Text size="xs" c="blue">
                      Series
                    </Text>
                  )}
                </Group>

                {/* Scale */}
                <Box>
                  <Text size="sm" fw={500} mb="xs">
                    Scale
                  </Text>
                  {isContinuousMode ? (
                    <SegmentedControl
                      fullWidth
                      value={
                        displaySettings.fitMode === "width" ||
                        displaySettings.fitMode === "original"
                          ? displaySettings.fitMode
                          : "width"
                      }
                      onChange={(value) =>
                        handleFitModeChange(value as FitMode)
                      }
                      data={[
                        { label: "Fit width", value: "width" },
                        { label: "Original", value: "original" },
                      ]}
                    />
                  ) : (
                    <Select
                      value={displaySettings.fitMode}
                      onChange={(value) =>
                        value && handleFitModeChange(value as FitMode)
                      }
                      data={[
                        { label: "Fit screen", value: "screen" },
                        { label: "Fit width", value: "width" },
                        {
                          label: "Fit width (shrink only)",
                          value: "width-shrink",
                        },
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
                    value={displaySettings.backgroundColor}
                    onChange={(value) =>
                      handleBackgroundChange(value as BackgroundColor)
                    }
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
                      value={
                        displaySettings.pageLayout === "continuous"
                          ? "single"
                          : displaySettings.pageLayout
                      }
                      onChange={(value) =>
                        handlePageLayoutChange(value as PageLayout)
                      }
                      data={[
                        { label: "Single", value: "single" },
                        { label: "Double", value: "double" },
                      ]}
                    />
                  </Box>
                )}

                {/* Double Page Options */}
                {!isContinuousMode &&
                  displaySettings.pageLayout === "double" && (
                    <Box
                      p="sm"
                      style={{
                        backgroundColor: "var(--mantine-color-dark-7)",
                        borderRadius: "var(--mantine-radius-sm)",
                      }}
                    >
                      <Stack gap="xs">
                        <Box>
                          <Group justify="space-between">
                            <Text size="sm">Wide pages alone</Text>
                            <Switch
                              size="sm"
                              checked={displaySettings.doublePageShowWideAlone}
                              onChange={(e) =>
                                handleDoublePageWideAloneChange(
                                  e.currentTarget.checked,
                                )
                              }
                            />
                          </Group>
                          {displaySettings.doublePageShowWideAlone && (
                            <Text size="xs" c="dimmed" mt={4}>
                              Wide pages define spread boundaries
                            </Text>
                          )}
                        </Box>
                        <Box>
                          <Group justify="space-between">
                            <Text size="sm">
                              {displaySettings.doublePageShowWideAlone
                                ? "Cover page alone"
                                : "Start on odd page"}
                            </Text>
                            <Switch
                              size="sm"
                              checked={displaySettings.doublePageStartOnOdd}
                              onChange={(e) =>
                                handleDoublePageStartOnOddChange(
                                  e.currentTarget.checked,
                                )
                              }
                            />
                          </Group>
                          {displaySettings.doublePageStartOnOdd &&
                            !displaySettings.doublePageShowWideAlone && (
                              <Text size="xs" c="dimmed" mt={4}>
                                Page 1 alone, then 2-3, 4-5, etc.
                              </Text>
                            )}
                        </Box>
                      </Stack>
                    </Box>
                  )}
              </Stack>
            </Box>
          </Grid.Col>

          {/* Right Column: Mode-specific options (Global settings) */}
          <Grid.Col span={{ base: 12, sm: 6 }}>
            <Stack gap="lg">
              {isContinuousMode ? (
                <>
                  <Title order={6} c="dimmed">
                    Scroll Options
                  </Title>

                  {/* Side Padding */}
                  <Box pb="md">
                    <Group justify="space-between" mb="xs">
                      <Text size="sm" fw={500}>
                        Side padding
                      </Text>
                      <Text size="xs" c="dimmed">
                        {globalSettings.webtoonSidePadding}%
                      </Text>
                    </Group>
                    <Slider
                      value={globalSettings.webtoonSidePadding}
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
                      <Text size="sm" fw={500}>
                        Page gap
                      </Text>
                      <Text size="xs" c="dimmed">
                        {globalSettings.webtoonPageGap}px
                      </Text>
                    </Group>
                    <Slider
                      value={globalSettings.webtoonPageGap}
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
                  <Title order={6} c="dimmed">
                    Transitions
                  </Title>

                  {/* Page Transitions */}
                  <Box>
                    <Text size="sm" fw={500} mb="xs">
                      Page transitions
                    </Text>
                    <SegmentedControl
                      fullWidth
                      value={globalSettings.pageTransition}
                      onChange={(value) =>
                        setPageTransition(value as PageTransition)
                      }
                      data={[
                        { label: "None", value: "none" },
                        { label: "Fade", value: "fade" },
                        { label: "Slide", value: "slide" },
                      ]}
                    />
                  </Box>

                  {/* Transition Speed */}
                  {globalSettings.pageTransition !== "none" && (
                    <Box pb="md">
                      <Group justify="space-between" mb="xs">
                        <Text size="sm" fw={500}>
                          Speed
                        </Text>
                        <Text size="xs" c="dimmed">
                          {globalSettings.transitionDuration}ms
                        </Text>
                      </Group>
                      <Slider
                        value={globalSettings.transitionDuration}
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
                  <Text size="sm" fw={500}>
                    Preload pages
                  </Text>
                  <Text size="xs" c="dimmed" visibleFrom="sm">
                    (Doubled for double-page layout)
                  </Text>
                  <Text size="xs" c="dimmed">
                    {globalSettings.preloadPages}
                  </Text>
                </Group>
                <Slider
                  value={globalSettings.preloadPages}
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

        {/* Fork button - shown when series context exists but no override yet */}
        {seriesId && !hasSeriesOverride && (
          <Box
            p="sm"
            style={{
              backgroundColor: "var(--mantine-color-dark-6)",
              borderRadius: "var(--mantine-radius-sm)",
              textAlign: "center",
            }}
          >
            <Button
              variant="light"
              color="blue"
              leftSection={<IconBookmark size={16} />}
              onClick={forkToSeries}
            >
              Customize Settings for This Series
            </Button>
            <Text size="xs" c="dimmed" mt="xs">
              Display settings will be saved specifically for this series
            </Text>
          </Box>
        )}

        {/* Keyboard shortcuts - desktop only, compact */}
        <Divider visibleFrom="sm" />
        <Group justify="space-between" gap="xl" visibleFrom="sm">
          <Group gap="lg">
            <Text size="xs" c="dimmed">
              <Text span fw={500}>
                ← → ↑ ↓
              </Text>{" "}
              {isContinuousMode ? "Scroll" : "Navigate"}
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
                T
              </Text>{" "}
              Toolbar
            </Text>
            <Text size="xs" c="dimmed">
              <Text span fw={500}>
                M
              </Text>{" "}
              Cycle scale
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
