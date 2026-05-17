import { Alert } from "@mantine/core";
import { IconWifiOff } from "@tabler/icons-react";
import { useEffect, useState } from "react";

/**
 * Read the current online state from the browser, defaulting to `true` when
 * `navigator.onLine` is unavailable (SSR / older browsers).
 */
function readOnlineState(): boolean {
  if (typeof navigator === "undefined" || navigator.onLine === undefined) {
    return true;
  }
  return navigator.onLine;
}

/**
 * Thin top banner shown when the browser reports the user is offline. The
 * service worker's NetworkFirst strategy for `/api/` will fall through to
 * cached responses or fail; without this cue the user sees "No content
 * available" with no indication they're disconnected. (U6)
 *
 * Mounted in `AppLayout` below `PluginStatusBanner`. Reader pages keep their
 * intentional chrome-free presentation; the banner does not appear in
 * fullscreen reader mode (it's only inside the AppShell main area).
 */
export function OfflineBanner() {
  const [isOnline, setIsOnline] = useState<boolean>(readOnlineState);

  useEffect(() => {
    const handleOnline = () => setIsOnline(true);
    const handleOffline = () => setIsOnline(false);

    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  }, []);

  if (isOnline) {
    return null;
  }

  return (
    <Alert
      role="status"
      icon={<IconWifiOff size={16} />}
      color="yellow"
      variant="light"
      radius={0}
      style={{ borderBottom: "1px solid var(--mantine-color-yellow-3)" }}
    >
      You&apos;re offline. Showing cached content.
    </Alert>
  );
}
