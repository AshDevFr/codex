import path from "node:path";
import react from "@vitejs/plugin-react-swc";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";
import tsconfigPaths from "vite-tsconfig-paths";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tsconfigPaths(),
    VitePWA({
      registerType: "prompt",
      // Skip the SW entirely in dev so MSW's mockServiceWorker.js owns the page.
      // The hand-authored manifest at web/public/manifest.webmanifest is served
      // as-is; vite-plugin-pwa only compiles the service worker source.
      injectRegister: null,
      manifest: false,
      // Use injectManifest (not generateSW) so the SW can own a custom route
      // for per-book offline caches and the downloads broadcast bus. App-shell
      // precache and the runtime caching rules live inside src/sw.ts.
      strategies: "injectManifest",
      srcDir: "src",
      filename: "sw.ts",
      injectManifest: {
        globPatterns: ["**/*.{js,css,html,ico,png,svg,woff2}"],
        // The main app bundle is currently ~2.9 MB. Allow precaching files up
        // to 4 MiB so the full app shell loads instantly in standalone mode.
        // Code-splitting would shrink this; revisit if the bundle grows further.
        maximumFileSizeToCacheInBytes: 4 * 1024 * 1024,
      },
      devOptions: {
        enabled: false,
      },
    }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    host: true,
    // Disable proxy when using mock API (MSW handles all requests)
    proxy:
      process.env.VITE_MOCK_API === "true"
        ? undefined
        : {
            "/api": {
              target: process.env.VITE_API_URL || "http://localhost:8080",
              changeOrigin: true,
              // Better handling of SSE connections and server restarts
              timeout: 60000, // 60s timeout for long-running SSE
              proxyTimeout: 60000,
              ws: true, // Enable WebSocket proxying (for future use)
              // IMPORTANT: Preserve cookies between frontend and backend
              cookieDomainRewrite: {
                localhost: "localhost",
              },
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
                    console.log(
                      `SSE stream: ${req.url} - ${proxyRes.statusCode}`,
                    );
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
            "/komga": {
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
