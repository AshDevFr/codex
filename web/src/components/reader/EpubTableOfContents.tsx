import {
  ActionIcon,
  Box,
  Drawer,
  NavLink,
  ScrollArea,
  Text,
  Tooltip,
} from "@mantine/core";
import { IconList } from "@tabler/icons-react";
import type { NavItem } from "epubjs";

interface EpubTableOfContentsProps {
  /** Table of contents items from epub.js */
  toc: NavItem[];
  /** Currently active chapter href */
  currentHref?: string;
  /** Whether the drawer is open */
  opened: boolean;
  /** Callback to open/close the drawer */
  onToggle: () => void;
  /** Callback when a TOC item is clicked */
  onNavigate: (href: string) => void;
}

/**
 * Table of Contents drawer for EPUB reader.
 *
 * Displays a hierarchical navigation structure from the EPUB's NCX/nav document.
 * Supports nested chapters (subitems) with indentation.
 */
export function EpubTableOfContents({
  toc,
  currentHref,
  opened,
  onToggle,
  onNavigate,
}: EpubTableOfContentsProps) {
  // Render a single TOC item (potentially with children)
  const renderTocItem = (item: NavItem, depth = 0) => {
    const isActive = currentHref === item.href;
    const hasChildren = item.subitems && item.subitems.length > 0;

    return (
      <Box key={item.id || item.href}>
        <NavLink
          label={item.label}
          active={isActive}
          onClick={() => {
            onNavigate(item.href);
            onToggle(); // Close drawer after navigation
          }}
          pl={depth * 16 + 12}
          styles={{
            root: {
              borderRadius: "var(--mantine-radius-sm)",
            },
            label: {
              fontSize: depth > 0 ? "0.875rem" : "1rem",
            },
          }}
        />
        {hasChildren &&
          item.subitems?.map((subitem) => renderTocItem(subitem, depth + 1))}
      </Box>
    );
  };

  return (
    <>
      {/* Toggle button */}
      <Tooltip label="Table of Contents" position="bottom">
        <ActionIcon
          variant="subtle"
          color="gray"
          size="lg"
          onClick={onToggle}
          aria-label="Table of Contents"
        >
          <IconList size={20} />
        </ActionIcon>
      </Tooltip>

      {/* TOC Drawer */}
      <Drawer
        opened={opened}
        onClose={onToggle}
        title="Table of Contents"
        position="left"
        size="sm"
        styles={{
          body: {
            padding: 0,
          },
        }}
      >
        <ScrollArea h="calc(100vh - 60px)" px="md" pb="md">
          {toc.length === 0 ? (
            <Text c="dimmed" size="sm" ta="center" py="xl">
              No table of contents available
            </Text>
          ) : (
            <Box>{toc.map((item) => renderTocItem(item))}</Box>
          )}
        </ScrollArea>
      </Drawer>
    </>
  );
}
