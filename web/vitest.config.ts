import path from "node:path";
import react from "@vitejs/plugin-react-swc";
import tsconfigPaths from "vite-tsconfig-paths";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react(), tsconfigPaths()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    // Vitest's 5s default is sized for fast unit tests. 74 of these files drive
    // the UI through `userEvent`, which awaits a React render per keystroke, and
    // the runner gives every core its own jsdom environment. Under that
    // contention the heavier interaction tests legitimately exceed 5s: the full
    // suite failed a different one on each run while every one of them passed in
    // isolation. The budget was wrong for the environment, not the tests.
    //
    // Individual `findBy*` calls keep their own shorter timeouts, so a genuinely
    // missing element still fails quickly. What this costs is that a truly hung
    // test now takes 20s to report rather than 5s.
    testTimeout: 20000,
    hookTimeout: 20000,
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "src/test/",
        "**/*.d.ts",
        "**/*.config.*",
        "**/mockData",
        "dist/",
      ],
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
