import { createTheme, type MantineColorsTuple } from "@mantine/core";

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

	// Dark theme colors (matching Komga's aesthetic)
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
			vars: (_theme, props) => {
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
