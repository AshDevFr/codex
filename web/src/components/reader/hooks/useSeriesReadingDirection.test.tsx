import { notifications } from "@mantine/notifications";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useReaderStore } from "@/store/readerStore";
import { useSeriesReadingDirection } from "./useSeriesReadingDirection";

vi.mock("@/api/userSeriesReaderSettings", () => ({
  userSeriesReaderSettingsApi: {
    get: vi.fn().mockResolvedValue({}),
    patch: vi.fn().mockResolvedValue({}),
    remove: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock("@/api/seriesMetadata", () => ({
  seriesMetadataApi: {
    patchMetadata: vi.fn().mockResolvedValue({}),
    updateLocks: vi.fn().mockResolvedValue({}),
  },
}));

vi.mock("@mantine/notifications", () => ({
  notifications: { show: vi.fn() },
}));

import { userSeriesReaderSettingsApi } from "@/api/userSeriesReaderSettings";

const SERIES_ID = "series-123";

function wrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({});
  vi.mocked(userSeriesReaderSettingsApi.patch).mockResolvedValue({});
  useReaderStore.setState({ readingDirectionOverride: null });
});

describe("useSeriesReadingDirection", () => {
  it("reports what the user would inherit, and from where", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "ltr",
      inheritedReadingDirection: "rtl",
      inheritedReadingDirectionSource: "series",
    });

    const { result } = renderHook(() => useSeriesReadingDirection(SERIES_ID), {
      wrapper: wrapper(),
    });

    await waitFor(() => {
      expect(result.current.userDirection).toBe("ltr");
    });
    // The book response carries the direction already resolved, so this is the
    // only way the UI can name what dropping the override falls back to.
    expect(result.current.inheritedDirection).toBe("rtl");
    expect(result.current.inheritedSource).toBe("series");
  });

  it("clears the override by writing an explicit null", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "ltr",
      inheritedReadingDirection: "rtl",
      inheritedReadingDirectionSource: "series",
    });

    const { result } = renderHook(() => useSeriesReadingDirection(SERIES_ID), {
      wrapper: wrapper(),
    });
    await waitFor(() => {
      expect(result.current.userDirection).toBe("ltr");
    });

    result.current.clearUserDirection();

    await waitFor(() => {
      expect(userSeriesReaderSettingsApi.patch).toHaveBeenCalledWith(
        SERIES_ID,
        {
          readingDirection: null,
        },
      );
    });
  });

  it("re-renders the open book in the inherited direction", async () => {
    // Clearing alone would leave the store falling back to the user's *global*
    // preference, and ComicReader will not re-seed a book it has already
    // initialised, so the direction has to be applied here or the current page
    // keeps rendering the value that was just dropped.
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "ltr",
      inheritedReadingDirection: "webtoon",
      inheritedReadingDirectionSource: "library",
    });

    const { result } = renderHook(() => useSeriesReadingDirection(SERIES_ID), {
      wrapper: wrapper(),
    });
    await waitFor(() => {
      expect(result.current.inheritedDirection).toBe("webtoon");
    });

    useReaderStore.getState().setReadingDirectionOverride("ltr");
    result.current.clearUserDirection();

    await waitFor(() => {
      expect(useReaderStore.getState().readingDirectionOverride).toBe(
        "webtoon",
      );
    });
  });

  it("falls back to the reader's own settings when nothing is inherited", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "rtl",
    });

    const { result } = renderHook(() => useSeriesReadingDirection(SERIES_ID), {
      wrapper: wrapper(),
    });
    await waitFor(() => {
      expect(result.current.userDirection).toBe("rtl");
    });

    useReaderStore.getState().setReadingDirectionOverride("rtl");
    result.current.clearUserDirection();

    await waitFor(() => {
      expect(useReaderStore.getState().readingDirectionOverride).toBeNull();
    });
  });

  it("surfaces a failed clear instead of swallowing it", async () => {
    vi.mocked(userSeriesReaderSettingsApi.get).mockResolvedValue({
      readingDirection: "ltr",
      inheritedReadingDirection: "rtl",
      inheritedReadingDirectionSource: "series",
    });
    vi.mocked(userSeriesReaderSettingsApi.patch).mockRejectedValue(
      new Error("nope"),
    );

    const { result } = renderHook(() => useSeriesReadingDirection(SERIES_ID), {
      wrapper: wrapper(),
    });
    await waitFor(() => {
      expect(result.current.userDirection).toBe("ltr");
    });

    result.current.clearUserDirection();

    await waitFor(() => {
      expect(notifications.show).toHaveBeenCalledWith(
        expect.objectContaining({ color: "red" }),
      );
    });
  });

  it("does nothing without series context", async () => {
    const { result } = renderHook(() => useSeriesReadingDirection(null), {
      wrapper: wrapper(),
    });

    result.current.clearUserDirection();

    expect(userSeriesReaderSettingsApi.get).not.toHaveBeenCalled();
    expect(userSeriesReaderSettingsApi.patch).not.toHaveBeenCalled();
  });
});
