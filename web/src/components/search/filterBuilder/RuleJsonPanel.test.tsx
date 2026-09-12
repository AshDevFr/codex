import { beforeEach, describe, expect, it, vi } from "vitest";
import { librariesApi } from "@/api/libraries";
import {
  fireEvent,
  renderWithProviders,
  screen,
  userEvent,
  waitFor,
} from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import { RuleJsonPanel } from "./RuleJsonPanel";

vi.mock("@/api/libraries", () => ({
  librariesApi: { getAll: vi.fn() },
}));

const MANGA_ID = "018f4b2c-0000-4000-8000-000000000001";
const GHOST_ID = "018f4b2c-0000-4000-8000-0000000000ff";

const RULE: SeriesCondition = {
  allOf: [
    { year: { operator: "gte", value: 2000 } },
    { genre: { operator: "is", value: "Action" } },
  ],
};

/** The panel's textarea, addressed the way a screen reader would. */
const editor = () => screen.getByLabelText(/rule json/i) as HTMLTextAreaElement;

function setText(value: string) {
  fireEvent.change(editor(), { target: { value } });
}

describe("RuleJsonPanel", () => {
  beforeEach(() => {
    vi.mocked(librariesApi.getAll).mockResolvedValue([
      { id: MANGA_ID, name: "Manga" },
    ] as Awaited<ReturnType<typeof librariesApi.getAll>>);
  });

  it("shows the rule the builder would save", () => {
    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={vi.fn()} />,
    );
    expect(JSON.parse(editor().value)).toEqual(RULE);
  });

  it("leaves the editor empty when no complete filter exists yet", () => {
    renderWithProviders(
      <RuleJsonPanel
        condition={{ allOf: [{ genre: { operator: "is", value: "" } }] }}
        target="series"
        onChange={vi.fn()}
      />,
    );
    expect(editor().value).toBe("");
  });

  it("copies the rule to the clipboard", async () => {
    const user = userEvent.setup();
    // userEvent installs a read-only clipboard stub on setup, so spy on the one
    // it left rather than replacing the property.
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);

    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={vi.fn()} />,
    );
    await user.click(screen.getByRole("button", { name: /copy/i }));

    expect(writeText).toHaveBeenCalledTimes(1);
    expect(JSON.parse(writeText.mock.calls[0][0])).toEqual(RULE);
  });

  it("applies edited JSON to the builder, wrapped in a root group", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={onChange} />,
    );

    setText(JSON.stringify({ year: { operator: "lt", value: 1990 } }));
    await user.click(screen.getByRole("button", { name: /apply/i }));

    expect(onChange).toHaveBeenCalledWith({
      allOf: [{ year: { operator: "lt", value: 1990 } }],
    });
  });

  it("reports what is wrong and leaves the builder alone", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={onChange} />,
    );

    setText(JSON.stringify({ year: { operator: "contains", value: "20" } }));
    await user.click(screen.getByRole("button", { name: /apply/i }));

    expect(onChange).not.toHaveBeenCalled();
    expect(
      await screen.findByText(/does not support operator "contains"/i),
    ).toBeInTheDocument();
  });

  it("reports unparseable text as invalid JSON", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={onChange} />,
    );

    setText("{ not json");
    await user.click(screen.getByRole("button", { name: /apply/i }));

    expect(onChange).not.toHaveBeenCalled();
    expect(await screen.findByText(/invalid json/i)).toBeInTheDocument();
  });

  it("stops mirroring the builder once edited, and resumes on revert", async () => {
    const user = userEvent.setup();
    const { rerender } = renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={vi.fn()} />,
    );

    setText('{ "title": { "operator": "is", "value": "draft" } }');
    // A builder row changing underneath must not wipe what is being typed.
    rerender(
      <RuleJsonPanel
        condition={{ allOf: [{ year: { operator: "gte", value: 2010 } }] }}
        target="series"
        onChange={vi.fn()}
      />,
    );
    expect(editor().value).toContain("draft");

    await user.click(screen.getByRole("button", { name: /revert/i }));
    expect(editor().value).not.toContain("draft");
    expect(JSON.parse(editor().value)).toEqual({
      year: { operator: "gte", value: 2010 },
    });
  });

  it("clears a stale error once the rule applies", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <RuleJsonPanel condition={RULE} target="series" onChange={vi.fn()} />,
    );

    setText("{ not json");
    await user.click(screen.getByRole("button", { name: /apply/i }));
    expect(await screen.findByText(/invalid json/i)).toBeInTheDocument();

    setText(JSON.stringify({ year: { operator: "lt", value: 1990 } }));
    await user.click(screen.getByRole("button", { name: /apply/i }));
    await waitFor(() =>
      expect(screen.queryByText(/invalid json/i)).not.toBeInTheDocument(),
    );
  });

  it("warns when the rule names a library this server does not have", async () => {
    renderWithProviders(
      <RuleJsonPanel
        condition={{ libraryId: { operator: "is", value: GHOST_ID } }}
        target="series"
        onChange={vi.fn()}
      />,
    );
    expect(await screen.findByText(new RegExp(GHOST_ID))).toBeInTheDocument();
  });

  it("stays quiet when every library resolves", async () => {
    renderWithProviders(
      <RuleJsonPanel
        condition={{ libraryId: { operator: "is", value: MANGA_ID } }}
        target="series"
        onChange={vi.fn()}
      />,
    );
    await waitFor(() => expect(librariesApi.getAll).toHaveBeenCalled());
    expect(screen.queryByText(/not on this server/i)).not.toBeInTheDocument();
  });
});
