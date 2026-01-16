import { MantineProvider } from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { ThemeSync } from "./components/ThemeSync.tsx";
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
			retry: 1,
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
					<ThemeSync />
					<Notifications zIndex={10000} />
					<QueryClientProvider client={queryClient}>
						<App />
					</QueryClientProvider>
				</MantineProvider>
			</StrictMode>,
		);
	}
});
