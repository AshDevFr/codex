import {
	createTheme,
	type CSSVariablesResolver,
	type MantineColorsTuple,
	type MantineTheme,
	type ModalProps,
} from "@mantine/core";

// Primary blue color palette (inspired by Komga)
const primaryBlue: MantineColorsTuple = [
	"#e6f2ff",
	"#cce4ff",
	"#99c9ff",
	"#66adff",
	"#3b82f6", // Main blue
	"#2563eb",
	"#1d4ed8",
	"#1e40af",
	"#1e3a8a",
	"#1e3a8a",
];

export const theme = createTheme({
	primaryColor: "blue",
	colors: {
		blue: primaryBlue,
	},

	// Base text color (dark theme defaults)
	black: "#121212",
	white: "#e0e0e0",

	// Component defaults
	defaultRadius: "md",

	// Spacing
	spacing: {
		xs: "0.5rem",
		sm: "0.75rem",
		md: "1rem",
		lg: "1.5rem",
		xl: "2rem",
	},

	// Custom properties for layout
	other: {
		sidebarWidth: 240,
		headerHeight: 64,
	},

	components: {
		AppShell: {
			defaultProps: {
				padding: "md",
			},
		},
		Card: {
			defaultProps: {
				shadow: "sm",
				radius: "md",
			},
		},
		Button: {
			defaultProps: {
				radius: "md",
			},
		},
		Modal: {
			styles: {
				content: {
					// Make modals wider on desktop
					maxWidth: "min(90vw, var(--modal-size))",
				},
			},
			vars: (_theme: MantineTheme, props: ModalProps) => {
				// Increase modal sizes for desktop
				const sizeMap: Record<string, string> = {
					xs: "400px",
					sm: "500px",
					md: "600px",
					lg: "800px",
					xl: "1000px",
				};
				const size = props.size as string | undefined;
				return {
					root: {
						"--modal-size": sizeMap[size || "md"] || size || "600px",
					},
				};
			},
		},
	},
});

// CSS variables resolver for light/dark mode specific customizations
export const cssVariablesResolver: CSSVariablesResolver = (_theme) => ({
	variables: {
		// Scheme-independent variables
	},
	light: {
		// Light mode: Use a clean white background with better contrast
		"--mantine-color-body": "#ffffff",

		// Improve text contrast in light mode
		"--mantine-color-text": "#1a1b1e",
		"--mantine-color-dimmed": "#495057",

		// AppShell colors for light mode
		"--mantine-color-default": "#ffffff",
		"--mantine-color-default-hover": "#f8f9fa",
		"--mantine-color-default-border": "#dee2e6",

		// Card and surface colors
		"--app-shell-main-bg": "#f8f9fa",
		"--card-bg": "#ffffff",
		"--card-border": "#e9ecef",

		// Navbar styling in light mode
		"--mantine-color-gray-light": "#f1f3f5",
		"--mantine-color-gray-light-hover": "#e9ecef",
	},
	dark: {
		// Dark mode keeps existing styling
		"--mantine-color-body": "#121212",
		"--mantine-color-text": "#e0e0e0",
		"--mantine-color-dimmed": "#909296",

		// AppShell colors for dark mode
		"--mantine-color-default": "#1a1a1a",
		"--mantine-color-default-hover": "#2c2c2c",
		"--mantine-color-default-border": "#373a40",

		// Card and surface colors
		"--app-shell-main-bg": "#1a1a1a",
		"--card-bg": "#242424",
		"--card-border": "#373a40",
	},
});
