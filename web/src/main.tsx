import { MantineProvider } from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { InstallPrompt, PwaUpdatePrompt } from "./components/pwa";
import { ThemeSync } from "./components/ThemeSync.tsx";
import { MotionProvider } from "./lib/motion/MotionProvider";
import { initObservability } from "./lib/observability";
import { installOutboxDrainListeners } from "./lib/offline/outbox";
import { cssVariablesResolver, theme } from "./theme";

// Import Mantine styles
import "@mantine/core/styles.css";
import "@mantine/notifications/styles.css";
import "./index.css";

// Create React Query client
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5000, // 5 seconds - reduced for real-time updates
      refetchOnWindowFocus: true, // Refetch when switching tabs
      refetchOnMount: true, // Refetch when component mounts
      refetchOnReconnect: true, // Refetch when network reconnects
      retry: (failureCount, error) => {
        // Don't retry on client errors (4xx) - axios handles 429 retries internally
        const apiError = error as { error?: string };
        if (
          apiError?.error === "rate_limit_exceeded" ||
          apiError?.error?.startsWith("4")
        ) {
          return false;
        }
        // Retry server errors (5xx) and network errors up to 1 time
        return failureCount < 1;
      },
    },
    mutations: {
      retry: false, // Don't retry mutations by default
    },
  },
});

// Initialize mock service worker in development
async function enableMocking() {
  if (import.meta.env.VITE_MOCK_API !== "true") {
    return;
  }

  const { startMockServiceWorker } = await import("./mocks/browser");
  return startMockServiceWorker();
}

// Drain the offline write outbox whenever the browser comes back online
// or the tab regains focus. Safe to install before render: the listeners
// no-op if there is nothing queued, and double-install is guarded.
installOutboxDrainListeners();

// Kick off the OTel web SDK bootstrap. The call returns immediately;
// the network round-trip + SDK code-split happen in the background and
// never block render. If the server says RUM is disabled we never load
// the SDK bundle in the first place.
void initObservability();

// Start the application after mocking is ready
enableMocking().then(() => {
  const rootElement = document.getElementById("root");
  if (rootElement) {
    createRoot(rootElement).render(
      <StrictMode>
        <MantineProvider
          theme={theme}
          defaultColorScheme="dark"
          cssVariablesResolver={cssVariablesResolver}
        >
          <MotionProvider>
            <ThemeSync />
            <Notifications zIndex={10000} />
            {import.meta.env.PROD && <PwaUpdatePrompt />}
            {import.meta.env.PROD && <InstallPrompt />}
            <QueryClientProvider client={queryClient}>
              <App />
            </QueryClientProvider>
          </MotionProvider>
        </MantineProvider>
      </StrictMode>,
    );
  }
});
