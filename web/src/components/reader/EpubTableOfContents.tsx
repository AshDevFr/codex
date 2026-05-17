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

interface EpubTableOfContentsTriggerProps {
  /** Callback to open/close the drawer */
  onToggle: () => void;
}

/**
 * Trigger ActionIcon for the EPUB TOC drawer. Lives inside the toolbar so it
 * can sit alongside other reader actions on desktop. The {@link EpubTableOfContentsDrawer}
 * is rendered separately at the reader level so it survives the toolbar's
 * auto-hide unmount.
 */
export function EpubTableOfContentsTrigger({
  onToggle,
}: EpubTableOfContentsTriggerProps) {
  return (
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
  );
}

interface EpubTableOfContentsDrawerProps {
  /** Table of contents items from epub.js */
  toc: NavItem[];
  /** Currently active chapter href */
  currentHref?: string;
  /** Whether the drawer is open */
  opened: boolean;
  /** Callback to close the drawer */
  onClose: () => void;
  /** Callback when a TOC item is clicked */
  onNavigate: (href: string) => void;
}

/**
 * Drawer-only TOC. Rendered at the reader level (outside the toolbar's
 * `<Transition>` subtree) so it stays mounted while the toolbar auto-hides.
 */
export function EpubTableOfContentsDrawer({
  toc,
  currentHref,
  opened,
  onClose,
  onNavigate,
}: EpubTableOfContentsDrawerProps) {
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
            onClose();
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
    <Drawer
      opened={opened}
      onClose={onClose}
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
  );
}
