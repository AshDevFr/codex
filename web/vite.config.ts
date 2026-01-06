import path from "node:path";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
	plugins: [react()],
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},
	server: {
		// Proxy API requests to the backend during development
		proxy: {
			"/api": {
				target: process.env.VITE_API_URL || "http://localhost:8080",
				changeOrigin: true,
			},
			"/docs": {
				target: process.env.VITE_API_URL || "http://localhost:8080",
				changeOrigin: true,
			},
			"/health": {
				target: process.env.VITE_API_URL || "http://localhost:8080",
				changeOrigin: true,
			},
			"/opds": {
				target: process.env.VITE_API_URL || "http://localhost:8080",
				changeOrigin: true,
			},
		},
	},
	build: {
		outDir: "dist",
		sourcemap: true,
		// Optimize build
		rollupOptions: {
			output: {
				manualChunks: {
					vendor: ["react", "react-dom"],
				},
			},
		},
	},
});
