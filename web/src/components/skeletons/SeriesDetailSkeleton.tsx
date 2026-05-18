import { Box, Grid, Group, Skeleton, Stack } from "@mantine/core";
import { CoverGridSkeleton } from "./CoverGridSkeleton";

const COVER_ASPECT_RATIO = "150/212.125";

/**
 * Shape-matched skeleton for [SeriesDetail](../../pages/SeriesDetail.tsx):
 * breadcrumbs + cover + title + badges + action buttons + metadata rows +
 * a books grid placeholder. Mirrors the real layout's `Grid` columns so
 * the swap to real data does not shift the page.
 */
export function SeriesDetailSkeleton() {
  return (
    <Box pt="xs" pb="md" px="md" data-testid="series-detail-skeleton">
      <Stack gap="md">
        {/* Breadcrumbs */}
        <Skeleton height={14} width="40%" radius="sm" />

        {/* Header: Cover + Info */}
        <Grid gutter="md">
          <Grid.Col span={{ base: 4, xs: 3, sm: 2 }}>
            <Skeleton
              radius="sm"
              style={{ aspectRatio: COVER_ASPECT_RATIO, height: "auto" }}
            />
          </Grid.Col>
          <Grid.Col span={{ base: 8, xs: 9, sm: 10 }}>
            <Stack gap="xs">
              {/* Title */}
              <Skeleton height={28} width="60%" radius="sm" />
              {/* Badges */}
              <Group gap="xs" mt={4}>
                <Skeleton height={18} width={60} radius="xl" />
                <Skeleton height={18} width={80} radius="xl" />
                <Skeleton height={18} width={40} radius="xl" />
              </Group>
              {/* Book count */}
              <Skeleton height={12} width="35%" radius="sm" />
              {/* Action buttons */}
              <Group gap="sm" mt="xs">
                <Skeleton height={28} width={96} radius="sm" />
                <Skeleton height={28} width={108} radius="sm" />
                <Skeleton height={28} width={28} circle />
              </Group>
              {/* Summary preview */}
              <Skeleton height={12} width="100%" radius="sm" mt="xs" />
              <Skeleton height={12} width="90%" radius="sm" />
            </Stack>
          </Grid.Col>
        </Grid>

        {/* Metadata rows */}
        <Stack gap="xs">
          {Array.from({ length: 4 }).map((_, idx) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
            <Group gap="md" key={`series-meta-row-${idx}`}>
              <Skeleton height={10} width={80} radius="sm" />
              <Skeleton height={18} width={140} radius="xl" />
            </Group>
          ))}
        </Stack>

        {/* Books grid placeholder */}
        <CoverGridSkeleton data-testid="series-detail-books-skeleton" />
      </Stack>
    </Box>
  );
}
