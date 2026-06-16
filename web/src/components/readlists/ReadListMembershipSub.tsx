import { Loader, Menu } from "@mantine/core";
import { IconCheck, IconListNumbers, IconPlus } from "@tabler/icons-react";
import {
  useAddBooksToReadList,
  useReadLists,
  useReadListsForBook,
  useRemoveBookFromReadLists,
} from "@/hooks/useReadLists";

/**
 * Nested submenu for toggling a book across read lists, made for embedding
 * inside another Menu (e.g. the media card dropdown).
 *
 * The inline "New read list…" action is delegated to the parent via
 * `onRequestCreate`: the create modal must live outside this Menu, because
 * clicking any item closes the surrounding menu, which would unmount a modal
 * rendered within the dropdown before it could open.
 *
 * The membership query only runs once this component mounts, which Mantine
 * defers until the parent dropdown is opened, so a grid of cards does not fire
 * one request per card on mount.
 *
 * Render only for users with `readlists:write`.
 */
export function ReadListMembershipSub({
  bookId,
  onRequestCreate,
}: {
  bookId: string;
  onRequestCreate: () => void;
}) {
  const { data: readLists, isLoading } = useReadLists();
  const { data: memberOf } = useReadListsForBook(bookId);
  const add = useAddBooksToReadList();
  const remove = useRemoveBookFromReadLists();

  const memberIds = new Set((memberOf ?? []).map((r) => r.id));
  const busy = add.isPending || remove.isPending;

  const toggle = (readListId: string) => {
    if (memberIds.has(readListId)) {
      remove.mutate({ readListId, bookId });
    } else {
      add.mutate({ readListId, bookIds: [bookId] });
    }
  };

  return (
    <Menu.Sub>
      <Menu.Sub.Target>
        <Menu.Sub.Item leftSection={<IconListNumbers size={14} />}>
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
              closeMenuOnClick={false}
              disabled={busy}
              leftSection={
                memberIds.has(r.id) ? (
                  <IconCheck size={16} />
                ) : (
                  <span style={{ width: 16, display: "inline-block" }} />
                )
              }
              onClick={(e: React.MouseEvent) => {
                e.stopPropagation();
                toggle(r.id);
              }}
            >
              {r.name}
            </Menu.Item>
          ))
        )}
        <Menu.Divider />
        <Menu.Item
          leftSection={<IconPlus size={16} />}
          onClick={(e: React.MouseEvent) => {
            e.stopPropagation();
            onRequestCreate();
          }}
        >
          New read list…
        </Menu.Item>
      </Menu.Sub.Dropdown>
    </Menu.Sub>
  );
}
