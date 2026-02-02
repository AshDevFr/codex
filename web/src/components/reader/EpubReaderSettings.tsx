import {
  Box,
  Divider,
  Grid,
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
    <Modal opened={opened} onClose={onClose} title="Reader Settings" size="lg">
      <Stack gap="md">
        {/* Two-column layout */}
        <Grid gutter="xl">
          {/* Left Column: Appearance */}
          <Grid.Col span={{ base: 12, sm: 6 }}>
            <Stack gap="md">
              {/* Theme */}
              <Box>
                <Text size="sm" fw={500} mb="xs">
                  Theme
                </Text>
                <Select
                  value={settings.epubTheme}
                  onChange={(value) =>
                    value && setEpubTheme(value as EpubTheme)
                  }
                  data={THEME_OPTIONS}
                  allowDeselect={false}
                />
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
              </Box>

              {/* Font Size */}
              <Box pb="md">
                <Group justify="space-between" mb="xs">
                  <Text size="sm" fw={500}>
                    Font Size
                  </Text>
                  <Text size="xs" c="dimmed">
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
                    { value: 200, label: "200%" },
                  ]}
                />
              </Box>
            </Stack>
          </Grid.Col>

          {/* Right Column: Typography & Layout */}
          <Grid.Col span={{ base: 12, sm: 6 }}>
            <Stack gap="md">
              {/* Line Height */}
              <Box pb="md">
                <Group justify="space-between" mb="xs">
                  <Text size="sm" fw={500}>
                    Line Spacing
                  </Text>
                  <Text size="xs" c="dimmed">
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
                    { value: 175, label: "Normal" },
                    { value: 250, label: "Loose" },
                  ]}
                />
              </Box>

              {/* Margins */}
              <Box pb="md">
                <Group justify="space-between" mb="xs">
                  <Text size="sm" fw={500}>
                    Margins
                  </Text>
                  <Text size="xs" c="dimmed">
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
                    { value: 15, label: "Normal" },
                    { value: 30, label: "Max" },
                  ]}
                />
              </Box>
            </Stack>
          </Grid.Col>
        </Grid>

        <Divider />

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

        {/* Keyboard shortcuts - desktop only, compact */}
        <Divider visibleFrom="sm" />
        <Group justify="space-between" gap="xl" visibleFrom="sm">
          <Group gap="lg">
            <Text size="xs" c="dimmed">
              <Text span fw={500}>
                ← →
              </Text>{" "}
              Navigate
            </Text>
            <Text size="xs" c="dimmed">
              <Text span fw={500}>
                T
              </Text>{" "}
              Contents
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
                Space
              </Text>{" "}
              Toolbar
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
