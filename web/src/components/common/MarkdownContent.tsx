import { Table, Text, useMantineColorScheme } from "@mantine/core";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

export interface MarkdownContentProps {
  /**
   * The markdown content to render
   */
  children: string;
  /**
   * Custom class name for the container
   */
  className?: string;
  /**
   * Compact mode for smaller previews (reduced font sizes and spacing)
   */
  compact?: boolean;
}

/**
 * A styled Markdown content component using Mantine's Text component.
 * Renders markdown with consistent styling that adapts to the current color scheme.
 * Supports GitHub Flavored Markdown (tables, strikethrough, etc.)
 */
export function MarkdownContent({
  children,
  className,
  compact = false,
}: MarkdownContentProps) {
  const { colorScheme } = useMantineColorScheme();

  // Size configurations based on compact mode
  const sizes = compact
    ? { h1: "xs", h2: "xs", h3: "xs", body: "xs", spacing: 2 }
    : { h1: "lg", h2: "md", h3: "sm", body: "sm", spacing: "xs" };

  const components: Components = {
    h1: ({ children }) => (
      <Text fw={600} size={sizes.h1} mb={sizes.spacing}>
        {children}
      </Text>
    ),
    h2: ({ children }) => (
      <Text fw={600} size={sizes.h2} mb={sizes.spacing}>
        {children}
      </Text>
    ),
    h3: ({ children }) => (
      <Text fw={500} size={sizes.h3} mb={sizes.spacing}>
        {children}
      </Text>
    ),
    p: ({ children }) => (
      <Text size={sizes.body} mb={sizes.spacing}>
        {children}
      </Text>
    ),
    li: ({ children }) => (
      <Text component="li" size={sizes.body} mb={compact ? 1 : 4}>
        {children}
      </Text>
    ),
    a: ({ href, children }) => (
      <Text
        component="a"
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        c="blue"
        size={compact ? sizes.body : undefined}
        style={compact ? undefined : { textDecoration: "underline" }}
      >
        {children}
      </Text>
    ),
    code: ({ children }) => (
      <Text
        component="code"
        size={sizes.body}
        style={{
          fontFamily: "monospace",
          backgroundColor:
            colorScheme === "dark"
              ? "var(--mantine-color-dark-5)"
              : "var(--mantine-color-gray-2)",
          padding: compact ? "1px 4px" : "2px 6px",
          borderRadius: compact ? 2 : 4,
        }}
      >
        {children}
      </Text>
    ),
    // Table components using Mantine's Table
    table: ({ children }) => (
      <Table
        striped
        highlightOnHover
        withTableBorder
        withColumnBorders
        mb={sizes.spacing}
        fz={sizes.body}
      >
        {children}
      </Table>
    ),
    thead: ({ children }) => <Table.Thead>{children}</Table.Thead>,
    tbody: ({ children }) => <Table.Tbody>{children}</Table.Tbody>,
    tr: ({ children }) => <Table.Tr>{children}</Table.Tr>,
    th: ({ children }) => <Table.Th>{children}</Table.Th>,
    td: ({ children }) => <Table.Td>{children}</Table.Td>,
  };

  return (
    <div className={className}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {children}
      </ReactMarkdown>
    </div>
  );
}
