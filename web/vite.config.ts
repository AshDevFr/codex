import path from "node:path";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";
import tsconfigPaths from "vite-tsconfig-paths";

// https://vite.dev/config/
export default defineConfig({
	plugins: [react(), tsconfigPaths()],
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
				// Better handling of SSE connections and server restarts
				timeout: 60000, // 60s timeout for long-running SSE
				proxyTimeout: 60000,
				ws: true, // Enable WebSocket proxying (for future use)
				configure: (proxy, _options) => {
					proxy.on("error", (err, _req, _res) => {
						console.log("Proxy error:", err.message);
					});
					proxy.on("proxyReq", (proxyReq, req, _res) => {
						// Don't cache SSE connections
						if (
							req.url?.includes("/stream") ||
							req.headers.accept?.includes("text/event-stream")
						) {
							proxyReq.setHeader("Cache-Control", "no-cache");
							proxyReq.setHeader("Connection", "keep-alive");
						}
					});
					proxy.on("proxyRes", (proxyRes, req, _res) => {
						if (req.url?.includes("/stream")) {
							console.log(`SSE stream: ${req.url} - ${proxyRes.statusCode}`);
						}
					});
				},
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
