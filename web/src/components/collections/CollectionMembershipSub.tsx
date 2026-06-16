import { Loader, Menu } from "@mantine/core";
import { IconCheck, IconFolderPlus, IconPlus } from "@tabler/icons-react";
import {
  useAddSeriesToCollection,
  useCollections,
  useCollectionsForSeries,
  useRemoveSeriesFromCollections,
} from "@/hooks/useCollections";

/**
 * Nested submenu for toggling a series across collections, made for embedding
 * inside another Menu (e.g. the media card dropdown).
 *
 * The inline "New collection…" action is delegated to the parent via
 * `onRequestCreate`: the create modal must live outside this Menu, because
 * clicking any item closes the surrounding menu, which would unmount a modal
 * rendered within the dropdown before it could open.
 *
 * The membership query only runs once this component mounts, which Mantine
 * defers until the parent dropdown is opened, so a grid of cards does not fire
 * one request per card on mount.
 *
 * Render only for users with `collections:write`.
 */
export function CollectionMembershipSub({
  seriesId,
  onRequestCreate,
}: {
  seriesId: string;
  onRequestCreate: () => void;
}) {
  const { data: collections, isLoading } = useCollections();
  const { data: memberOf } = useCollectionsForSeries(seriesId);
  const add = useAddSeriesToCollection();
  const remove = useRemoveSeriesFromCollections();

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
    <Menu.Sub>
      <Menu.Sub.Target>
        <Menu.Sub.Item leftSection={<IconFolderPlus size={14} />}>
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
              closeMenuOnClick={false}
              disabled={busy}
              leftSection={
                memberIds.has(c.id) ? (
                  <IconCheck size={16} />
                ) : (
                  <span style={{ width: 16, display: "inline-block" }} />
                )
              }
              onClick={(e: React.MouseEvent) => {
                e.stopPropagation();
                toggle(c.id);
              }}
            >
              {c.name}
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
          New collection…
        </Menu.Item>
      </Menu.Sub.Dropdown>
    </Menu.Sub>
  );
}
