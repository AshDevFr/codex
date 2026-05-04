import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Group,
  Stack,
  Text,
  Tooltip,
  UnstyledButton,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconCheck,
  IconChevronDown,
  IconChevronLeft,
  IconChevronRight,
  IconPackage,
  IconPlus,
} from "@tabler/icons-react";
import { useCallback, useRef, useState } from "react";
import type { PluginDto } from "@/api/plugins";
import classes from "./OfficialPlugins.module.css";
import type { PluginFormValues } from "./PluginForm";

// ---------------------------------------------------------------------------
// Official plugin registry
// ---------------------------------------------------------------------------

export interface OfficialPlugin {
  /** Unique slug name used as the plugin identifier */
  name: string;
  /** Human-readable display name */
  displayName: string;
  /** Short description of what the plugin does */
  description: string;
  /** Plugin type badge */
  type: "Metadata" | "Sync" | "Recommendations" | "Releases";
  /** npm package name */
  packageName: string;
  /** Short auth requirement description shown on the back face */
  authInfo: string;
  /** Plugin author */
  author: string;
  /** Whether this runs per-user or system-wide */
  scope: "user" | "system";
  /** Pre-filled form values for one-click creation */
  formDefaults: Pick<
    PluginFormValues,
    "command" | "args" | "credentialDelivery"
  > &
    Partial<Pick<PluginFormValues, "credentials">>;
}

export const OFFICIAL_PLUGINS: OfficialPlugin[] = [
  {
    name: "metadata-echo",
    displayName: "Echo Metadata",
    description:
      "Development and testing plugin that echoes back sample metadata. Useful for verifying the plugin system is working correctly and for plugin development.",
    type: "Metadata",
    packageName: "@ashdev/codex-plugin-metadata-echo",
    authInfo: "No authentication required",
    author: "Codex Team",
    scope: "system",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-metadata-echo",
      credentialDelivery: "env",
    },
  },
  {
    name: "metadata-mangabaka",
    displayName: "Mangabaka Metadata",
    description:
      "Fetch manga metadata from MangaBaka, an aggregation service that combines data from multiple sources including MangaDex, AniList, and MyAnimeList for comprehensive coverage.",
    type: "Metadata",
    packageName: "@ashdev/codex-plugin-metadata-mangabaka",
    authInfo: "API key required for setup",
    author: "Codex Team",
    scope: "system",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-metadata-mangabaka",
      credentialDelivery: "init_message",
      credentials: '{\n  api_key: "",\n}',
    },
  },
  {
    name: "metadata-openlibrary",
    displayName: "Open Library Metadata",
    description:
      "Fetch book metadata from Open Library using ISBN or title search. Covers millions of books with rich bibliographic data.",
    type: "Metadata",
    packageName: "@ashdev/codex-plugin-metadata-openlibrary",
    authInfo: "No authentication required",
    author: "Codex Team",
    scope: "system",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-metadata-openlibrary",
      credentialDelivery: "env",
    },
  },
  {
    name: "sync-anilist",
    displayName: "AniList Sync",
    description:
      "Bidirectional manga reading progress sync between Codex and AniList. Keeps your reading status, chapter progress, and scores in sync across both platforms.",
    type: "Sync",
    packageName: "@ashdev/codex-plugin-sync-anilist",
    authInfo: "OAuth or API key per user at runtime",
    author: "Codex Team",
    scope: "user",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-sync-anilist",
      credentialDelivery: "env",
    },
  },
  {
    name: "recommendations-anilist",
    displayName: "AniList Recommendations",
    description:
      "Personalized manga recommendations powered by AniList. Analyzes your library to suggest new series based on your reading history, genres, and community ratings.",
    type: "Recommendations",
    packageName: "@ashdev/codex-plugin-recommendations-anilist",
    authInfo: "OAuth or API key per user at runtime",
    author: "Codex Team",
    scope: "user",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-recommendations-anilist",
      credentialDelivery: "env",
    },
  },
  {
    name: "release-mangaupdates",
    displayName: "MangaUpdates Releases",
    description:
      "Announces new chapter and volume releases for tracked series via MangaUpdates per-series RSS feeds. Multi-language support filtered by per-series language preferences. Notify-only — Codex does not download anything.",
    type: "Releases",
    packageName: "@ashdev/codex-plugin-release-mangaupdates",
    authInfo: "No authentication required",
    author: "Codex Team",
    scope: "system",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-release-mangaupdates",
      credentialDelivery: "env",
    },
  },
  {
    name: "release-nyaa",
    displayName: "Nyaa Releases",
    description:
      "Announces new chapter and volume torrents for tracked series via Nyaa.si uploader RSS feeds. Limited to an admin-configured uploader allowlist; matches via title aliases. Notify-only — Codex does not download anything.",
    type: "Releases",
    packageName: "@ashdev/codex-plugin-release-nyaa",
    authInfo: "No authentication required",
    author: "Codex Team",
    scope: "system",
    formDefaults: {
      command: "npx",
      args: "-y\n@ashdev/codex-plugin-release-nyaa",
      credentialDelivery: "env",
    },
  },
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const pluginTypeBadgeColors: Record<OfficialPlugin["type"], string> = {
  Metadata: "blue",
  Sync: "teal",
  Recommendations: "grape",
  Releases: "orange",
};

const credentialLabels: Record<string, string> = {
  env: "Environment Variables",
  init_message: "Init Message",
  none: "None",
};

const CARD_WIDTH = 270;
const CARD_HEIGHT = 220;

interface PluginFlipCardProps {
  plugin: OfficialPlugin;
  isInstalled: boolean;
  onAdd: (plugin: OfficialPlugin) => void;
}

function PluginFlipCard({ plugin, isInstalled, onAdd }: PluginFlipCardProps) {
  return (
    <div
      className={classes.flipCard}
      style={{ width: CARD_WIDTH, minWidth: CARD_WIDTH, height: CARD_HEIGHT }}
    >
      <div className={classes.flipInner}>
        {/* Front face */}
        <div className={classes.flipFace}>
          <Stack gap={6} justify="space-between" h="100%">
            <Box>
              <Group gap="xs" mb={4}>
                <Badge
                  size="xs"
                  variant="light"
                  color={pluginTypeBadgeColors[plugin.type]}
                >
                  {plugin.type}
                </Badge>
                {isInstalled && (
                  <Badge size="xs" variant="light" color="green">
                    Installed
                  </Badge>
                )}
              </Group>
              <Text fw={500} size="sm" lineClamp={1}>
                {plugin.displayName}
              </Text>
              <Text size="xs" c="dimmed" lineClamp={5} mt={2}>
                {plugin.description}
              </Text>
            </Box>
            <Text size="xs" c="dimmed">
              by {plugin.author}
            </Text>
          </Stack>
        </div>

        {/* Back face */}
        <div className={`${classes.flipFace} ${classes.flipBack}`}>
          <Stack gap={6} justify="space-between" h="100%">
            <Box>
              <Text size="xs" fw={500} c="dimmed" mb={2}>
                Package
              </Text>
              <Tooltip label={plugin.packageName} openDelay={300}>
                <Anchor
                  href={`https://www.npmjs.com/package/${plugin.packageName}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  size="xs"
                  ff="monospace"
                  lineClamp={1}
                >
                  {plugin.packageName}
                </Anchor>
              </Tooltip>
              <Text size="xs" fw={500} c="dimmed" mt={6} mb={2}>
                Credentials
              </Text>
              <Text size="xs">
                {credentialLabels[plugin.formDefaults.credentialDelivery] ??
                  plugin.formDefaults.credentialDelivery}
              </Text>
              <Text size="xs" fw={500} c="dimmed" mt={6} mb={2}>
                Auth
              </Text>
              <Text size="xs">{plugin.authInfo}</Text>
              <Group gap="xs" mt={6}>
                <Badge
                  size="xs"
                  variant="dot"
                  color={plugin.scope === "system" ? "blue" : "orange"}
                >
                  {plugin.scope === "system" ? "System" : "Per-user"}
                </Badge>
              </Group>
            </Box>
            <Group justify="flex-end">
              {isInstalled ? (
                <Button
                  size="compact-xs"
                  variant="light"
                  color="green"
                  leftSection={<IconCheck size={14} />}
                  disabled
                >
                  Added
                </Button>
              ) : (
                <Button
                  size="compact-xs"
                  variant="light"
                  leftSection={<IconPlus size={14} />}
                  onClick={() => onAdd(plugin)}
                >
                  Add
                </Button>
              )}
            </Group>
          </Stack>
        </div>
      </div>
    </div>
  );
}

interface OfficialPluginsProps {
  /** Currently installed plugins to check for "already added" state */
  installedPlugins: PluginDto[];
  /** Callback when user clicks "Add" on an official plugin */
  onAdd: (plugin: OfficialPlugin) => void;
}

export function OfficialPlugins({
  installedPlugins,
  onAdd,
}: OfficialPluginsProps) {
  const [opened, { toggle }] = useDisclosure(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(true);

  // Build a set of installed plugin names for fast lookup
  const installedNames = new Set(installedPlugins.map((p) => p.name));

  const updateScrollState = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 0);
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  }, []);

  const scroll = useCallback((direction: "left" | "right") => {
    const el = scrollRef.current;
    if (!el) return;
    const amount = CARD_WIDTH * 2;
    el.scrollTo({
      left:
        direction === "left" ? el.scrollLeft - amount : el.scrollLeft + amount,
      behavior: "smooth",
    });
  }, []);

  return (
    <Card withBorder>
      <UnstyledButton onClick={toggle} w="100%">
        <Group justify="space-between">
          <Group gap="sm">
            <IconPackage size={20} />
            <div>
              <Text fw={500}>Official Plugins</Text>
              <Text size="xs" c="dimmed">
                Pre-configured plugins maintained by the Codex team
              </Text>
            </div>
          </Group>
          {opened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
        </Group>
      </UnstyledButton>

      <Collapse in={opened}>
        <Box mt="sm" pos="relative">
          {/* Scroll arrows */}
          {canScrollLeft && (
            <ActionIcon
              variant="filled"
              size="sm"
              radius="xl"
              pos="absolute"
              left={-6}
              top="50%"
              style={{ transform: "translateY(-50%)", zIndex: 2 }}
              onClick={() => scroll("left")}
              aria-label="Scroll left"
            >
              <IconChevronLeft size={14} />
            </ActionIcon>
          )}
          {canScrollRight && (
            <ActionIcon
              variant="filled"
              size="sm"
              radius="xl"
              pos="absolute"
              right={-6}
              top="50%"
              style={{ transform: "translateY(-50%)", zIndex: 2 }}
              onClick={() => scroll("right")}
              aria-label="Scroll right"
            >
              <IconChevronRight size={14} />
            </ActionIcon>
          )}

          {/* Scrollable track */}
          <div
            ref={scrollRef}
            onScroll={updateScrollState}
            style={{
              overflowX: "auto",
              overflowY: "hidden",
              scrollbarWidth: "none",
              msOverflowStyle: "none",
            }}
          >
            <Group gap="sm" wrap="nowrap" pb={4}>
              {OFFICIAL_PLUGINS.map((plugin) => (
                <PluginFlipCard
                  key={plugin.name}
                  plugin={plugin}
                  isInstalled={installedNames.has(plugin.name)}
                  onAdd={onAdd}
                />
              ))}
            </Group>
          </div>
        </Box>
      </Collapse>
    </Card>
  );
}
