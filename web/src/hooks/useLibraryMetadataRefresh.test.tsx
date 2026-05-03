import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import type { ReactNode } from "react";
import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import {
  useDryRunMetadataRefresh,
  useFieldGroups,
  useMetadataRefreshConfig,
  useRunMetadataRefreshNow,
  useUpdateMetadataRefreshConfig,
} from "./useLibraryMetadataRefresh";

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "bypass" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const LIBRARY_ID = "11111111-1111-1111-1111-111111111111";

const baseConfig = {
  enabled: true,
  cronSchedule: "0 4 * * *",
  timezone: null,
  fieldGroups: ["ratings"],
  extraFields: [],
  providers: ["plugin:mangabaka"],
  existingSourceIdsOnly: true,
  skipRecentlySyncedWithinS: 3600,
  maxConcurrency: 4,
};

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

describe("useMetadataRefreshConfig", () => {
  it("fetches the config for the given library", async () => {
    server.use(
      http.get(`/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`, () =>
        HttpResponse.json(baseConfig),
      ),
    );

    const { result } = renderHook(() => useMetadataRefreshConfig(LIBRARY_ID), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(baseConfig);
  });

  it("does not fetch when libraryId is missing", () => {
    const { result } = renderHook(() => useMetadataRefreshConfig(undefined), {
      wrapper: createWrapper(),
    });
    expect(result.current.fetchStatus).toBe("idle");
  });
});

describe("useFieldGroups", () => {
  it("returns the catalog", async () => {
    const groups = [
      { id: "ratings", label: "Ratings", fields: ["rating"] },
      { id: "status", label: "Status", fields: ["status"] },
    ];
    server.use(
      http.get("/api/v1/metadata-refresh/field-groups", () =>
        HttpResponse.json(groups),
      ),
    );

    const { result } = renderHook(() => useFieldGroups(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(groups);
  });
});

describe("useUpdateMetadataRefreshConfig", () => {
  it("PATCHes and updates the cache on success", async () => {
    const updated = { ...baseConfig, fieldGroups: ["ratings", "status"] };
    server.use(
      http.patch(`/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`, () =>
        HttpResponse.json(updated),
      ),
    );

    const { result } = renderHook(
      () => useUpdateMetadataRefreshConfig(LIBRARY_ID),
      { wrapper: createWrapper() },
    );

    let response: typeof updated | undefined;
    await act(async () => {
      response = await result.current.mutateAsync({
        fieldGroups: ["ratings", "status"],
      });
    });

    expect(response).toEqual(updated);
    await waitFor(() => expect(result.current.data).toEqual(updated));
  });
});

describe("useRunMetadataRefreshNow", () => {
  it("returns the task id from the run-now endpoint", async () => {
    const taskId = "22222222-2222-2222-2222-222222222222";
    server.use(
      http.post(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh/run-now`,
        () => HttpResponse.json({ taskId }),
      ),
    );

    const { result } = renderHook(() => useRunMetadataRefreshNow(LIBRARY_ID), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      const res = await result.current.mutateAsync();
      expect(res).toEqual({ taskId });
    });
  });
});

describe("useDryRunMetadataRefresh", () => {
  it("returns dry-run results", async () => {
    const dryRun = {
      sample: [],
      totalEligible: 0,
      estSkippedNoId: 0,
      estSkippedRecentlySynced: 0,
    };
    server.use(
      http.post(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh/dry-run`,
        () => HttpResponse.json(dryRun),
      ),
    );

    const { result } = renderHook(() => useDryRunMetadataRefresh(LIBRARY_ID), {
      wrapper: createWrapper(),
    });

    await act(async () => {
      const res = await result.current.mutateAsync({
        configOverride: null,
        sampleSize: 5,
      });
      expect(res).toEqual(dryRun);
    });
  });
});
