import { Box, Skeleton, Stack } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";

const COVER_ASPECT_RATIO = "150/212.125";

const MOBILE_DEFAULT_COUNT = 6;
const DESKTOP_DEFAULT_COUNT = 12;

/**
 * Shape-matched skeleton for the `repeat(auto-fill, minmax(150px, 1fr))`
 * cover grid used by Library, SeriesDetail, and Home. Renders N cards
 * matching the cover aspect ratio + title row used by `MediaCard`, so the
 * layout doesn't shift when real data lands.
 *
 * Defaults to fewer cards on mobile (2-column grid fills with ~6 cards)
 * vs desktop (6+ columns benefit from a longer grid).
 */
export interface CoverGridSkeletonProps {
  count?: number;
  /** Force a count regardless of viewport. Use when the caller already
   * knows the exact page size and wants no mobile/desktop heuristic. */
  exactCount?: boolean;
  "data-testid"?: string;
}

export function CoverGridSkeleton({
  count,
  exactCount = false,
  "data-testid": dataTestId = "cover-grid-skeleton",
}: CoverGridSkeletonProps) {
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;
  const resolvedCount = exactCount
    ? (count ?? DESKTOP_DEFAULT_COUNT)
    : (count ?? (isMobile ? MOBILE_DEFAULT_COUNT : DESKTOP_DEFAULT_COUNT));

  return (
    <div
      data-testid={dataTestId}
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
        gap: "var(--mantine-spacing-md)",
        width: "100%",
      }}
    >
      {Array.from({ length: resolvedCount }).map((_, idx) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder, never reorders
        <Box key={`cover-skel-${idx}`}>
          <Skeleton
            radius="md"
            mb="xs"
            style={{ aspectRatio: COVER_ASPECT_RATIO, height: "auto" }}
          />
          <Stack gap={4}>
            <Skeleton height={14} width="80%" radius="sm" />
            <Skeleton height={10} width="50%" radius="sm" />
          </Stack>
        </Box>
      ))}
    </div>
  );
}
