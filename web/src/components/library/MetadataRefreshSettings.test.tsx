import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { MetadataRefreshSettings } from "./MetadataRefreshSettings";

vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

// Avoid pulling task-progress SSE machinery into tests.
vi.mock("@/hooks/useTaskProgress", () => ({
  useTaskProgress: () => ({
    activeTasks: [],
    connectionState: "disconnected",
    pendingCounts: {},
    getTask: () => undefined,
    getTasksByStatus: () => [],
    getTasksByLibrary: () => [],
  }),
}));

// Pin the scheduler timezone so the placeholder text is stable in tests.
vi.mock("@/hooks/useSchedulerTimezone", () => ({
  useSchedulerTimezone: () => "UTC",
}));

const LIBRARY_ID = "11111111-1111-1111-1111-111111111111";

const baseConfig = {
  enabled: true,
  cronSchedule: "0 4 * * *",
  timezone: null,
  fieldGroups: ["ratings", "status"],
  extraFields: [],
  providers: ["plugin:mangabaka"],
  existingSourceIdsOnly: true,
  skipRecentlySyncedWithinS: 3600,
  maxConcurrency: 4,
};

const fieldGroups = [
  { id: "ratings", label: "Ratings", fields: ["rating"] },
  { id: "status", label: "Status", fields: ["status", "year"] },
  { id: "counts", label: "Counts", fields: ["totalChapters"] },
];

const pluginActions = {
  scope: "series:bulk",
  actions: [
    {
      pluginId: "22222222-2222-2222-2222-222222222222",
      pluginName: "mangabaka",
      pluginDisplayName: "MangaBaka",
      actionType: "metadata_search",
      label: "Fetch from MangaBaka",
    },
  ],
};

const server = setupServer(
  http.get(`/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`, () =>
    HttpResponse.json(baseConfig),
  ),
  http.get("/api/v1/metadata-refresh/field-groups", () =>
    HttpResponse.json(fieldGroups),
  ),
  http.get("/api/v1/plugins/actions", () => HttpResponse.json(pluginActions)),
);

beforeAll(() => server.listen({ onUnhandledRequest: "bypass" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

beforeEach(() => {
  vi.clearAllMocks();
});

describe("MetadataRefreshSettings", () => {
  it("hydrates the form from the saved config", async () => {
    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    await waitFor(() =>
      expect(screen.getByLabelText(/Enable scheduled refresh/i)).toBeChecked(),
    );

    expect(
      screen.getByLabelText(/Use existing source IDs only/i),
    ).toBeChecked();
    // Concurrency hydrated to 4
    expect(screen.getByLabelText(/Max concurrency/i)).toHaveValue("4");
  });

  it("PATCHes the config when Save is clicked", async () => {
    let captured: unknown = null;
    server.use(
      http.patch(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`,
        async ({ request }) => {
          captured = await request.json();
          return HttpResponse.json({ ...baseConfig, enabled: false });
        },
      ),
    );

    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    const enableCheckbox = await screen.findByLabelText(
      /Enable scheduled refresh/i,
    );
    const user = userEvent.setup();
    await user.click(enableCheckbox);

    const saveButton = screen.getByRole("button", { name: "Save" });
    await user.click(saveButton);

    await waitFor(() => expect(captured).not.toBeNull());
    expect((captured as { enabled: boolean }).enabled).toBe(false);
  });

  it("triggers run-now and shows the response", async () => {
    const taskId = "33333333-3333-3333-3333-333333333333";
    let runNowCalled = false;
    server.use(
      http.post(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh/run-now`,
        () => {
          runNowCalled = true;
          return HttpResponse.json({ taskId });
        },
      ),
    );

    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    const runNowButton = await screen.findByRole("button", {
      name: /Run now/i,
    });
    const user = userEvent.setup();
    await user.click(runNowButton);

    await waitFor(() => expect(runNowCalled).toBe(true));
  });

  it("opens the dry-run modal with results", async () => {
    server.use(
      http.post(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh/dry-run`,
        () =>
          HttpResponse.json({
            sample: [],
            totalEligible: 7,
            estSkippedNoId: 0,
            estSkippedRecentlySynced: 0,
          }),
      ),
    );

    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    const previewButton = await screen.findByRole("button", {
      name: /Preview changes/i,
    });
    await waitFor(() => expect(previewButton).not.toBeDisabled());

    const user = userEvent.setup();
    await user.click(previewButton);

    await waitFor(() =>
      expect(screen.getByText("Dry-run preview")).toBeInTheDocument(),
    );
    await waitFor(() => expect(screen.getByText("7")).toBeInTheDocument());
  });

  it("renders selected providers as inheriting the library by default", async () => {
    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    await waitFor(() =>
      expect(
        screen.getByTestId("override-card-plugin:mangabaka"),
      ).toBeInTheDocument(),
    );

    const card = screen.getByTestId("override-card-plugin:mangabaka");
    // Default state: no override saved → "Inherits library" badge.
    expect(card).toHaveTextContent(/Inherits library/i);
    expect(card).not.toHaveTextContent(/^Custom$/i);
  });

  it("hydrates an existing per-provider override and surfaces the Custom badge", async () => {
    server.use(
      http.get(`/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`, () =>
        HttpResponse.json({
          ...baseConfig,
          perProviderOverrides: {
            "plugin:mangabaka": {
              fieldGroups: ["ratings"],
              extraFields: [],
            },
          },
        }),
      ),
    );

    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    await waitFor(() =>
      expect(
        screen.getByTestId("override-card-plugin:mangabaka"),
      ).toBeInTheDocument(),
    );

    const card = screen.getByTestId("override-card-plugin:mangabaka");
    expect(card).toHaveTextContent(/Custom/i);
    expect(card).toHaveTextContent(/Reset to inherit/i);
  });

  it("PATCHes the override map when the user resets a provider", async () => {
    server.use(
      http.get(`/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`, () =>
        HttpResponse.json({
          ...baseConfig,
          perProviderOverrides: {
            "plugin:mangabaka": {
              fieldGroups: ["ratings"],
              extraFields: [],
            },
          },
        }),
      ),
    );

    let captured: { perProviderOverrides?: unknown } | null = null;
    server.use(
      http.patch(
        `/api/v1/libraries/${LIBRARY_ID}/metadata-refresh`,
        async ({ request }) => {
          captured = (await request.json()) as {
            perProviderOverrides?: unknown;
          };
          return HttpResponse.json(baseConfig);
        },
      ),
    );

    renderWithProviders(<MetadataRefreshSettings libraryId={LIBRARY_ID} />);

    const resetButton = await screen.findByRole("button", {
      name: /Reset to inherit/i,
    });
    const user = userEvent.setup();
    await user.click(resetButton);

    const saveButton = screen.getByRole("button", { name: "Save" });
    await user.click(saveButton);

    await waitFor(() => expect(captured).not.toBeNull());
    // Reset wipes the only override, so the patch should explicitly null
    // out perProviderOverrides (clears it on the server).
    expect(captured!.perProviderOverrides).toBeNull();
  });
});
