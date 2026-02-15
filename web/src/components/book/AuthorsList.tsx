import { Badge, Group, Stack, Text, Tooltip } from "@mantine/core";
import { IconUser, IconUsers } from "@tabler/icons-react";
import type { BookAuthor, BookAuthorRole } from "@/types/book-metadata";
import {
  AUTHOR_ROLE_COLORS,
  AUTHOR_ROLE_DISPLAY,
  parseAuthorsJson,
} from "@/types/book-metadata";

interface AuthorsListProps {
  /** Authors as JSON string or pre-parsed array */
  authors: string | BookAuthor[] | null | undefined;
  /** Maximum number of authors to display before collapsing */
  maxDisplay?: number;
  /** Size of badges */
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  /** Layout direction */
  direction?: "horizontal" | "vertical";
  /** Whether to show role badges */
  showRoles?: boolean;
  /** Whether to group by role */
  groupByRole?: boolean;
}

/**
 * Component to display a list of authors with their roles.
 * Supports both JSON string input (from API) and pre-parsed array.
 */
export function AuthorsList({
  authors,
  maxDisplay,
  size = "sm",
  direction = "horizontal",
  showRoles = true,
  groupByRole = false,
}: AuthorsListProps) {
  // Parse authors if string
  const authorList: BookAuthor[] =
    typeof authors === "string" ? parseAuthorsJson(authors) : (authors ?? []);

  if (authorList.length === 0) return null;

  // Apply maxDisplay limit
  const displayAuthors = maxDisplay
    ? authorList.slice(0, maxDisplay)
    : authorList;
  const hiddenCount = authorList.length - displayAuthors.length;

  if (groupByRole) {
    // Group authors by role
    const byRole = new Map<BookAuthorRole, BookAuthor[]>();
    for (const author of displayAuthors) {
      const role = author.role ?? "author";
      const existing = byRole.get(role) ?? [];
      existing.push(author);
      byRole.set(role, existing);
    }

    return (
      <Stack gap="xs">
        {Array.from(byRole.entries()).map(([role, roleAuthors]) => (
          <Group key={role} gap="xs" align="flex-start">
            <Text size="xs" c="dimmed" w={80} tt="uppercase">
              {roleAuthors.length > 1
                ? `${AUTHOR_ROLE_DISPLAY[role]}s`
                : AUTHOR_ROLE_DISPLAY[role]}
            </Text>
            <Group gap="xs">
              {roleAuthors.map((author) => (
                <Tooltip
                  key={`${role}-${author.name}`}
                  label={author.name}
                  disabled
                >
                  <Badge
                    variant="light"
                    color={AUTHOR_ROLE_COLORS[role] ?? "gray"}
                    size={size}
                  >
                    {author.name}
                  </Badge>
                </Tooltip>
              ))}
            </Group>
          </Group>
        ))}
        {hiddenCount > 0 && (
          <Text size="xs" c="dimmed">
            +{hiddenCount} more
          </Text>
        )}
      </Stack>
    );
  }

  // Flat display
  const Container = direction === "vertical" ? Stack : Group;
  const containerProps =
    direction === "vertical"
      ? { gap: "xs" as const }
      : { gap: "xs" as const, wrap: "wrap" as const };

  return (
    <Container {...containerProps}>
      {displayAuthors.map((author) => {
        const role = author.role ?? "author";
        const Icon = displayAuthors.length > 1 ? IconUsers : IconUser;

        return (
          <Tooltip
            key={`${author.name}-${role}`}
            label={
              showRoles && role !== "author"
                ? AUTHOR_ROLE_DISPLAY[role]
                : author.name
            }
            disabled={!showRoles || role === "author"}
          >
            <Badge
              variant="light"
              color={AUTHOR_ROLE_COLORS[role] ?? "gray"}
              size={size}
              leftSection={<Icon size={10} />}
            >
              {author.name}
              {showRoles && role !== "author" && (
                <Text span size="xs" c="dimmed" ml={4}>
                  ({AUTHOR_ROLE_DISPLAY[role]})
                </Text>
              )}
            </Badge>
          </Tooltip>
        );
      })}
      {hiddenCount > 0 && (
        <Text size="xs" c="dimmed">
          +{hiddenCount} more
        </Text>
      )}
    </Container>
  );
}

/**
 * Compact display showing only author names as a comma-separated list.
 */
export function AuthorsCompact({
  authors,
  maxDisplay = 3,
}: Pick<AuthorsListProps, "authors" | "maxDisplay">) {
  const authorList: BookAuthor[] =
    typeof authors === "string" ? parseAuthorsJson(authors) : (authors ?? []);

  if (authorList.length === 0) return null;

  const displayNames = authorList.slice(0, maxDisplay).map((a) => a.name);
  const hiddenCount = authorList.length - displayNames.length;

  const displayText =
    hiddenCount > 0
      ? `${displayNames.join(", ")} +${hiddenCount} more`
      : displayNames.join(", ");

  return (
    <Tooltip
      label={authorList.map((a) => a.name).join(", ")}
      disabled={hiddenCount === 0}
    >
      <Text size="sm">{displayText}</Text>
    </Tooltip>
  );
}
