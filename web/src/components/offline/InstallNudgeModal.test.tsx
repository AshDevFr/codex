import { IDBFactory } from "fake-indexeddb";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type MockInstance,
  vi,
} from "vitest";
import { _resetForTests, setDbContext } from "@/lib/offline/db";
import * as downloadManagerModule from "@/lib/offline/downloadManager";
import { _resetPersistenceForTests } from "@/lib/offline/downloadManager";
import {
  INSTALL_NUDGE_DISMISSED_KEY,
  isNudgeDismissed,
} from "@/lib/offline/installNudge";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { DownloadButton } from "./DownloadButton";

const ORIGINAL_UA = navigator.userAgent;
const ORIGINAL_PLATFORM = navigator.platform;

let downloadSpy: MockInstance<
  typeof downloadManagerModule.downloadSingleFileBook
> | null = null;

function setIosUserAgent(): void {
  Object.defineProperty(navigator, "userAgent", {
    configurable: true,
    value:
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605",
  });
  Object.defineProperty(navigator, "platform", {
    configurable: true,
    value: "iPhone",
  });
}

function restoreUserAgent(): void {
  Object.defineProperty(navigator, "userAgent", {
    configurable: true,
    value: ORIGINAL_UA,
  });
  Object.defineProperty(navigator, "platform", {
    configurable: true,
    value: ORIGINAL_PLATFORM,
  });
}

beforeEach(() => {
  setDbContext({ indexedDB: new IDBFactory() });
  window.localStorage.clear();
});

afterEach(() => {
  setDbContext(null);
  _resetForTests();
  _resetPersistenceForTests();
  restoreUserAgent();
  downloadSpy?.mockRestore();
  downloadSpy = null;
});

// Each test chains a menuitem interaction inside Mantine's portal/transition
// pipeline; under heavy parallel-test load the menu finder can need a few
// seconds to settle. Bump the per-test timeout so the chain fits comfortably.
describe("DownloadButton + InstallNudgeModal (T10)", { timeout: 20000 }, () => {
  it("shows the nudge on first tap from an iOS Safari tab, then downloads after Continue", async () => {
    setIosUserAgent();
    downloadSpy = vi
      .spyOn(downloadManagerModule, "downloadSingleFileBook")
      .mockResolvedValue({ bookId: "book-ios", bytes: 4 });

    renderWithProviders(<DownloadButton bookId="book-ios" fileFormat="epub" />);
    const trigger = await screen.findByRole("button", {
      name: /^download options$/i,
    });
    await userEvent.click(trigger);
    await userEvent.click(
      await screen.findByRole(
        "menuitem",
        { name: /save for offline/i },
        { timeout: 5000 },
      ),
    );

    // Modal appears with the iOS-specific copy.
    expect(
      await screen.findByText(/iOS Safari may clear offline downloads/i),
    ).toBeInTheDocument();
    // Manager has NOT been called yet — nudge is a gate on the first call.
    expect(downloadSpy).not.toHaveBeenCalled();

    await userEvent.click(
      screen.getByRole("button", { name: /continue anyway/i }),
    );

    await waitFor(() => {
      expect(downloadSpy).toHaveBeenCalledTimes(1);
    });
    expect(isNudgeDismissed()).toBe(true);
  });

  it("subsequent taps within the TTL skip the modal and download immediately", async () => {
    setIosUserAgent();
    // Pretend the user previously dismissed.
    window.localStorage.setItem(
      INSTALL_NUDGE_DISMISSED_KEY,
      String(Date.now()),
    );
    downloadSpy = vi
      .spyOn(downloadManagerModule, "downloadSingleFileBook")
      .mockResolvedValue({ bookId: "book-ios-2", bytes: 4 });

    renderWithProviders(
      <DownloadButton bookId="book-ios-2" fileFormat="epub" />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /^download options$/i }),
    );
    await userEvent.click(
      await screen.findByRole(
        "menuitem",
        { name: /save for offline/i },
        { timeout: 5000 },
      ),
    );

    await waitFor(() => {
      expect(downloadSpy).toHaveBeenCalledTimes(1);
    });
    expect(
      screen.queryByText(/iOS Safari may clear offline downloads/i),
    ).not.toBeInTheDocument();
  });

  it("does not show the nudge on non-iOS browsers", async () => {
    // userAgent stays as the test runner default (jsdom).
    downloadSpy = vi
      .spyOn(downloadManagerModule, "downloadSingleFileBook")
      .mockResolvedValue({ bookId: "book-desktop", bytes: 4 });

    renderWithProviders(
      <DownloadButton bookId="book-desktop" fileFormat="epub" />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /^download options$/i }),
    );
    await userEvent.click(
      await screen.findByRole(
        "menuitem",
        { name: /save for offline/i },
        { timeout: 5000 },
      ),
    );

    await waitFor(() => {
      expect(downloadSpy).toHaveBeenCalledTimes(1);
    });
    expect(
      screen.queryByText(/iOS Safari may clear offline downloads/i),
    ).not.toBeInTheDocument();
  });
});
