import { ActionIcon, type MantineSize, Tooltip } from "@mantine/core";
import { IconBookmark, IconBookmarkFilled } from "@tabler/icons-react";
import {
  useAddToWantToRead,
  useRemoveFromWantToRead,
} from "@/hooks/useWantToRead";

interface WantToReadButtonProps {
  itemType: "series" | "book";
  id: string;
  /** Current queue membership (from the series/book DTO). */
  wantToRead: boolean | null | undefined;
  size?: MantineSize | number;
}

/**
 * Bookmark-style toggle that adds/removes a series or book from the user's
 * want-to-read queue. State is driven by the `wantToRead` prop; after a toggle
 * the owning detail query is invalidated so the prop refreshes.
 */
export function WantToReadButton({
  itemType,
  id,
  wantToRead,
  size = "lg",
}: WantToReadButtonProps) {
  const add = useAddToWantToRead();
  const remove = useRemoveFromWantToRead();

  const active = Boolean(wantToRead);
  const pending = add.isPending || remove.isPending;
  const label = active ? "Remove from Want to Read" : "Add to Want to Read";

  const toggle = () => {
    if (pending) return;
    if (active) {
      remove.mutate({ itemType, id });
    } else {
      add.mutate({ itemType, id });
    }
  };

  return (
    <Tooltip label={label} openDelay={300}>
      <ActionIcon
        variant={active ? "filled" : "subtle"}
        color="yellow"
        size={size}
        loading={pending}
        onClick={toggle}
        aria-label={label}
        aria-pressed={active}
      >
        {active ? <IconBookmarkFilled size={20} /> : <IconBookmark size={20} />}
      </ActionIcon>
    </Tooltip>
  );
}
