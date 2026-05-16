import {
  ActionIcon,
  Button,
  Group,
  List,
  Modal,
  Paper,
  Stack,
  Text,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconDeviceMobile, IconShare, IconX } from "@tabler/icons-react";
import { useEffect, useState } from "react";

const DISMISSED_KEY = "codex-pwa-install-dismissed";
const DISMISS_TTL_MS = 1000 * 60 * 60 * 24 * 30;

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

function isStandaloneDisplay(): boolean {
  if (typeof window === "undefined") return false;
  const standaloneMedia = window.matchMedia?.(
    "(display-mode: standalone)",
  ).matches;
  const iosStandalone =
    "standalone" in window.navigator &&
    (window.navigator as { standalone?: boolean }).standalone === true;
  return Boolean(standaloneMedia || iosStandalone);
}

function isIos(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  const isIPad =
    /iPad/.test(ua) ||
    (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
  return /iPhone|iPod/.test(ua) || isIPad;
}

function isDismissed(): boolean {
  try {
    const raw = window.localStorage.getItem(DISMISSED_KEY);
    if (!raw) return false;
    const ts = Number.parseInt(raw, 10);
    if (Number.isNaN(ts)) return false;
    return Date.now() - ts < DISMISS_TTL_MS;
  } catch {
    return false;
  }
}

function recordDismissal() {
  try {
    window.localStorage.setItem(DISMISSED_KEY, String(Date.now()));
  } catch {
    /* storage not available — silently ignore */
  }
}

export function InstallPrompt() {
  const [installEvent, setInstallEvent] =
    useState<BeforeInstallPromptEvent | null>(null);
  const [showIosBanner, setShowIosBanner] = useState(false);
  const [iosModalOpened, { open: openIosModal, close: closeIosModal }] =
    useDisclosure(false);

  useEffect(() => {
    if (isStandaloneDisplay()) return;
    if (isDismissed()) return;

    if (isIos()) {
      setShowIosBanner(true);
      return;
    }

    const handler = (event: Event) => {
      event.preventDefault();
      setInstallEvent(event as BeforeInstallPromptEvent);
    };
    window.addEventListener("beforeinstallprompt", handler);
    const installedHandler = () => {
      setInstallEvent(null);
      setShowIosBanner(false);
      recordDismissal();
    };
    window.addEventListener("appinstalled", installedHandler);
    return () => {
      window.removeEventListener("beforeinstallprompt", handler);
      window.removeEventListener("appinstalled", installedHandler);
    };
  }, []);

  const dismiss = () => {
    recordDismissal();
    setInstallEvent(null);
    setShowIosBanner(false);
  };

  const handleAndroidInstall = async () => {
    if (!installEvent) return;
    await installEvent.prompt();
    const result = await installEvent.userChoice;
    if (result.outcome === "dismissed") {
      recordDismissal();
    }
    setInstallEvent(null);
  };

  if (!installEvent && !showIosBanner) return null;

  return (
    <>
      <Paper
        withBorder
        shadow="md"
        radius="md"
        p="sm"
        style={{
          position: "fixed",
          left: 12,
          right: 12,
          bottom: `calc(12px + env(safe-area-inset-bottom, 0px))`,
          zIndex: 9999,
          maxWidth: 480,
          marginInline: "auto",
        }}
        aria-label="Install Codex"
      >
        <Group justify="space-between" wrap="nowrap" align="flex-start">
          <Group wrap="nowrap" align="flex-start" gap="sm" style={{ flex: 1 }}>
            <IconDeviceMobile size={28} style={{ flexShrink: 0 }} />
            <Stack gap={2} style={{ minWidth: 0 }}>
              <Text fw={600} size="sm">
                Install Codex
              </Text>
              <Text size="xs" c="dimmed">
                {showIosBanner
                  ? "Add to your home screen for a full-screen experience."
                  : "Install the app for offline-ready shell and faster loads."}
              </Text>
              <Group gap="xs" mt={6}>
                {installEvent && (
                  <Button size="xs" onClick={handleAndroidInstall}>
                    Install
                  </Button>
                )}
                {showIosBanner && (
                  <Button size="xs" onClick={openIosModal}>
                    Show me how
                  </Button>
                )}
                <Button size="xs" variant="subtle" onClick={dismiss}>
                  Not now
                </Button>
              </Group>
            </Stack>
          </Group>
          <ActionIcon
            variant="subtle"
            aria-label="Dismiss install prompt"
            onClick={dismiss}
          >
            <IconX size={16} />
          </ActionIcon>
        </Group>
      </Paper>

      <Modal
        opened={iosModalOpened}
        onClose={closeIosModal}
        title="Add Codex to your Home Screen"
        centered
      >
        <Stack gap="md">
          <Text size="sm">
            iOS Safari does not offer a one-tap install button, but you can add
            Codex to your Home Screen in three steps:
          </Text>
          <List type="ordered" spacing="xs" size="sm">
            <List.Item>
              <Group gap={6} align="center" wrap="nowrap">
                <Text size="sm">Tap the Share icon</Text>
                <IconShare size={16} />
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
              .
            </List.Item>
          </List>
          <Text size="xs" c="dimmed">
            Once installed, Codex opens in its own full-screen window, with the
            iOS status bar respected by the reader.
          </Text>
          <Group justify="flex-end">
            <Button variant="subtle" onClick={closeIosModal}>
              Close
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
