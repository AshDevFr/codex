import { Box, Group, Text, Tooltip, UnstyledButton } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useNews } from "@/hooks/useNews";
import { usePermissions } from "@/hooks/usePermissions";
import { WhatsNewModal } from "./WhatsNewModal";

/**
 * The running version in the sidebar footer. For admins, when a newer Codex
 * release exists, it also names the target version and opens the release
 * notes for everything between the two. Regular users cannot upgrade the
 * server, so they only ever see the plain version.
 */
export function VersionLabel({ version }: { version: string }) {
  const { isAdmin } = usePermissions();
  const { data } = useNews(isAdmin);
  const [opened, { open, close }] = useDisclosure(false);

  const update = isAdmin && data?.enabled ? data.updateAvailable : undefined;

  if (!update) {
    return (
      <Text size="xs" c="dimmed" ta="center" fw={400}>
        v{version}
      </Text>
    );
  }

  const label = `Codex ${update.latestVersion} is available, see what's new since ${update.currentVersion}`;

  return (
    <>
      <Tooltip label={label} withArrow>
        <UnstyledButton
          onClick={open}
          aria-label={label}
          style={{ alignSelf: "center" }}
        >
          <Group gap={6} justify="center" wrap="nowrap">
            <Box
              w={7}
              h={7}
              bg="orange"
              style={{ borderRadius: "50%" }}
              aria-hidden
            />
            <Text size="xs" c="orange" fw={500}>
              v{version} → {update.latestVersion}
            </Text>
          </Group>
        </UnstyledButton>
      </Tooltip>
      <WhatsNewModal opened={opened} onClose={close} update={update} />
    </>
  );
}
