import { Button, Group, Menu } from "@mantine/core";
import {
  IconChevronDown,
  IconExternalLink,
  IconSearch,
} from "@tabler/icons-react";
import { capitalize } from "es-toolkit/string";
import type { WebLinkProviderDto } from "@/api/plugins";
import { MetadataLabel } from "@/components/common";
import { usePluginWebLinks } from "@/hooks/usePluginWebLinks";
import { SOURCE_CONFIG } from "./ExternalLinks";

/** Minimal external-ID shape needed to match series links. */
export interface WebLinkExternalId {
  source: string;
  externalId: string;
}

/** One concrete navigation target a provider offers for a series. */
export interface WebLinkOption {
  /** Bare source name for direct links, or `"search"` for the fallback. */
  key: string;
  /** Human-readable label shown in the dropdown. */
  label: string;
  url: string;
}

/**
 * Strip the storage namespace from an external-ID source so it matches the
 * bare Codex source names web-link templates are keyed by: both
 * `api:mangabaka` and `plugin:mangabaka` count as `mangabaka`. Mirrors the
 * host-side normalization used when handing external IDs to plugins.
 */
function bareSource(source: string): string {
  return source.replace(/^(api|plugin):/, "").toLowerCase();
}

function sourceLabel(source: string): string {
  return SOURCE_CONFIG[source.toLowerCase()]?.name ?? capitalize(source);
}

/**
 * Build every navigation target a provider offers for a series, in priority
 * order: matching direct series links first (in the provider's declared
 * order), then the title search as the final entry. The first element is the
 * primary button target; the rest feed the dropdown. Always non-empty, since
 * search is unconditional. Runtime values are URL-encoded.
 */
export function buildWebLinkOptions(
  provider: WebLinkProviderDto,
  title: string,
  externalIds: WebLinkExternalId[],
): WebLinkOption[] {
  const idsBySource = new Map<string, string>();
  // First occurrence wins so earlier rows (e.g. `api:` over `plugin:`
  // duplicates) stay stable regardless of later entries.
  for (const externalId of externalIds) {
    const key = bareSource(externalId.source);
    if (!idsBySource.has(key)) {
      idsBySource.set(key, externalId.externalId);
    }
  }

  const options: WebLinkOption[] = [];
  for (const link of provider.seriesLinks) {
    const id = idsBySource.get(link.source.toLowerCase());
    if (id !== undefined && id !== "") {
      options.push({
        key: link.source,
        label: sourceLabel(link.source),
        url: link.urlTemplate.replaceAll(
          "{externalId}",
          encodeURIComponent(id),
        ),
      });
    }
  }
  options.push({
    key: "search",
    label: "Search",
    url: provider.searchUrlTemplate.replaceAll(
      "{title}",
      encodeURIComponent(title),
    ),
  });
  return options;
}

interface PluginWebLinksProps {
  /** Series display title, used for the search fallback. */
  title: string;
  /** The series' external IDs (raw stored sources, any namespace). */
  externalIds: WebLinkExternalId[];
}

const externalLinkProps = {
  component: "a",
  target: "_blank",
  rel: "noopener noreferrer",
} as const;

/**
 * "Open on <site>" buttons for every plugin declaring the `webLinks`
 * capability, rendered as a labeled metadata row matching the detail page's
 * other rows. The primary button opens the best target (first matching
 * direct link, else search); when other targets exist they hang off a
 * chevron dropdown (alternate source links plus Search). Renders nothing
 * when no provider is configured, so the row never shows as an empty label.
 */
export function PluginWebLinks({ title, externalIds }: PluginWebLinksProps) {
  const { data } = usePluginWebLinks();
  const providers = data?.providers ?? [];

  if (providers.length === 0) {
    return null;
  }

  return (
    <Group gap="md" align="flex-start">
      <MetadataLabel>OPEN ON</MetadataLabel>
      <Group gap="xs">
        {providers.map((provider) => {
          const options = buildWebLinkOptions(provider, title, externalIds);
          const primary = options[0];
          // Direct links and the search fallback are separated by a divider
          // in the dropdown; search is always the builder's last entry.
          const directLinks = options.filter(
            (option) => option.key !== "search",
          );
          const search = options[options.length - 1];
          const primaryButton = (
            <Button
              {...externalLinkProps}
              href={primary.url}
              variant="light"
              size="compact-xs"
              rightSection={<IconExternalLink size={12} />}
            >
              {provider.displayName}
            </Button>
          );

          if (options.length === 1) {
            return <span key={provider.pluginName}>{primaryButton}</span>;
          }

          return (
            <Button.Group key={provider.pluginName}>
              {primaryButton}
              <Menu position="bottom-end" width="max-content">
                <Menu.Target>
                  <Button
                    variant="light"
                    size="compact-xs"
                    px={4}
                    aria-label={`More ${provider.displayName} links`}
                  >
                    <IconChevronDown size={12} />
                  </Button>
                </Menu.Target>
                <Menu.Dropdown>
                  {directLinks.map((option) => (
                    <Menu.Item
                      {...externalLinkProps}
                      key={option.key}
                      href={option.url}
                      leftSection={<IconExternalLink size={12} />}
                    >
                      {option.label}
                    </Menu.Item>
                  ))}
                  {directLinks.length > 0 && <Menu.Divider />}
                  <Menu.Item
                    {...externalLinkProps}
                    key={search.key}
                    href={search.url}
                    leftSection={<IconSearch size={12} />}
                  >
                    {search.label}
                  </Menu.Item>
                </Menu.Dropdown>
              </Menu>
            </Button.Group>
          );
        })}
      </Group>
    </Group>
  );
}
