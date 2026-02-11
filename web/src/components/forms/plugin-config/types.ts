import type { UseFormReturnType } from "@mantine/form";
import type { PluginDto } from "@/api/plugins";
import { AVAILABLE_PERMISSIONS, AVAILABLE_SCOPES } from "@/api/plugins";
import { SAMPLE_SERIES_CONTEXT } from "@/utils/templateUtils";

// =============================================================================
// Capability detection helpers
// =============================================================================

export function isMetadataProvider(plugin: PluginDto): boolean {
  return (plugin.manifest?.capabilities?.metadataProvider?.length ?? 0) > 0;
}

export function isSyncProvider(plugin: PluginDto): boolean {
  return plugin.manifest?.capabilities?.userReadSync === true;
}

export function isOAuthPlugin(plugin: PluginDto): boolean {
  return plugin.manifest?.oauth != null;
}

export function hasManifest(plugin: PluginDto): boolean {
  return plugin.manifest != null;
}

// =============================================================================
// Form types
// =============================================================================

export type MetadataTarget = "series" | "book";

export interface PluginConfigFormValues {
  // Permissions & Access
  permissions: string[];
  scopes: string[];
  allLibraries: boolean;
  libraryIds: string[];
  // Search config (metadata providers only)
  searchQueryTemplate: string;
  searchResultsLimit: number | null;
  useExistingExternalId: boolean;
  metadataTargets: MetadataTarget[];
  // OAuth config (OAuth plugins only)
  oauthClientId: string;
  oauthClientSecret: string;
}

export type PluginConfigForm = UseFormReturnType<PluginConfigFormValues>;

// =============================================================================
// Permission grouping by capability
// =============================================================================

const METADATA_PERMISSION_VALUES = new Set(
  AVAILABLE_PERMISSIONS.filter((p) => p.value.startsWith("metadata:")).map(
    (p) => p.value,
  ),
);

const LIBRARY_PERMISSION_VALUES = new Set(
  AVAILABLE_PERMISSIONS.filter((p) => p.value.startsWith("library:")).map(
    (p) => p.value,
  ),
);

export function getPermissionData(plugin: PluginDto) {
  const isMeta = isMetadataProvider(plugin);
  const isSync = isSyncProvider(plugin);
  const noManifest = !hasManifest(plugin);

  if (noManifest) {
    return {
      data: [
        {
          group: "Metadata",
          items: AVAILABLE_PERMISSIONS.filter((p) =>
            METADATA_PERMISSION_VALUES.has(p.value),
          ).map((p) => ({ value: p.value, label: p.label })),
        },
        {
          group: "Library",
          items: AVAILABLE_PERMISSIONS.filter((p) =>
            LIBRARY_PERMISSION_VALUES.has(p.value),
          ).map((p) => ({ value: p.value, label: p.label })),
        },
      ],
      showNoManifestWarning: true,
    };
  }

  const groups: { group: string; items: { value: string; label: string }[] }[] =
    [];

  if (isMeta) {
    groups.push({
      group: "Metadata",
      items: AVAILABLE_PERMISSIONS.filter((p) =>
        METADATA_PERMISSION_VALUES.has(p.value),
      ).map((p) => ({ value: p.value, label: p.label })),
    });
  }

  if (isSync || isMeta) {
    groups.push({
      group: "Library",
      items: AVAILABLE_PERMISSIONS.filter((p) =>
        LIBRARY_PERMISSION_VALUES.has(p.value),
      ).map((p) => ({ value: p.value, label: p.label })),
    });
  }

  return { data: groups, showNoManifestWarning: false };
}

// =============================================================================
// Scope filtering by capability
// =============================================================================

// Mirrors backend PluginScope::series_scopes()
const SERIES_SCOPES = new Set([
  "series:detail",
  "series:bulk",
  "library:detail",
  "library:scan",
]);

// Mirrors backend PluginScope::book_scopes()
const BOOK_SCOPES = new Set([
  "book:detail",
  "book:bulk",
  "library:detail",
  "library:scan",
]);

// Sync providers operate at series/library level
const SYNC_SCOPES = new Set([
  "series:detail",
  "library:detail",
  "library:scan",
]);

export function getScopeData(plugin: PluginDto) {
  const noManifest = !hasManifest(plugin);

  if (noManifest) {
    return AVAILABLE_SCOPES.map((s) => ({ value: s.value, label: s.label }));
  }

  const metadataTargets = plugin.manifest?.capabilities?.metadataProvider ?? [];
  const canSeries = metadataTargets.includes("series");
  const canBook = metadataTargets.includes("book");
  const isSync = isSyncProvider(plugin);

  const allowed = new Set<string>();
  if (canSeries) for (const s of SERIES_SCOPES) allowed.add(s);
  if (canBook) for (const s of BOOK_SCOPES) allowed.add(s);
  if (isSync) for (const s of SYNC_SCOPES) allowed.add(s);

  return AVAILABLE_SCOPES.filter((s) => allowed.has(s.value)).map((s) => ({
    value: s.value,
    label: s.label,
  }));
}

// =============================================================================
// Template helpers
// =============================================================================

export const TEMPLATE_HELPERS = [
  {
    name: "clean",
    example: "{{clean metadata.title}}",
    description: "Remove noise (Digital, year, etc.)",
  },
  {
    name: "truncate",
    example: "{{truncate metadata.title 50}}",
    description: "Limit to N characters",
  },
  {
    name: "first_word",
    example: "{{first_word metadata.title}}",
    description: "First word only",
  },
  {
    name: "lowercase",
    example: "{{lowercase metadata.title}}",
    description: "Convert to lowercase",
  },
] as const;

export function renderTemplatePreview(template: string): string {
  if (!template.trim()) return "(default: series title)";

  let preview = template;
  const ctx = SAMPLE_SERIES_CONTEXT;
  const meta = ctx.metadata;

  preview = preview.replace(/\{\{bookCount\}\}/g, String(ctx.bookCount ?? 0));
  preview = preview.replace(/\{\{seriesId\}\}/g, ctx.seriesId ?? "");

  preview = preview.replace(/\{\{metadata\.title\}\}/g, meta?.title ?? "");
  preview = preview.replace(
    /\{\{metadata\.titleSort\}\}/g,
    meta?.titleSort ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.year\}\}/g,
    String(meta?.year ?? ""),
  );
  preview = preview.replace(
    /\{\{metadata\.publisher\}\}/g,
    meta?.publisher ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.language\}\}/g,
    meta?.language ?? "",
  );
  preview = preview.replace(/\{\{metadata\.status\}\}/g, meta?.status ?? "");
  preview = preview.replace(
    /\{\{metadata\.ageRating\}\}/g,
    String(meta?.ageRating ?? ""),
  );
  preview = preview.replace(
    /\{\{metadata\.genres\}\}/g,
    meta?.genres?.join(", ") ?? "",
  );
  preview = preview.replace(
    /\{\{metadata\.tags\}\}/g,
    meta?.tags?.join(", ") ?? "",
  );

  preview = preview.replace(/\{\{clean metadata\.title\}\}/g, "One Piece");
  preview = preview.replace(
    /\{\{truncate metadata\.title \d+\}\}/g,
    "One Piece (D...",
  );
  preview = preview.replace(/\{\{first_word metadata\.title\}\}/g, "One");
  preview = preview.replace(
    /\{\{lowercase metadata\.title\}\}/g,
    "one piece (digital)",
  );

  preview = preview.replace(/\{\{#if [\w.]+\}\}(.*?)\{\{\/if\}\}/g, "$1");
  preview = preview.replace(/\{\{#unless [\w.]+\}\}(.*?)\{\{\/unless\}\}/g, "");

  return preview || "(empty)";
}
