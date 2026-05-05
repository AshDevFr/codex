import { ActionIcon, Tooltip } from "@mantine/core";
import {
  IconCloudDownload,
  IconDownload,
  IconMagnet,
} from "@tabler/icons-react";

interface MediaUrlIconProps {
  url: string;
  kind?: string | null;
}

const KIND_META: Record<
  string,
  { label: string; Icon: React.ComponentType<{ size?: number }> }
> = {
  torrent: { label: "Download .torrent", Icon: IconDownload },
  magnet: { label: "Open magnet link", Icon: IconMagnet },
  direct: { label: "Direct download", Icon: IconDownload },
  other: { label: "Open media link", Icon: IconCloudDownload },
};

export function MediaUrlIcon({ url, kind }: MediaUrlIconProps) {
  const meta = (kind && KIND_META[kind]) ?? KIND_META.other;
  const { label, Icon } = meta;

  return (
    <Tooltip label={label}>
      <ActionIcon
        component="a"
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        variant="subtle"
        size="sm"
        aria-label={label}
      >
        <Icon size={16} />
      </ActionIcon>
    </Tooltip>
  );
}
