import { AppShell } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import { act, renderWithProviders, screen, userEvent } from "@/test/utils";
import { Header } from "./Header";

vi.mock("@/api/userPreferences", () => ({
  userPreferencesApi: {
    getAll: vi.fn(),
    get: vi.fn(),
    set: vi.fn().mockResolvedValue(undefined),
    bulkSet: vi.fn(),
    delete: vi.fn(),
  },
}));

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  useUserPreferencesStore.setState({
    preferences: { "ui.theme": "dark" },
    isLoaded: true,
    loadError: null,
  });
});

afterEach(() => {
  vi.useRealTimers();
  useUserPreferencesStore.setState({
    preferences: {},
    isLoaded: false,
    loadError: null,
  });
});

function renderHeader() {
  return renderWithProviders(
    <AppShell>
      <Header
        mobileOpened={false}
        toggleMobile={() => {}}
        toggleDesktop={() => {}}
      />
    </AppShell>,
  );
}

describe("Header theme toggle", () => {
  it("marks the icon as spinning on toggle and clears it after the animation", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    renderHeader();

    const toggle = screen.getByRole("button", { name: "Toggle color scheme" });
    const iconBefore = toggle.querySelector(".theme-toggle-icon");
    expect(iconBefore).not.toBeNull();
    expect(iconBefore?.className).not.toContain("theme-toggle-icon--spinning");

    await user.click(toggle);

    const iconDuring = toggle.querySelector(".theme-toggle-icon");
    // The spinning class is what the keyframe in index.css latches on to.
    // If this drops, the click stops feeling animated.
    expect(iconDuring?.className).toContain("theme-toggle-icon--spinning");

    // The component clears the spin marker shortly after the keyframe ends,
    // so the next click can re-trigger the animation.
    act(() => {
      vi.advanceTimersByTime(500);
    });

    const iconAfter = toggle.querySelector(".theme-toggle-icon");
    expect(iconAfter?.className).not.toContain("theme-toggle-icon--spinning");
  });
});
