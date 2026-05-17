import { Button, Group, List, Modal, Stack, Text } from "@mantine/core";
import { IconShare } from "@tabler/icons-react";
import { recordNudgeDismissal } from "@/lib/offline/installNudge";

/**
 * Phase 12 T10: iOS-Safari-only soft modal shown before the first
 * offline download in a session.
 *
 * Explains the platform-specific eviction risk and offers two paths:
 *
 * - "Continue anyway" — proceeds with the download. The intent is a soft
 *   nudge, never a gate: users still get their book.
 * - "Show me how to install" — keeps the modal open while explaining the
 *   Add-to-Home-Screen flow (the same content as InstallPrompt.tsx).
 *   The user closes manually when ready.
 *
 * Either path records dismissal so we do not re-nag on subsequent
 * downloads within the 30-day TTL.
 */

export interface InstallNudgeModalProps {
  opened: boolean;
  /** Invoked after dismissal is recorded — caller proceeds with download. */
  onContinue: () => void;
  /** Invoked when user cancels without continuing. */
  onClose: () => void;
}

export function InstallNudgeModal({
  opened,
  onContinue,
  onClose,
}: InstallNudgeModalProps) {
  const handleContinue = () => {
    recordNudgeDismissal();
    onContinue();
  };

  const handleClose = () => {
    // Closing without continuing still records dismissal: re-prompting on
    // every tap during the same session would be aggressive, and the
    // 30-day TTL gives us a natural retry window.
    recordNudgeDismissal();
    onClose();
  };

  return (
    <Modal
      opened={opened}
      onClose={handleClose}
      title="Save offline on iOS Safari"
      centered
      size="md"
    >
      <Stack gap="md">
        <Text size="sm">
          iOS Safari may clear offline downloads after about a week of
          inactivity unless Codex is installed to your Home Screen. You can
          still download now — this is just a heads-up.
        </Text>

        <Stack gap={4}>
          <Text size="sm" fw={600}>
            To make downloads durable:
          </Text>
          <List type="ordered" spacing={4} size="sm">
            <List.Item>
              <Group gap={6} align="center" wrap="nowrap">
                <Text size="sm">Tap the Share icon</Text>
                <IconShare size={14} />
                <Text size="sm">in Safari's bottom toolbar.</Text>
              </Group>
            </List.Item>
            <List.Item>
              Scroll down and choose{" "}
              <Text span fw={600}>
                Add to Home Screen
              </Text>
              .
            </List.Item>
            <List.Item>
              Confirm the name and tap{" "}
              <Text span fw={600}>
                Add
              </Text>
              , then open Codex from your Home Screen and try again.
            </List.Item>
          </List>
        </Stack>

        <Group justify="flex-end" gap="sm">
          <Button variant="subtle" onClick={handleClose}>
            Not now
          </Button>
          <Button onClick={handleContinue}>Continue anyway</Button>
        </Group>
      </Stack>
    </Modal>
  );
}
