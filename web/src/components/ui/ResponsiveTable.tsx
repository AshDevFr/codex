import {
  Box,
  type BoxProps,
  Card,
  type CardProps,
  Group,
  Stack,
  Table,
  type TableProps,
  type TableTdProps,
  type TableThProps,
  Text,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import type { CSSProperties, ReactNode } from "react";

/**
 * Mobile breakpoint used by the responsive table. Matches the `xs` value in
 * `web/src/theme.ts` (30.125em). The `0.0625em` (~1px) deduction guards against
 * sub-pixel rounding so the query fires exactly when Mantine considers the
 * viewport below `xs`.
 */
export const MOBILE_MEDIA_QUERY = "(max-width: 30.0625em)";

export interface ResponsiveTableColumn<T> {
  /** Stable react key for this column. */
  key: string;
  /** Header cell content. Doubles as the mobile card label when `mobileLabel` is not set. */
  header: ReactNode;
  /** Returns the cell content for a given row. */
  accessor: (row: T, rowIndex: number) => ReactNode;
  /** Override the label shown next to the value on mobile (defaults to `header`). */
  mobileLabel?: ReactNode;
  /** Skip the column on mobile. Use for columns better expressed elsewhere on a card. */
  hideOnMobile?: boolean;
  /** Skip the column on desktop. Useful for mobile-only summary lines. */
  hideOnDesktop?: boolean;
  /**
   * Render the value as a card header on mobile — no label, larger emphasis,
   * placed before the label/value rows. Useful for the primary identifier
   * (e.g. user name, plugin name).
   */
  mobilePrimary?: boolean;
  /** Hide the label on the mobile card; render the value full-width. */
  mobileFullWidth?: boolean;
  /** Props applied to the desktop `<Table.Th>`. */
  thProps?: Omit<TableThProps, "children">;
  /** Props applied to the desktop `<Table.Td>`. */
  tdProps?: Omit<TableTdProps, "children">;
}

export interface ResponsiveTableProps<T> {
  /** Row data. */
  data: T[];
  /** Column definitions. */
  columns: ResponsiveTableColumn<T>[];
  /** Stable react key per row. */
  getRowKey: (row: T, index: number) => string;
  /**
   * Optional per-row actions. On desktop the actions render as the last cell
   * of each row. On mobile they render as a footer at the bottom of each card.
   */
  rowActions?: (row: T, rowIndex: number) => ReactNode;
  /** Header text for the actions column on desktop. */
  rowActionsHeader?: ReactNode;
  /** Props applied to the desktop `<Table>`. */
  tableProps?: Omit<TableProps, "children">;
  /** Props applied to each mobile `<Card>`. */
  cardProps?: Omit<CardProps, "children">;
  /** Wrapper props for the desktop table. */
  desktopWrapperProps?: BoxProps;
  /** Wrapper props for the mobile stack. */
  mobileWrapperProps?: BoxProps;
  /** Rendered (in both layouts) when `data` is empty. */
  emptyState?: ReactNode;
  /**
   * Custom mobile card body. If provided, replaces the default label/value
   * list. `rowActions`, if present, is still appended below the body.
   */
  renderMobileCard?: (row: T, rowIndex: number) => ReactNode;
  /**
   * Optional `data-testid` applied to both the desktop table and the mobile
   * stack. Useful for visual regression tests that need to address both
   * layouts.
   */
  "data-testid"?: string;
}

const PRIMARY_TEXT_STYLE: CSSProperties = {
  minWidth: 0,
  wordBreak: "break-word",
};

const VALUE_BOX_STYLE: CSSProperties = {
  minWidth: 0,
  textAlign: "right",
  flex: 1,
};

const FULL_WIDTH_VALUE_STYLE: CSSProperties = {
  minWidth: 0,
};

/**
 * Renders a data table that gracefully degrades to a stack of cards below the
 * `xs` breakpoint. Above `xs` (≥ 30.125em) the standard Mantine `<Table>` is
 * used. Below `xs` each row becomes a `<Card>` with stacked label/value rows
 * and row actions in a footer.
 *
 * For pages with a bespoke mobile layout (e.g. expandable details rows,
 * multi-line primary content), pass `renderMobileCard` to override the
 * default body.
 */
export function ResponsiveTable<T>({
  data,
  columns,
  getRowKey,
  rowActions,
  rowActionsHeader = "Actions",
  tableProps,
  cardProps,
  desktopWrapperProps,
  mobileWrapperProps,
  emptyState,
  renderMobileCard,
  "data-testid": dataTestid,
}: ResponsiveTableProps<T>) {
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;

  if (data.length === 0 && emptyState !== undefined) {
    return <>{emptyState}</>;
  }

  if (isMobile) {
    return (
      <Box {...mobileWrapperProps} data-testid={dataTestid}>
        <Stack gap="sm">
          {data.map((row, idx) => (
            <Card
              key={getRowKey(row, idx)}
              withBorder
              padding="md"
              {...cardProps}
            >
              {renderMobileCard ? (
                renderMobileCard(row, idx)
              ) : (
                <Stack gap="xs">
                  {columns
                    .filter((col) => !col.hideOnMobile)
                    .map((col) => {
                      const value = col.accessor(row, idx);
                      if (col.mobilePrimary) {
                        return (
                          <Box key={col.key} style={PRIMARY_TEXT_STYLE}>
                            {value}
                          </Box>
                        );
                      }
                      if (col.mobileFullWidth) {
                        return (
                          <Stack key={col.key} gap={2}>
                            <Text size="xs" c="dimmed">
                              {col.mobileLabel ?? col.header}
                            </Text>
                            <Box style={FULL_WIDTH_VALUE_STYLE}>{value}</Box>
                          </Stack>
                        );
                      }
                      return (
                        <Group
                          key={col.key}
                          justify="space-between"
                          gap="md"
                          wrap="nowrap"
                          align="flex-start"
                        >
                          <Text size="sm" c="dimmed" style={{ flexShrink: 0 }}>
                            {col.mobileLabel ?? col.header}
                          </Text>
                          <Box style={VALUE_BOX_STYLE}>{value}</Box>
                        </Group>
                      );
                    })}
                </Stack>
              )}
              {rowActions ? (
                <Group justify="flex-end" gap="xs" mt="sm" wrap="nowrap">
                  {rowActions(row, idx)}
                </Group>
              ) : null}
            </Card>
          ))}
        </Stack>
      </Box>
    );
  }

  return (
    <Box {...desktopWrapperProps} data-testid={dataTestid}>
      <Table {...tableProps}>
        <Table.Thead>
          <Table.Tr>
            {columns
              .filter((col) => !col.hideOnDesktop)
              .map((col) => (
                <Table.Th key={col.key} {...col.thProps}>
                  {col.header}
                </Table.Th>
              ))}
            {rowActions ? <Table.Th>{rowActionsHeader}</Table.Th> : null}
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {data.map((row, idx) => (
            <Table.Tr key={getRowKey(row, idx)}>
              {columns
                .filter((col) => !col.hideOnDesktop)
                .map((col) => (
                  <Table.Td key={col.key} {...col.tdProps}>
                    {col.accessor(row, idx)}
                  </Table.Td>
                ))}
              {rowActions ? (
                <Table.Td>
                  <Group gap="xs" wrap="nowrap">
                    {rowActions(row, idx)}
                  </Group>
                </Table.Td>
              ) : null}
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Box>
  );
}
