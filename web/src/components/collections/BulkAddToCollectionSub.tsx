import { Loader, Menu } from "@mantine/core";
import { IconFolderPlus, IconPlus } from "@tabler/icons-react";
import { useCollections } from "@/hooks/useCollections";

/**
 * Nested submenu listing every collection, for embedding in the bulk-selection
 * "More" menu. Unlike the single-series version this does not show membership
 * checkmarks: across many series a single checked state is meaningless, so
 * picking a collection simply adds all selected series to it.
 *
 * `onRequestCreate` is delegated to the parent so the create modal can be
 * mounted outside this Menu (an item click closes the menu, which would
 * otherwise unmount a modal rendered within it).
 *
 * Render only for users with `collections:write`.
 */
export function BulkAddToCollectionSub({
  onSelect,
  onRequestCreate,
  disabled,
}: {
  onSelect: (collectionId: string) => void;
  onRequestCreate: () => void;
  disabled?: boolean;
}) {
  const { data: collections, isLoading } = useCollections();

  return (
    <Menu.Sub>
      <Menu.Sub.Target>
        <Menu.Sub.Item leftSection={<IconFolderPlus size={16} />}>
          Add to collection
        </Menu.Sub.Item>
      </Menu.Sub.Target>
      <Menu.Sub.Dropdown>
        {isLoading ? (
          <Menu.Item disabled>
            <Loader size="xs" />
          </Menu.Item>
        ) : !collections || collections.length === 0 ? (
          <Menu.Item disabled>No collections yet</Menu.Item>
        ) : (
          collections.map((c) => (
            <Menu.Item
              key={c.id}
              disabled={disabled}
              onClick={() => onSelect(c.id)}
            >
              {c.name}
            </Menu.Item>
          ))
        )}
        <Menu.Divider />
        <Menu.Item
          leftSection={<IconPlus size={16} />}
          onClick={onRequestCreate}
        >
          New collection…
        </Menu.Item>
      </Menu.Sub.Dropdown>
    </Menu.Sub>
  );
}
