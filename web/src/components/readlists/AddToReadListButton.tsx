import { ActionIcon, Loader, Menu, Tooltip } from "@mantine/core";
import { IconCheck, IconListNumbers, IconPlus } from "@tabler/icons-react";
import { useState } from "react";
import {
  useAddBooksToReadList,
  useReadLists,
  useReadListsForBook,
  useRemoveBookFromReadLists,
} from "@/hooks/useReadLists";
import { ReadListFormModal } from "./ReadListFormModal";

/**
 * Toggle membership of a book across read lists, and create new ones inline.
 * Render only for users with `readlists:write`.
 */
export function AddToReadListButton({ bookId }: { bookId: string }) {
  const { data: readLists, isLoading } = useReadLists();
  const { data: memberOf } = useReadListsForBook(bookId);
  const add = useAddBooksToReadList();
  const remove = useRemoveBookFromReadLists();
  const [createOpen, setCreateOpen] = useState(false);

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
    <>
      <Menu position="bottom-end" withinPortal shadow="md" width={260}>
        <Menu.Target>
          <Tooltip label="Add to read list" openDelay={300}>
            <ActionIcon
              variant="subtle"
              size="lg"
              aria-label="Add to read list"
            >
              <IconListNumbers size={20} />
            </ActionIcon>
          </Tooltip>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>Read lists</Menu.Label>
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
                onClick={() => toggle(r.id)}
              >
                {r.name}
              </Menu.Item>
            ))
          )}
          <Menu.Divider />
          <Menu.Item
            leftSection={<IconPlus size={16} />}
            onClick={() => setCreateOpen(true)}
          >
            New read list…
          </Menu.Item>
        </Menu.Dropdown>
      </Menu>

      <ReadListFormModal
        opened={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={(r) => add.mutate({ readListId: r.id, bookIds: [bookId] })}
      />
    </>
  );
}
