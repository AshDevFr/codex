import { Box, Grid, Group, Skeleton, Stack } from "@mantine/core";

const COVER_ASPECT_RATIO = "150/212.125";

/**
 * Shape-matched skeleton for [BookDetail](../../pages/BookDetail.tsx):
 * breadcrumbs + cover + title + badges + action row + reading progress +
 * metadata rows. Matches the real layout's column ratio so swapping in
 * real data does not shift the page.
 */
export function BookDetailSkeleton() {
  return (
    <Box py="md" px="md" data-testid="book-detail-skeleton">
      <Stack gap="md">
        {/* Breadcrumbs */}
        <Skeleton height={14} width="55%" radius="sm" />

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
              <Skeleton height={28} width="55%" radius="sm" />
              <Group gap="xs" mt={4}>
                <Skeleton height={18} width={70} radius="xl" />
                <Skeleton height={18} width={60} radius="xl" />
              </Group>
              {/* Reading progress bar */}
              <Skeleton height={8} width="100%" radius="xl" mt="xs" />
              <Skeleton height={10} width="30%" radius="sm" />
              {/* Action buttons */}
              <Group gap="sm" mt="xs">
                <Skeleton height={28} width={96} radius="sm" />
                <Skeleton height={28} width={108} radius="sm" />
                <Skeleton height={28} width={28} circle />
              </Group>
              {/* Summary preview */}
              <Skeleton height={12} width="100%" radius="sm" mt="xs" />
              <Skeleton height={12} width="92%" radius="sm" />
              <Skeleton height={12} width="78%" radius="sm" />
            </Stack>
          </Grid.Col>
        </Grid>

        {/* Metadata rows */}
        <Stack gap="xs">
          {Array.from({ length: 5 }).map((_, idx) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
            <Group gap="md" key={`book-meta-row-${idx}`}>
              <Skeleton height={10} width={80} radius="sm" />
              <Skeleton height={18} width={160} radius="xl" />
            </Group>
          ))}
        </Stack>
      </Stack>
    </Box>
  );
}
