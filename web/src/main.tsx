import { MantineProvider } from "@mantine/core";
import { Notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import { theme } from "./theme";

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

const rootElement = document.getElementById("root");
if (rootElement) {
	createRoot(rootElement).render(
		<StrictMode>
			<MantineProvider theme={theme} defaultColorScheme="dark">
				<Notifications />
				<QueryClientProvider client={queryClient}>
					<App />
				</QueryClientProvider>
			</MantineProvider>
		</StrictMode>,
	);
}
