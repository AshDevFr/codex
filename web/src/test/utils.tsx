import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type RenderOptions, render } from "@testing-library/react";
import { MemoryRouter, type MemoryRouterProps } from "react-router-dom";
import { theme } from "@/theme";

type CustomRenderOptions = RenderOptions & {
	queryClient?: QueryClient;
	initialEntries?: MemoryRouterProps["initialEntries"];
	initialIndex?: MemoryRouterProps["initialIndex"];
};

// Create a custom render function that includes all providers
export function renderWithProviders(
	ui: React.ReactElement,
	{
		queryClient = new QueryClient({
			defaultOptions: {
				queries: { retry: false },
				mutations: { retry: false },
			},
		}),
		initialEntries,
		initialIndex,
		...renderOptions
	}: CustomRenderOptions = {},
) {
	function Wrapper({ children }: { children: React.ReactNode }) {
		return (
			<MantineProvider theme={theme} defaultColorScheme="dark">
				<QueryClientProvider client={queryClient}>
					<MemoryRouter initialEntries={initialEntries} initialIndex={initialIndex}>
						{children}
					</MemoryRouter>
				</QueryClientProvider>
			</MantineProvider>
		);
	}

	return { ...render(ui, { wrapper: Wrapper, ...renderOptions }), queryClient };
}

// Re-export everything from testing library
export * from "@testing-library/react";
export { default as userEvent } from "@testing-library/user-event";
