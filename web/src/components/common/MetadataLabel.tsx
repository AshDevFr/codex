import { Text, type TextProps } from "@mantine/core";
import { forwardRef } from "react";

export interface MetadataLabelProps extends TextProps {
  children: React.ReactNode;
}

/**
 * Section label for metadata rows (PUBLISHER, GENRES, TAGS, etc.) on
 * BookDetail and SeriesDetail. Pins the typography defined in the Phase 7
 * micro-pass: 11px, weight 600, +0.04em tracking, uppercase, dimmed.
 *
 * The fixed `w={100}` matches the legacy `<Text size="sm" c="dimmed" w={100}>`
 * pattern so existing two-column metadata rows stay aligned. Callers can
 * override any default via rest props (e.g. `style={{ flexShrink: 0 }}`).
 */
export const MetadataLabel = forwardRef<
  HTMLParagraphElement,
  MetadataLabelProps
>(function MetadataLabel({ children, ...rest }, ref) {
  return (
    <Text
      ref={ref}
      fz={11}
      fw={600}
      lts="0.04em"
      tt="uppercase"
      c="dimmed"
      w={100}
      {...rest}
    >
      {children}
    </Text>
  );
});
