import { Badge, type MantineColor } from "@mantine/core";
import {
  IconBook,
  IconBook2,
  IconBookmark,
  IconBrush,
  IconNews,
  IconNotebook,
  IconStack2,
} from "@tabler/icons-react";
import type { BookType } from "@/types/book-metadata";
import { BOOK_TYPE_COLORS, BOOK_TYPE_DISPLAY } from "@/types/book-metadata";

interface BookTypeBadgeProps {
  bookType: BookType | string | null | undefined;
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  variant?: "filled" | "light" | "outline" | "dot" | "gradient";
  withIcon?: boolean;
}

/**
 * Get the appropriate icon for a book type
 */
function getBookTypeIcon(bookType: BookType | string) {
  switch (bookType) {
    case "comic":
      return IconBook2;
    case "manga":
      return IconBook2;
    case "novel":
      return IconBook;
    case "novella":
      return IconBookmark;
    case "anthology":
      return IconStack2;
    case "artbook":
      return IconBrush;
    case "oneshot":
      return IconBookmark;
    case "omnibus":
      return IconStack2;
    case "graphic_novel":
      return IconNotebook;
    case "magazine":
      return IconNews;
    default:
      return IconBook;
  }
}

/**
 * Badge component displaying the book type with appropriate styling.
 */
export function BookTypeBadge({
  bookType,
  size = "sm",
  variant = "light",
  withIcon = false,
}: BookTypeBadgeProps) {
  if (!bookType) return null;

  const normalizedType = bookType.toLowerCase() as BookType;
  const displayName =
    BOOK_TYPE_DISPLAY[normalizedType] ??
    bookType.charAt(0).toUpperCase() + bookType.slice(1).replace("_", " ");
  const color = (BOOK_TYPE_COLORS[normalizedType] ?? "gray") as MantineColor;
  const Icon = getBookTypeIcon(normalizedType);

  return (
    <Badge
      variant={variant}
      color={color}
      size={size}
      leftSection={withIcon ? <Icon size={12} /> : undefined}
    >
      {displayName}
    </Badge>
  );
}
