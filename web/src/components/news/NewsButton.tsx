import { ActionIcon, Indicator } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconSpeakerphone } from "@tabler/icons-react";
import { useNews } from "@/hooks/useNews";
import { usePermissions } from "@/hooks/usePermissions";
import { NewsDrawer } from "./NewsDrawer";

/**
 * Header button for Codex release and app news. Admins only, and hidden
 * entirely when the server has news turned off (`news.enabled`).
 */
export function NewsButton() {
  const { isAdmin } = usePermissions();
  const { data } = useNews(isAdmin);
  const [opened, { open, close }] = useDisclosure(false);

  if (!isAdmin || !data?.enabled) {
    return null;
  }

  const unread = data.unreadCount;
  const hasUpdate = Boolean(data.updateAvailable);
  const label = hasUpdate
    ? "News, update available"
    : unread > 0
      ? `News, ${unread} unread`
      : "News";

  return (
    <>
      <Indicator
        disabled={unread === 0 && !hasUpdate}
        color={hasUpdate ? "orange" : "blue"}
        size={9}
        offset={6}
        processing={false}
      >
        <ActionIcon
          variant="subtle"
          onClick={open}
          aria-label={label}
          title={label}
        >
          <IconSpeakerphone size={18} />
        </ActionIcon>
      </Indicator>
      <NewsDrawer opened={opened} onClose={close} news={data} />
    </>
  );
}
