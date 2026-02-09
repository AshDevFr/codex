import { notifications } from "@mantine/notifications";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { userPluginsApi } from "@/api/userPlugins";

const OAUTH_POPUP_WIDTH = 600;
const OAUTH_POPUP_HEIGHT = 700;
const OAUTH_POPUP_TIMEOUT_MS = 5 * 60 * 1000; // 5 minutes

/**
 * Hook to handle OAuth flow for user plugins.
 *
 * Opens an OAuth authorization URL in a popup window and listens for the
 * callback redirect. When the OAuth flow completes, the popup closes and
 * the plugin list is refreshed.
 */
export function useOAuthFlow() {
  const popupRef = useRef<Window | null>(null);
  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const queryClient = useQueryClient();

  // Cleanup popup polling on unmount
  useEffect(() => {
    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
    };
  }, []);

  const startOAuthFlow = useCallback(
    async (pluginId: string) => {
      try {
        const { redirectUrl } = await userPluginsApi.startOAuth(pluginId);

        // Calculate popup position (centered)
        const left =
          window.screenX + (window.outerWidth - OAUTH_POPUP_WIDTH) / 2;
        const top =
          window.screenY + (window.outerHeight - OAUTH_POPUP_HEIGHT) / 2;

        // Open popup
        const popup = window.open(
          redirectUrl,
          "oauth_popup",
          `width=${OAUTH_POPUP_WIDTH},height=${OAUTH_POPUP_HEIGHT},left=${left},top=${top},scrollbars=yes`,
        );

        if (!popup) {
          notifications.show({
            title: "Popup blocked",
            message:
              "Please allow popups for this site to connect your account.",
            color: "red",
          });
          return;
        }

        popupRef.current = popup;

        const startedAt = Date.now();

        // Poll for popup close (OAuth callback will redirect back and close)
        pollIntervalRef.current = setInterval(() => {
          if (popup.closed) {
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
            popupRef.current = null;

            // Refresh the plugin list - the OAuth callback stored tokens server-side
            queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
          } else if (Date.now() - startedAt > OAUTH_POPUP_TIMEOUT_MS) {
            // Timeout — stop polling and close the popup
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
            popup.close();
            popupRef.current = null;

            notifications.show({
              title: "OAuth Timeout",
              message:
                "The authentication window was open too long. Please try again.",
              color: "orange",
            });
          }
        }, 500);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "Failed to start OAuth flow";
        notifications.show({
          title: "OAuth Error",
          message,
          color: "red",
        });
      }
    },
    [queryClient],
  );

  return { startOAuthFlow };
}

/**
 * Hook to handle OAuth callback query params on the integrations page.
 *
 * Checks for `?oauth=success&plugin=...` or `?oauth=error` in the URL
 * (set by the backend OAuth callback redirect) and shows notifications.
 */
export function useOAuthCallback() {
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();

  useEffect(() => {
    const oauthResult = searchParams.get("oauth");
    const pluginId = searchParams.get("plugin");

    if (oauthResult === "success") {
      notifications.show({
        title: "Connected",
        message: "Successfully connected your account.",
        color: "green",
      });

      // Refresh plugin data
      queryClient.invalidateQueries({ queryKey: ["user-plugins"] });
      if (pluginId) {
        queryClient.invalidateQueries({
          queryKey: ["user-plugin", pluginId],
        });
      }

      // Clean up URL params
      searchParams.delete("oauth");
      searchParams.delete("plugin");
      setSearchParams(searchParams, { replace: true });
    } else if (oauthResult === "error") {
      notifications.show({
        title: "Connection Failed",
        message: "OAuth authentication failed. Please try again.",
        color: "red",
      });

      // Clean up URL params
      searchParams.delete("oauth");
      searchParams.delete("plugin");
      setSearchParams(searchParams, { replace: true });
    }
  }, [searchParams, setSearchParams, queryClient]);
}
