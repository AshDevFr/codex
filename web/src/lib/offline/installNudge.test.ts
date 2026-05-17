import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  INSTALL_NUDGE_DISMISSED_KEY,
  INSTALL_NUDGE_TTL_MS,
  isIosUserAgent,
  isNudgeDismissed,
  isStandaloneDisplay,
  recordNudgeDismissal,
  shouldShowInstallNudge,
} from "./installNudge";

const ORIGINAL_UA = navigator.userAgent;
const ORIGINAL_PLATFORM = navigator.platform;

function setUserAgent(ua: string, platform: string = ORIGINAL_PLATFORM): void {
  Object.defineProperty(navigator, "userAgent", {
    configurable: true,
    value: ua,
  });
  Object.defineProperty(navigator, "platform", {
    configurable: true,
    value: platform,
  });
}

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  setUserAgent(ORIGINAL_UA, ORIGINAL_PLATFORM);
  vi.restoreAllMocks();
});

describe("isIosUserAgent", () => {
  it("returns true for iPhone UA", () => {
    setUserAgent(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605",
    );
    expect(isIosUserAgent()).toBe(true);
  });

  it("returns false for desktop Chrome UA", () => {
    setUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36");
    expect(isIosUserAgent()).toBe(false);
  });

  it("treats iPadOS (MacIntel with touch points) as iOS", () => {
    Object.defineProperty(navigator, "maxTouchPoints", {
      configurable: true,
      value: 5,
    });
    setUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605",
      "MacIntel",
    );
    expect(isIosUserAgent()).toBe(true);
  });
});

describe("isStandaloneDisplay", () => {
  it("returns false in jsdom by default", () => {
    expect(isStandaloneDisplay()).toBe(false);
  });
});

describe("dismissal persistence", () => {
  it("isNudgeDismissed is false when nothing has been written", () => {
    expect(isNudgeDismissed()).toBe(false);
  });

  it("recordNudgeDismissal writes a timestamp", () => {
    recordNudgeDismissal(1_000_000);
    expect(window.localStorage.getItem(INSTALL_NUDGE_DISMISSED_KEY)).toBe(
      "1000000",
    );
  });

  it("isNudgeDismissed is true within the TTL window", () => {
    recordNudgeDismissal(1_000_000);
    expect(isNudgeDismissed(1_000_000 + 1000)).toBe(true);
  });

  it("isNudgeDismissed is false after the TTL expires", () => {
    recordNudgeDismissal(1_000_000);
    expect(isNudgeDismissed(1_000_000 + INSTALL_NUDGE_TTL_MS + 1)).toBe(false);
  });

  it("malformed timestamps are treated as not-dismissed", () => {
    window.localStorage.setItem(INSTALL_NUDGE_DISMISSED_KEY, "not-a-number");
    expect(isNudgeDismissed()).toBe(false);
  });
});

describe("shouldShowInstallNudge", () => {
  it("is false on non-iOS browsers", () => {
    setUserAgent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36");
    expect(shouldShowInstallNudge()).toBe(false);
  });

  it("is true on a fresh iOS Safari tab", () => {
    setUserAgent(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605",
    );
    expect(shouldShowInstallNudge()).toBe(true);
  });

  it("is false on iOS Safari after dismissal within TTL", () => {
    setUserAgent(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605",
    );
    recordNudgeDismissal(Date.now());
    expect(shouldShowInstallNudge()).toBe(false);
  });
});
