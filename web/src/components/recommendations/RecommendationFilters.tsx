import {
  Badge,
  Collapse,
  Group,
  RangeSlider,
  SimpleGrid,
  Stack,
  Text,
  UnstyledButton,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconChevronDown,
  IconChevronUp,
  IconFilter,
} from "@tabler/icons-react";
import { useMemo } from "react";
import type { RecommendationDto } from "@/api/recommendations";
import { TriStateChip } from "@/components/library/TriStateChip";
import type { TriState } from "@/types";

// =============================================================================
// Types
// =============================================================================

/** Tri-state filter group: maps values to neutral/include/exclude */
export interface TriStateGroup {
  values: Map<string, TriState>;
}

export interface RecommendationFilterState {
  /** Status filter (neutral/include/exclude per value) */
  statuses: TriStateGroup;
  /** Genre filter */
  genres: TriStateGroup;
  /** Tag filter */
  tags: TriStateGroup;
  /** Format filter */
  formats: TriStateGroup;
  /** Country filter */
  countries: TriStateGroup;
  /** Seed (basedOn) filter */
  seeds: TriStateGroup;
  /** Min score range [0, 100] */
  scoreRange: [number, number];
}

function emptyGroup(): TriStateGroup {
  return { values: new Map() };
}

export const DEFAULT_FILTERS: RecommendationFilterState = {
  statuses: emptyGroup(),
  genres: emptyGroup(),
  tags: emptyGroup(),
  formats: emptyGroup(),
  countries: emptyGroup(),
  seeds: emptyGroup(),
  scoreRange: [0, 100],
};

/** Human-readable labels for country codes */
const COUNTRY_LABELS: Record<string, string> = {
  JP: "Japan (Manga)",
  KR: "Korea (Manhwa)",
  CN: "China (Manhua)",
  TW: "Taiwan",
};

/** Human-readable labels for formats */
const FORMAT_LABELS: Record<string, string> = {
  MANGA: "Manga",
  NOVEL: "Novel",
  ONE_SHOT: "One Shot",
};

/** Human-readable labels for statuses */
const STATUS_LABELS: Record<string, string> = {
  ongoing: "Ongoing",
  ended: "Ended",
  hiatus: "Hiatus",
  abandoned: "Abandoned",
  unknown: "Unknown",
};

// =============================================================================
// Helpers
// =============================================================================

/** Extract unique values from recommendations for each filter dimension */
export function extractFilterOptions(recommendations: RecommendationDto[]) {
  const statuses = new Set<string>();
  const genres = new Set<string>();
  const tags = new Set<string>();
  const formats = new Set<string>();
  const countries = new Set<string>();
  const seeds = new Set<string>();

  for (const rec of recommendations) {
    if (rec.status) statuses.add(rec.status);
    for (const g of rec.genres ?? []) genres.add(g);
    for (const t of rec.tags ?? []) tags.add(t.name);
    if (rec.format) formats.add(rec.format);
    if (rec.countryOfOrigin) countries.add(rec.countryOfOrigin);
    for (const s of rec.basedOn ?? []) seeds.add(s);
  }

  return { statuses, genres, tags, formats, countries, seeds };
}

/** Get included values from a tri-state group */
function getIncluded(group: TriStateGroup): string[] {
  const result: string[] = [];
  for (const [value, state] of group.values) {
    if (state === "include") result.push(value);
  }
  return result;
}

/** Get excluded values from a tri-state group */
function getExcluded(group: TriStateGroup): string[] {
  const result: string[] = [];
  for (const [value, state] of group.values) {
    if (state === "exclude") result.push(value);
  }
  return result;
}

/** Check if a group has any active (non-neutral) filters */
function groupHasActive(group: TriStateGroup): boolean {
  for (const state of group.values.values()) {
    if (state !== "neutral") return true;
  }
  return false;
}

/** Apply filters to a list of recommendations */
export function applyFilters(
  recommendations: RecommendationDto[],
  filters: RecommendationFilterState,
): RecommendationDto[] {
  return recommendations.filter((rec) => {
    // Status filter
    if (groupHasActive(filters.statuses)) {
      const included = getIncluded(filters.statuses);
      const excluded = getExcluded(filters.statuses);
      if (
        included.length > 0 &&
        (!rec.status || !included.includes(rec.status))
      ) {
        return false;
      }
      if (excluded.length > 0 && rec.status && excluded.includes(rec.status)) {
        return false;
      }
    }

    // Genre filter (include = must have at least one; exclude = must not have any)
    if (groupHasActive(filters.genres)) {
      const included = getIncluded(filters.genres);
      const excluded = getExcluded(filters.genres);
      const recGenres = rec.genres ?? [];
      if (included.length > 0 && !recGenres.some((g) => included.includes(g))) {
        return false;
      }
      if (excluded.length > 0 && recGenres.some((g) => excluded.includes(g))) {
        return false;
      }
    }

    // Tag filter (include = must have at least one; exclude = must not have any)
    if (groupHasActive(filters.tags)) {
      const included = getIncluded(filters.tags);
      const excluded = getExcluded(filters.tags);
      const recTags = (rec.tags ?? []).map((t) => t.name);
      if (included.length > 0 && !recTags.some((t) => included.includes(t))) {
        return false;
      }
      if (excluded.length > 0 && recTags.some((t) => excluded.includes(t))) {
        return false;
      }
    }

    // Format filter
    if (groupHasActive(filters.formats)) {
      const included = getIncluded(filters.formats);
      const excluded = getExcluded(filters.formats);
      if (
        included.length > 0 &&
        (!rec.format || !included.includes(rec.format))
      ) {
        return false;
      }
      if (excluded.length > 0 && rec.format && excluded.includes(rec.format)) {
        return false;
      }
    }

    // Country filter
    if (groupHasActive(filters.countries)) {
      const included = getIncluded(filters.countries);
      const excluded = getExcluded(filters.countries);
      if (
        included.length > 0 &&
        (!rec.countryOfOrigin || !included.includes(rec.countryOfOrigin))
      ) {
        return false;
      }
      if (
        excluded.length > 0 &&
        rec.countryOfOrigin &&
        excluded.includes(rec.countryOfOrigin)
      ) {
        return false;
      }
    }

    // Seed filter (basedOn)
    if (groupHasActive(filters.seeds)) {
      const included = getIncluded(filters.seeds);
      const excluded = getExcluded(filters.seeds);
      const recSeeds = rec.basedOn ?? [];
      if (included.length > 0 && !recSeeds.some((s) => included.includes(s))) {
        return false;
      }
      if (excluded.length > 0 && recSeeds.some((s) => excluded.includes(s))) {
        return false;
      }
    }

    // Score filter
    const scorePercent = Math.round(rec.score * 100);
    if (
      scorePercent < filters.scoreRange[0] ||
      scorePercent > filters.scoreRange[1]
    ) {
      return false;
    }

    return true;
  });
}

/** Count how many filter dimensions are active */
export function activeFilterCount(filters: RecommendationFilterState): number {
  let count = 0;
  if (groupHasActive(filters.statuses)) count++;
  if (groupHasActive(filters.genres)) count++;
  if (groupHasActive(filters.tags)) count++;
  if (groupHasActive(filters.formats)) count++;
  if (groupHasActive(filters.countries)) count++;
  if (groupHasActive(filters.seeds)) count++;
  if (filters.scoreRange[0] > 0 || filters.scoreRange[1] < 100) count++;
  return count;
}

// =============================================================================
// Component
// =============================================================================

interface RecommendationFiltersProps {
  recommendations: RecommendationDto[];
  filters: RecommendationFilterState;
  onChange: (filters: RecommendationFilterState) => void;
}

export function RecommendationFilters({
  recommendations,
  filters,
  onChange,
}: RecommendationFiltersProps) {
  const [opened, { toggle }] = useDisclosure(false);
  const options = useMemo(
    () => extractFilterOptions(recommendations),
    [recommendations],
  );
  const activeCount = activeFilterCount(filters);

  const setTriState = (
    key: keyof Pick<
      RecommendationFilterState,
      "statuses" | "genres" | "tags" | "formats" | "countries" | "seeds"
    >,
    value: string,
    state: TriState,
  ) => {
    const next = new Map(filters[key].values);
    if (state === "neutral") {
      next.delete(value);
    } else {
      next.set(value, state);
    }
    onChange({ ...filters, [key]: { values: next } });
  };

  const getState = (
    key: keyof Pick<
      RecommendationFilterState,
      "statuses" | "genres" | "tags" | "formats" | "countries" | "seeds"
    >,
    value: string,
  ): TriState => {
    return filters[key].values.get(value) ?? "neutral";
  };

  return (
    <Stack gap={0}>
      <Group gap="xs">
        <UnstyledButton onClick={toggle} data-testid="filter-toggle">
          <Group gap={4}>
            <IconFilter size={16} />
            <Text size="sm" fw={500}>
              Filters
            </Text>
            {activeCount > 0 && (
              <Badge size="xs" circle color="blue">
                {activeCount}
              </Badge>
            )}
            {opened ? (
              <IconChevronUp size={14} />
            ) : (
              <IconChevronDown size={14} />
            )}
          </Group>
        </UnstyledButton>
        {activeCount > 0 && (
          <UnstyledButton
            onClick={() => onChange({ ...DEFAULT_FILTERS })}
            data-testid="filter-clear"
          >
            <Text size="xs" c="dimmed" td="underline">
              Clear all
            </Text>
          </UnstyledButton>
        )}
      </Group>

      <Collapse in={opened}>
        <Stack gap="md" mt="sm" pl="xs">
          {/* Score range slider — full width */}
          <div>
            <Text size="sm" fw={500} mb={4}>
              Match Score
            </Text>
            <RangeSlider
              min={0}
              max={100}
              step={5}
              value={filters.scoreRange}
              onChange={(value) =>
                onChange({ ...filters, scoreRange: value as [number, number] })
              }
              label={(v) => `${v}%`}
              marks={[
                { value: 0, label: "0%" },
                { value: 50, label: "50%" },
                { value: 100, label: "100%" },
              ]}
              data-testid="score-slider"
            />
          </div>

          {/* Two-column grid for filter groups */}
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
            {/* Country filter */}
            {options.countries.size > 0 && (
              <div>
                <Text size="sm" fw={500} mb={4}>
                  Origin
                </Text>
                <Group gap={6}>
                  {[...options.countries].sort().map((country) => (
                    <TriStateChip
                      key={country}
                      label={COUNTRY_LABELS[country] ?? country}
                      state={getState("countries", country)}
                      onChange={(s) => setTriState("countries", country, s)}
                    />
                  ))}
                </Group>
              </div>
            )}

            {/* Format filter */}
            {options.formats.size > 0 && (
              <div>
                <Text size="sm" fw={500} mb={4}>
                  Format
                </Text>
                <Group gap={6}>
                  {[...options.formats].sort().map((format) => (
                    <TriStateChip
                      key={format}
                      label={FORMAT_LABELS[format] ?? format}
                      state={getState("formats", format)}
                      onChange={(s) => setTriState("formats", format, s)}
                    />
                  ))}
                </Group>
              </div>
            )}

            {/* Status filter */}
            {options.statuses.size > 0 && (
              <div>
                <Text size="sm" fw={500} mb={4}>
                  Status
                </Text>
                <Group gap={6}>
                  {[...options.statuses].sort().map((status) => (
                    <TriStateChip
                      key={status}
                      label={STATUS_LABELS[status] ?? status}
                      state={getState("statuses", status)}
                      onChange={(s) => setTriState("statuses", status, s)}
                    />
                  ))}
                </Group>
              </div>
            )}

            {/* Seed filter (based on) */}
            {options.seeds.size > 1 && (
              <div>
                <Text size="sm" fw={500} mb={4}>
                  Based On
                </Text>
                <Group gap={6}>
                  {[...options.seeds].sort().map((seed) => (
                    <TriStateChip
                      key={seed}
                      label={seed}
                      state={getState("seeds", seed)}
                      onChange={(s) => setTriState("seeds", seed, s)}
                    />
                  ))}
                </Group>
              </div>
            )}

            {/* Genre filter — spans full width */}
            {options.genres.size > 0 && (
              <div style={{ gridColumn: "1 / -1" }}>
                <Text size="sm" fw={500} mb={4}>
                  Genres
                </Text>
                <Group gap={6}>
                  {[...options.genres].sort().map((genre) => (
                    <TriStateChip
                      key={genre}
                      label={genre}
                      state={getState("genres", genre)}
                      onChange={(s) => setTriState("genres", genre, s)}
                    />
                  ))}
                </Group>
              </div>
            )}

            {/* Tag filter — spans full width */}
            {options.tags.size > 0 && (
              <div style={{ gridColumn: "1 / -1" }}>
                <Text size="sm" fw={500} mb={4}>
                  Tags
                </Text>
                <Group gap={6}>
                  {[...options.tags].sort().map((tag) => (
                    <TriStateChip
                      key={tag}
                      label={tag}
                      state={getState("tags", tag)}
                      onChange={(s) => setTriState("tags", tag, s)}
                    />
                  ))}
                </Group>
              </div>
            )}
          </SimpleGrid>
        </Stack>
      </Collapse>
    </Stack>
  );
}
