import { Button, Group } from "@mantine/core";
import { IconExternalLink } from "@tabler/icons-react";
import type { WebLinkProviderDto } from "@/api/plugins";
import { MetadataLabel } from "@/components/common";
import { usePluginWebLinks } from "@/hooks/usePluginWebLinks";

/** Minimal external-ID shape needed to match series links. */
export interface WebLinkExternalId {
  source: string;
  externalId: string;
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

/**
 * Build the URL a provider's button should open for a series.
 *
 * Walks `seriesLinks` in declaration order and uses the first entry whose
 * source the series has an external ID for (direct link); otherwise falls
 * back to the search template with the series title. Runtime values are
 * URL-encoded.
 */
export function buildWebLinkUrl(
  provider: WebLinkProviderDto,
  title: string,
  externalIds: WebLinkExternalId[],
): string {
  const idsBySource = new Map<string, string>();
  // First occurrence wins so earlier rows (e.g. `api:` over `plugin:`
  // duplicates) stay stable regardless of later entries.
  for (const externalId of externalIds) {
    const key = bareSource(externalId.source);
    if (!idsBySource.has(key)) {
      idsBySource.set(key, externalId.externalId);
    }
  }

  for (const link of provider.seriesLinks) {
    const id = idsBySource.get(link.source.toLowerCase());
    if (id !== undefined && id !== "") {
      return link.urlTemplate.replaceAll(
        "{externalId}",
        encodeURIComponent(id),
      );
    }
  }

  return provider.searchUrlTemplate.replaceAll(
    "{title}",
    encodeURIComponent(title),
  );
}

interface PluginWebLinksProps {
  /** Series display title, used for the search fallback. */
  title: string;
  /** The series' external IDs (raw stored sources, any namespace). */
  externalIds: WebLinkExternalId[];
}

/**
 * "Open on <site>" buttons for every plugin declaring the `webLinks`
 * capability, rendered as a labeled metadata row matching the detail page's
 * other rows. Renders nothing when no provider is configured, so the row
 * never shows as an empty label.
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
        {providers.map((provider) => (
          <Button
            key={provider.pluginName}
            component="a"
            href={buildWebLinkUrl(provider, title, externalIds)}
            target="_blank"
            rel="noopener noreferrer"
            variant="light"
            size="compact-xs"
            rightSection={<IconExternalLink size={12} />}
          >
            {provider.displayName}
          </Button>
        ))}
      </Group>
    </Group>
  );
}
