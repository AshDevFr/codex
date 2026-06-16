import { ActionIcon, Loader, Menu, Tooltip } from "@mantine/core";
import { IconCheck, IconFolderPlus, IconPlus } from "@tabler/icons-react";
import { useState } from "react";
import {
  useAddSeriesToCollection,
  useCollections,
  useCollectionsForSeries,
  useRemoveSeriesFromCollections,
} from "@/hooks/useCollections";
import { CollectionFormModal } from "./CollectionFormModal";

/**
 * Toggle membership of a series across collections, and create new ones inline.
 * Render only for users with `collections:write`.
 */
export function AddToCollectionButton({ seriesId }: { seriesId: string }) {
  const { data: collections, isLoading } = useCollections();
  const { data: memberOf } = useCollectionsForSeries(seriesId);
  const add = useAddSeriesToCollection();
  const remove = useRemoveSeriesFromCollections();
  const [createOpen, setCreateOpen] = useState(false);

  const memberIds = new Set((memberOf ?? []).map((c) => c.id));
  const busy = add.isPending || remove.isPending;

  const toggle = (collectionId: string) => {
    if (memberIds.has(collectionId)) {
      remove.mutate({ collectionId, seriesId });
    } else {
      add.mutate({ collectionId, seriesIds: [seriesId] });
    }
  };

  return (
    <>
      <Menu position="bottom-end" withinPortal shadow="md" width={260}>
        <Menu.Target>
          <Tooltip label="Add to collection" openDelay={300}>
            <ActionIcon
              variant="subtle"
              size="lg"
              aria-label="Add to collection"
            >
              <IconFolderPlus size={20} />
            </ActionIcon>
          </Tooltip>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>Collections</Menu.Label>
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
                closeMenuOnClick={false}
                disabled={busy}
                leftSection={
                  memberIds.has(c.id) ? (
                    <IconCheck size={16} />
                  ) : (
                    <span style={{ width: 16, display: "inline-block" }} />
                  )
                }
                onClick={() => toggle(c.id)}
              >
                {c.name}
              </Menu.Item>
            ))
          )}
          <Menu.Divider />
          <Menu.Item
            leftSection={<IconPlus size={16} />}
            onClick={() => setCreateOpen(true)}
          >
            New collection…
          </Menu.Item>
        </Menu.Dropdown>
      </Menu>

      <CollectionFormModal
        opened={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={(c) =>
          add.mutate({ collectionId: c.id, seriesIds: [seriesId] })
        }
      />
    </>
  );
}
