import {
  Anchor,
  Divider,
  Group,
  Modal,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { Fragment } from "react";
import type { NewsUpdateAvailableDto } from "@/api/news";
import { MarkdownContent } from "@/components/common/MarkdownContent";
import { formatNewsDate } from "./format";

interface WhatsNewModalProps {
  opened: boolean;
  onClose: () => void;
  update: NewsUpdateAvailableDto;
}

/**
 * Release notes for every Codex release newer than the running one, highest
 * version first: what upgrading would bring.
 */
export function WhatsNewModal({ opened, onClose, update }: WhatsNewModalProps) {
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      size="lg"
      title={
        <Text fw={600} size="lg">
          What's new since {update.currentVersion}
        </Text>
      }
    >
      <Stack gap="md">
        {update.releases.map((release, index) => (
          <Fragment key={release.id}>
            {index > 0 && <Divider />}
            <Stack gap="xs">
              <Group justify="space-between" wrap="nowrap">
                <Title order={3} size="h5">
                  {release.title}
                </Title>
                <Text size="xs" c="dimmed">
                  {formatNewsDate(release.publishedAt)}
                </Text>
              </Group>
              <MarkdownContent>{release.body}</MarkdownContent>
              {release.url && (
                <Anchor
                  href={release.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  size="sm"
                >
                  Full release notes
                </Anchor>
              )}
            </Stack>
          </Fragment>
        ))}
      </Stack>
    </Modal>
  );
}
