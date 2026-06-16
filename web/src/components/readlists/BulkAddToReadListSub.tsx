import { Loader, Menu } from "@mantine/core";
import { IconListNumbers, IconPlus } from "@tabler/icons-react";
import { useReadLists } from "@/hooks/useReadLists";

/**
 * Nested submenu listing every read list, for embedding in the bulk-selection
 * "More" menu. Unlike the single-book version this does not show membership
 * checkmarks: across many books a single checked state is meaningless, so
 * picking a read list simply adds all selected books to it.
 *
 * `onRequestCreate` is delegated to the parent so the create modal can be
 * mounted outside this Menu (an item click closes the menu, which would
 * otherwise unmount a modal rendered within it).
 *
 * Render only for users with `readlists:write`.
 */
export function BulkAddToReadListSub({
  onSelect,
  onRequestCreate,
  disabled,
}: {
  onSelect: (readListId: string) => void;
  onRequestCreate: () => void;
  disabled?: boolean;
}) {
  const { data: readLists, isLoading } = useReadLists();

  return (
    <Menu.Sub>
      <Menu.Sub.Target>
        <Menu.Sub.Item leftSection={<IconListNumbers size={16} />}>
          Add to read list
        </Menu.Sub.Item>
      </Menu.Sub.Target>
      <Menu.Sub.Dropdown>
        {isLoading ? (
          <Menu.Item disabled>
            <Loader size="xs" />
          </Menu.Item>
        ) : !readLists || readLists.length === 0 ? (
          <Menu.Item disabled>No read lists yet</Menu.Item>
        ) : (
          readLists.map((r) => (
            <Menu.Item
              key={r.id}
              disabled={disabled}
              onClick={() => onSelect(r.id)}
            >
              {r.name}
            </Menu.Item>
          ))
        )}
        <Menu.Divider />
        <Menu.Item
          leftSection={<IconPlus size={16} />}
          onClick={onRequestCreate}
        >
          New read list…
        </Menu.Item>
      </Menu.Sub.Dropdown>
    </Menu.Sub>
  );
}
