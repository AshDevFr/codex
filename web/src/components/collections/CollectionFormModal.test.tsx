import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { collectionsApi } from "@/api/collections";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import { CollectionFormModal } from "./CollectionFormModal";

vi.mock("@/api/collections", () => ({
  collectionsApi: {
    create: vi.fn().mockResolvedValue({
      id: "c1",
      name: "Batman",
      ordered: true,
      condition: null,
      automatic: false,
      seriesCount: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    }),
    update: vi.fn().mockResolvedValue({
      id: "c1",
      name: "Batman",
      ordered: false,
      condition: null,
      automatic: false,
      seriesCount: 0,
      createdAt: "2026-06-15T00:00:00Z",
      updatedAt: "2026-06-15T00:00:00Z",
    }),
  },
}));

vi.mock("@/api/libraries", () => ({
  librariesApi: { getAll: vi.fn(() => Promise.resolve([])) },
}));

function existingCollection(overrides: Record<string, unknown> = {}) {
  return {
    id: "c1",
    name: "Existing",
    summary: null,
    ordered: false,
    condition: null,
    automatic: false,
    seriesCount: 0,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  } as never;
}

const TAG_RULE: SeriesCondition = { tag: { operator: "is", value: "isekai" } };

describe("CollectionFormModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("creates a collection with the entered name and ordered flag", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onCreated = vi.fn();
    renderWithProviders(
      <CollectionFormModal opened onClose={onClose} onCreated={onCreated} />,
    );

    await user.type(screen.getByPlaceholderText("e.g. Batman"), "Batman");
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() =>
      expect(collectionsApi.create).toHaveBeenCalledWith({
        name: "Batman",
        ordered: true,
      }),
    );
    await waitFor(() => expect(onCreated).toHaveBeenCalled());
    expect(onClose).toHaveBeenCalled();
  });
});

describe("CollectionFormModal membership mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("defaults to hand-picked for a new collection", () => {
    renderWithProviders(<CollectionFormModal opened onClose={vi.fn()} />);
    expect(screen.getByText(/you choose which series belong/i)).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /add filter/i }),
    ).not.toBeInTheDocument();
  });

  it("reveals the filter builder when switching to automatic", async () => {
    const user = userEvent.setup();
    renderWithProviders(<CollectionFormModal opened onClose={vi.fn()} />);

    await user.click(screen.getByText("Automatic"));

    expect(
      screen.getByText(/series matching the rule below belong automatically/i),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: /add filter/i }),
    ).toBeInTheDocument();
  });

  it("hides the manual-order checkbox in automatic mode", async () => {
    const user = userEvent.setup();
    renderWithProviders(<CollectionFormModal opened onClose={vi.fn()} />);

    expect(screen.getByRole("checkbox")).toBeInTheDocument();
    await user.click(screen.getByText("Automatic"));
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  // The API rejects an empty rule, because an empty allOf would match the whole
  // library. The form must not let one be submitted.
  it("blocks submission of an automatic collection with no conditions", async () => {
    const user = userEvent.setup();
    renderWithProviders(<CollectionFormModal opened onClose={vi.fn()} />);

    await user.type(screen.getByPlaceholderText("e.g. Batman"), "Empty rule");
    await user.click(screen.getByText("Automatic"));

    expect(screen.getByRole("button", { name: /create/i })).toBeDisabled();
    expect(screen.getByText(/add at least one filter/i)).toBeVisible();
    expect(collectionsApi.create).not.toHaveBeenCalled();
  });

  it("opens in automatic mode when editing a rule-backed collection", () => {
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        collection={existingCollection({
          automatic: true,
          condition: TAG_RULE,
          seriesCount: null,
        })}
      />,
    );
    expect(
      screen.getByText(/series matching the rule below belong automatically/i),
    ).toBeVisible();
  });

  it("seeds the builder from an initial condition, for create-from-preset", () => {
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        initialCondition={TAG_RULE}
      />,
    );
    expect(
      screen.getByText(/series matching the rule below belong automatically/i),
    ).toBeVisible();
    // The seeded leaf is editable, so its field picker shows the field.
    expect(screen.getByDisplayValue("Tag")).toBeInTheDocument();
  });

  it("submits a seeded rule as the condition", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        initialCondition={TAG_RULE}
      />,
    );

    await user.type(screen.getByPlaceholderText("e.g. Batman"), "Isekai");
    await user.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() => expect(collectionsApi.create).toHaveBeenCalled());
    const body = vi.mocked(collectionsApi.create).mock.calls[0][0];
    expect(body.condition).toEqual(TAG_RULE);
    expect(body.ordered).toBe(false);
  });

  it("warns when a rule reads the viewer's own data", () => {
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        initialCondition={{ userRating: { operator: "gte", value: 85 } }}
      />,
    );
    expect(
      screen.getByText(/each person will see a different set of series/i),
    ).toBeVisible();
  });

  it("does not warn for a rule over library metadata", () => {
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        initialCondition={TAG_RULE}
      />,
    );
    expect(
      screen.queryByText(/each person will see a different set of series/i),
    ).not.toBeInTheDocument();
  });

  // Switching back to hand-picked must send an explicit null, which is what the
  // API reads as "clear the rule and convert to manual".
  it("clears the rule when switching an automatic collection to hand-picked", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        collection={existingCollection({
          automatic: true,
          condition: TAG_RULE,
          seriesCount: null,
        })}
      />,
    );

    await user.click(screen.getByText("Hand-picked"));
    await user.click(screen.getByRole("button", { name: /save/i }));

    await waitFor(() => expect(collectionsApi.update).toHaveBeenCalled());
    const [, body] = vi.mocked(collectionsApi.update).mock.calls[0];
    expect(body.condition).toBeNull();
  });

  it("keeps the rule when saving an automatic collection unchanged", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <CollectionFormModal
        opened
        onClose={vi.fn()}
        collection={existingCollection({
          automatic: true,
          condition: TAG_RULE,
          seriesCount: null,
        })}
      />,
    );

    await user.click(screen.getByRole("button", { name: /save/i }));

    await waitFor(() => expect(collectionsApi.update).toHaveBeenCalled());
    const [, body] = vi.mocked(collectionsApi.update).mock.calls[0];
    expect(body.condition).toEqual(TAG_RULE);
    expect(body.ordered).toBe(false);
  });
});
