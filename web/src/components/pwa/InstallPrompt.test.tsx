import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { InstallPrompt } from "./InstallPrompt";

const ORIGINAL_USER_AGENT = navigator.userAgent;
const ORIGINAL_PLATFORM = navigator.platform;
const ORIGINAL_MAX_TOUCH = navigator.maxTouchPoints;
const ORIGINAL_MATCH_MEDIA = window.matchMedia;

function setUserAgent(ua: string, platform = "MacIntel", maxTouchPoints = 0) {
  Object.defineProperty(navigator, "userAgent", {
    value: ua,
    configurable: true,
  });
  Object.defineProperty(navigator, "platform", {
    value: platform,
    configurable: true,
  });
  Object.defineProperty(navigator, "maxTouchPoints", {
    value: maxTouchPoints,
    configurable: true,
  });
}

function resetUserAgent() {
  Object.defineProperty(navigator, "userAgent", {
    value: ORIGINAL_USER_AGENT,
    configurable: true,
  });
  Object.defineProperty(navigator, "platform", {
    value: ORIGINAL_PLATFORM,
    configurable: true,
  });
  Object.defineProperty(navigator, "maxTouchPoints", {
    value: ORIGINAL_MAX_TOUCH,
    configurable: true,
  });
}

function mockStandalone(matches: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query === "(display-mode: standalone)" ? matches : false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
    onchange: null,
  }));
}

function fireBeforeInstallPrompt(
  prompt = vi.fn(),
  userChoice = Promise.resolve({ outcome: "accepted" as const }),
) {
  const event: Event & {
    prompt: () => Promise<void>;
    userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
  } = Object.assign(new Event("beforeinstallprompt"), {
    prompt: async () => {
      await prompt();
    },
    userChoice,
  });
  window.dispatchEvent(event);
  return event;
}

describe("InstallPrompt", () => {
  beforeEach(() => {
    mockStandalone(false);
    setUserAgent(
      "Mozilla/5.0 (X11; Linux x86_64) Chrome/120.0.0.0",
      "Linux x86_64",
      0,
    );
  });

  afterEach(() => {
    window.matchMedia = ORIGINAL_MATCH_MEDIA;
    resetUserAgent();
    localStorage.clear();
  });

  it("renders nothing initially on a non-iOS platform", () => {
    renderWithProviders(<InstallPrompt />);
    expect(screen.queryByLabelText("Install Codex")).not.toBeInTheDocument();
  });

  it("shows the Install button when beforeinstallprompt fires", async () => {
    renderWithProviders(<InstallPrompt />);
    fireBeforeInstallPrompt();
    expect(await screen.findByLabelText("Install Codex")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install" })).toBeInTheDocument();
  });

  it("calls prompt() and clears the banner when Install is clicked", async () => {
    const user = userEvent.setup();
    const promptSpy = vi.fn();
    renderWithProviders(<InstallPrompt />);
    fireBeforeInstallPrompt(promptSpy);

    await user.click(await screen.findByRole("button", { name: "Install" }));
    expect(promptSpy).toHaveBeenCalled();
  });

  it("persists dismissal in localStorage when Not now is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<InstallPrompt />);
    fireBeforeInstallPrompt();

    await user.click(await screen.findByRole("button", { name: "Not now" }));
    expect(localStorage.getItem("codex-pwa-install-dismissed")).not.toBeNull();
    expect(screen.queryByLabelText("Install Codex")).not.toBeInTheDocument();
  });

  it("does not render when already dismissed within the TTL window", () => {
    localStorage.setItem("codex-pwa-install-dismissed", String(Date.now()));
    renderWithProviders(<InstallPrompt />);
    fireBeforeInstallPrompt();
    expect(screen.queryByLabelText("Install Codex")).not.toBeInTheDocument();
  });

  it("does not render when in standalone display mode", () => {
    mockStandalone(true);
    renderWithProviders(<InstallPrompt />);
    fireBeforeInstallPrompt();
    expect(screen.queryByLabelText("Install Codex")).not.toBeInTheDocument();
  });

  it("renders iOS banner with Show me how button on iPhone Safari", () => {
    setUserAgent(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1",
      "iPhone",
      5,
    );
    renderWithProviders(<InstallPrompt />);
    expect(screen.getByLabelText("Install Codex")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show me how" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Install" }),
    ).not.toBeInTheDocument();
  });

  it("opens the iOS instructions modal on Show me how click", async () => {
    setUserAgent(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 16_6 like Mac OS X) AppleWebKit/605.1.15",
      "iPhone",
      5,
    );
    const user = userEvent.setup();
    renderWithProviders(<InstallPrompt />);

    await user.click(screen.getByRole("button", { name: "Show me how" }));
    expect(
      await screen.findByText("Add Codex to your Home Screen"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Add to Home Screen/i)).toBeInTheDocument();
  });

  it("detects iPad (MacIntel UA with touch points) as iOS", () => {
    setUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/16.6 Safari/605.1.15",
      "MacIntel",
      5,
    );
    renderWithProviders(<InstallPrompt />);
    expect(
      screen.getByRole("button", { name: "Show me how" }),
    ).toBeInTheDocument();
  });
});
