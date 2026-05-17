import { Box, Group, Text, Transition } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconHandFinger } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";

const STORAGE_KEY = "codex:reader-hint-shown";
const AUTO_HIDE_MS = 4000;

/**
 * Returns true once the hint has been shown in this session. Reads
 * sessionStorage on each call; safe under SSR / private mode where the API
 * can throw.
 */
function hasBeenShown(): boolean {
  try {
    return sessionStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

/**
 * Mark the hint as shown for the remainder of this session.
 */
function markShown(): void {
  try {
    sessionStorage.setItem(STORAGE_KEY, "1");
  } catch {
    // Ignore storage errors (private mode, quota, etc.)
  }
}

/**
 * Test-only: reset the session flag so the hint shows again. Used by tests
 * that need to verify first-run vs. subsequent-mount behavior.
 */
export function __resetReaderFirstRunHintForTests(): void {
  try {
    sessionStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore
  }
}

interface ReaderFirstRunHintProps {
  /**
   * When false, the hint won't render even on first visit. Readers use this
   * to suppress the hint while loading or while another overlay is showing.
   */
  enabled?: boolean;
}

/**
 * One-time mobile reader hint: "Tap the center to show controls."
 *
 * The auto-hiding toolbar is hidden by default 3s after mount, leaving a
 * first-time mobile user with no obvious way to bring it back (CBZ/PDF tap
 * zones split into left/center/right). This component shows a low-contrast
 * pill in the lower-center of the screen for the first reader open of a
 * session, then fades out on tap or after a short timeout.
 *
 * - Phone-only (gated on `MOBILE_MEDIA_QUERY`).
 * - Once per browser session (sessionStorage flag). Does not return on the
 *   next book open within the same tab.
 * - Tapping the hint itself dismisses it; the hint also dismisses on any
 *   pointer interaction elsewhere on the reader (handled by the parent
 *   reader's tap zones, which call `onDismiss` indirectly via the toolbar
 *   toggle).
 */
export function ReaderFirstRunHint({
  enabled = true,
}: ReaderFirstRunHintProps) {
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;
  // Compute initial visibility synchronously so we don't flash the hint on
  // subsequent mounts within the same session.
  const [visible, setVisible] = useState<boolean>(() => {
    if (!enabled) return false;
    return !hasBeenShown();
  });

  useEffect(() => {
    if (!visible || !isMobile) return;
    // Mark as shown immediately so a rapid re-mount in the same session
    // doesn't show it twice, then schedule the fade-out.
    markShown();
    const timer = setTimeout(() => setVisible(false), AUTO_HIDE_MS);
    return () => clearTimeout(timer);
  }, [visible, isMobile]);

  // Don't render at all on desktop or if dismissed/disabled.
  if (!isMobile || !enabled) {
    return null;
  }

  return (
    <Transition mounted={visible} transition="fade" duration={250}>
      {(styles) => (
        <Box
          role="button"
          tabIndex={0}
          aria-label="Dismiss reader hint"
          onClick={() => setVisible(false)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              setVisible(false);
            }
          }}
          style={{
            ...styles,
            position: "absolute",
            // Sit above the page content but below the toolbar/bottom-bar so
            // those still receive taps when visible.
            zIndex: 50,
            // Lower-center so it doesn't compete with the toolbar when both
            // are momentarily visible right after mount.
            bottom: "calc(96px + env(safe-area-inset-bottom, 0px))",
            left: "50%",
            transform: "translateX(-50%)",
            background: "rgba(0, 0, 0, 0.75)",
            color: "#fff",
            borderRadius: 999,
            padding: "8px 14px",
            pointerEvents: "auto",
            cursor: "pointer",
            maxWidth: "calc(100vw - 32px)",
          }}
        >
          <Group gap={8} wrap="nowrap" align="center">
            <IconHandFinger size={18} stroke={1.75} />
            <Text size="sm" style={{ color: "#fff" }}>
              Tap the center to show controls
            </Text>
          </Group>
        </Box>
      )}
    </Transition>
  );
}
