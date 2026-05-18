import { Box, Card, Group, Skeleton, Stack, Table, Text } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";

export interface TableSkeletonProps {
  /** Number of rows to render (desktop) or cards (mobile). */
  rows?: number;
  /** Number of columns to mimic on the desktop table. */
  columns?: number;
  /** Optional column header labels — when provided we render real `<th>`
   * cells instead of empty placeholders so width hints stay stable. */
  columnLabels?: string[];
  /** Render a primary larger placeholder line at the top of each mobile card
   * (mirrors `mobilePrimary` from `ResponsiveTable`). */
  withMobilePrimary?: boolean;
  "data-testid"?: string;
}

const DEFAULT_ROWS = 6;
const DEFAULT_COLUMNS = 4;

/**
 * Shape-matched skeleton for Settings pages that render a
 * [ResponsiveTable](../ui/ResponsiveTable.tsx). Auto-switches between a
 * desktop table layout and a stacked card layout below the same
 * `MOBILE_MEDIA_QUERY` width so the layout does not shift when data lands.
 */
export function TableSkeleton({
  rows = DEFAULT_ROWS,
  columns = DEFAULT_COLUMNS,
  columnLabels,
  withMobilePrimary = false,
  "data-testid": dataTestid = "table-skeleton",
}: TableSkeletonProps) {
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;
  const colCount = columnLabels?.length ?? columns;

  if (isMobile) {
    return (
      <Box data-testid={dataTestid}>
        <Stack gap="sm">
          {Array.from({ length: rows }).map((_, rIdx) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder, never reorders
            <Card key={`ts-card-${rIdx}`} withBorder padding="md">
              <Stack gap="xs">
                {withMobilePrimary && (
                  <Skeleton height={18} width="60%" radius="sm" />
                )}
                {Array.from({ length: colCount }).map((__, cIdx) => (
                  <Group
                    // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
                    key={`ts-card-${rIdx}-${cIdx}`}
                    justify="space-between"
                    gap="md"
                    wrap="nowrap"
                  >
                    <Skeleton height={10} width={70} radius="sm" />
                    <Skeleton
                      height={14}
                      width={`${50 + ((cIdx * 7) % 30)}%`}
                      radius="sm"
                    />
                  </Group>
                ))}
              </Stack>
            </Card>
          ))}
        </Stack>
      </Box>
    );
  }

  return (
    <Box data-testid={dataTestid}>
      <Table>
        <Table.Thead>
          <Table.Tr>
            {Array.from({ length: colCount }).map((_, idx) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
              <Table.Th key={`ts-th-${idx}`}>
                {columnLabels?.[idx] ? (
                  <Text size="sm" fw={600}>
                    {columnLabels[idx]}
                  </Text>
                ) : (
                  <Skeleton height={12} width={80} radius="sm" />
                )}
              </Table.Th>
            ))}
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {Array.from({ length: rows }).map((_, rIdx) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
            <Table.Tr key={`ts-tr-${rIdx}`}>
              {Array.from({ length: colCount }).map((__, cIdx) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
                <Table.Td key={`ts-td-${rIdx}-${cIdx}`}>
                  <Skeleton
                    height={14}
                    width={`${50 + ((cIdx * 11) % 40)}%`}
                    radius="sm"
                  />
                </Table.Td>
              ))}
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Box>
  );
}

/**
 * Mobile-friendly stacked-card placeholder for any list whose desktop layout
 * is a plain card or row list (e.g. PluginStatusBanner, settings cards that
 * don't render through `ResponsiveTable`). Renders independent of viewport
 * — pages that already use `TableSkeleton` get the responsive switch for
 * free.
 */
export interface CardListSkeletonProps {
  count?: number;
  /** Lines of placeholder content per card. */
  lines?: number;
  "data-testid"?: string;
}

export function CardListSkeleton({
  count = DEFAULT_ROWS,
  lines = 3,
  "data-testid": dataTestid = "card-list-skeleton",
}: CardListSkeletonProps) {
  return (
    <Stack gap="sm" data-testid={dataTestid}>
      {Array.from({ length: count }).map((_, idx) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder, never reorders
        <Card key={`cls-card-${idx}`} withBorder padding="md">
          <Stack gap="xs">
            <Skeleton height={18} width="55%" radius="sm" />
            {Array.from({ length: lines }).map((__, lineIdx) => (
              <Skeleton
                // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length placeholder
                key={`cls-line-${idx}-${lineIdx}`}
                height={12}
                width={`${65 + ((lineIdx * 9) % 25)}%`}
                radius="sm"
              />
            ))}
          </Stack>
        </Card>
      ))}
    </Stack>
  );
}
