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
    // Cap the worker pool at half the logical cores. Vitest defaults to one
    // worker per core, and each runs a full jsdom + React + Mantine portal
    // pipeline, so on a 12-thread machine twelve of them starve each other and
    // the heavier interaction tests miss their deadline. The failures were
    // spread across whichever tests happened to lose the race — InstallNudgeModal,
    // TemplateSelector, MediaCard, AddLibraryModal — rather than concentrated in
    // one broken test, which is the signature of contention rather than a bug.
    //
    // Fewer workers with real CPU each finish sooner than more workers thrashing.
    maxWorkers: "50%",
    coverage: {
      provider: "v8",
      // `json-summary` and `lcov` feed the CI coverage report; `html` is for
      // browsing locally (`npm run test:coverage`, then open coverage/index.html).
      reporter: ["text-summary", "json-summary", "lcov", "html"],
      // Without `include`, Vitest only counts files some test happened to
      // import, so a module nothing tests is invisible and the total flatters.
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "node_modules/",
        "src/test/",
        "src/mocks/",
        "**/*.test.{ts,tsx}",
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
