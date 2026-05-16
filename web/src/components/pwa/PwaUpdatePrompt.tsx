import { useRegisterSW } from "virtual:pwa-register/react";
import { Button, Group, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useEffect, useRef } from "react";

const UPDATE_NOTIFICATION_ID = "pwa-update-available";

export function PwaUpdatePrompt() {
  const {
    needRefresh: [needRefresh, setNeedRefresh],
    updateServiceWorker,
  } = useRegisterSW({
    onRegisterError(error) {
      console.error("Service worker registration failed", error);
    },
  });

  const shownRef = useRef(false);

  useEffect(() => {
    if (!needRefresh) {
      shownRef.current = false;
      return;
    }
    if (shownRef.current) return;
    shownRef.current = true;
    notifications.show({
      id: UPDATE_NOTIFICATION_ID,
      title: "Update available",
      autoClose: false,
      withCloseButton: true,
      onClose: () => setNeedRefresh(false),
      message: (
        <Stack gap="xs" mt={4}>
          <Text size="sm">A new version of Codex is ready.</Text>
          <Group gap="xs">
            <Button
              size="xs"
              onClick={() => {
                notifications.hide(UPDATE_NOTIFICATION_ID);
                updateServiceWorker(true);
              }}
            >
              Reload
            </Button>
            <Button
              size="xs"
              variant="subtle"
              onClick={() => {
                notifications.hide(UPDATE_NOTIFICATION_ID);
                setNeedRefresh(false);
              }}
            >
              Later
            </Button>
          </Group>
        </Stack>
      ),
    });
  }, [needRefresh, setNeedRefresh, updateServiceWorker]);

  return null;
}
