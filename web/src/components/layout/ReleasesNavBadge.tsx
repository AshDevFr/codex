import { Badge } from "@mantine/core";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";

/**
 * Inline badge shown next to the "Releases" nav entry. Hidden when the
 * counter is zero so it doesn't add visual noise to the sidebar.
 */
export function ReleasesNavBadge() {
  const unseen = useReleaseAnnouncementsStore((s) => s.unseenCount);
  if (unseen <= 0) return null;
  return (
    <Badge color="orange" variant="filled" size="sm" circle>
      {unseen > 99 ? "99+" : unseen}
    </Badge>
  );
}
