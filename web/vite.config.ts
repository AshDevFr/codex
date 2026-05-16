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
      // as-is; vite-plugin-pwa only generates the service worker.
      injectRegister: null,
      manifest: false,
      strategies: "generateSW",
      workbox: {
        globPatterns: ["**/*.{js,css,html,ico,png,svg,woff2}"],
        // The main app bundle is currently ~2.9 MB. Allow precaching files up
        // to 4 MiB so the full app shell loads instantly in standalone mode.
        // Code-splitting would shrink this; revisit if the bundle grows further.
        maximumFileSizeToCacheInBytes: 4 * 1024 * 1024,
        navigateFallback: "/index.html",
        // Don't try to handle backend routes from the SPA shell.
        navigateFallbackDenylist: [
          /^\/api\//,
          /^\/opds\//,
          /^\/komga\//,
          /^\/docs\//,
          /^\/health$/,
        ],
        cleanupOutdatedCaches: true,
        clientsClaim: false,
        skipWaiting: false,
        runtimeCaching: [
          {
            // Backend API: always go to network so auth + freshness win,
            // but fall back to cache when offline so a recently-viewed list
            // remains visible. Short cache TTL prevents stale auth state.
            urlPattern: ({ url }) => url.pathname.startsWith("/api/"),
            handler: "NetworkFirst",
            options: {
              cacheName: "codex-api",
              networkTimeoutSeconds: 5,
              expiration: {
                maxEntries: 64,
                maxAgeSeconds: 60 * 5,
              },
              cacheableResponse: { statuses: [0, 200] },
            },
          },
          {
            // Icons + fonts rarely change: cache aggressively.
            urlPattern: ({ request }) =>
              request.destination === "font" || request.destination === "image",
            handler: "CacheFirst",
            options: {
              cacheName: "codex-assets",
              expiration: {
                maxEntries: 128,
                maxAgeSeconds: 60 * 60 * 24 * 30,
              },
              cacheableResponse: { statuses: [0, 200] },
            },
          },
        ],
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
