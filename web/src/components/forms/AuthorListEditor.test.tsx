import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { BookAuthor } from "@/types/book-metadata";
import { AuthorListEditor } from "./AuthorListEditor";

function ControlledEditor({ initial }: { initial: BookAuthor[] }) {
  const [authors, setAuthors] = useState(initial);
  return <AuthorListEditor authors={authors} onChange={setAuthors} />;
}

const nameInputs = () =>
  screen.getAllByPlaceholderText("Author name") as HTMLInputElement[];

describe("AuthorListEditor", () => {
  it("renders one row per author", () => {
    renderWithProviders(
      <AuthorListEditor
        authors={[
          { name: "Alice", role: "writer" },
          { name: "Bob", role: "penciller" },
        ]}
        onChange={vi.fn()}
      />,
    );

    expect(nameInputs().map((input) => input.value)).toEqual(["Alice", "Bob"]);
  });

  it("adds an empty author row", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    renderWithProviders(
      <AuthorListEditor
        authors={[{ name: "Alice", role: "writer" }]}
        onChange={onChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Add Author" }));

    expect(onChange).toHaveBeenCalledWith([
      { name: "Alice", role: "writer" },
      { name: "", role: "author" },
    ]);
  });

  it("reports name edits for the edited row only", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <ControlledEditor
        initial={[
          { name: "Alice", role: "writer" },
          { name: "Bob", role: "writer" },
        ]}
      />,
    );

    await user.type(nameInputs()[1], "by");

    expect(nameInputs().map((input) => input.value)).toEqual([
      "Alice",
      "Bobby",
    ]);
  });

  it("keeps the remaining rows' elements when a middle row is removed", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <ControlledEditor
        initial={[
          { name: "Alice", role: "writer" },
          { name: "Bob", role: "writer" },
          { name: "Carol", role: "writer" },
        ]}
      />,
    );
    const [alice, , carol] = nameInputs();

    await user.click(
      screen.getAllByRole("button", { name: "Remove author" })[1],
    );

    const remaining = nameInputs();
    expect(remaining.map((input) => input.value)).toEqual(["Alice", "Carol"]);
    // With index-based keys, Carol's row would take over Bob's element.
    expect(remaining[0]).toBe(alice);
    expect(remaining[1]).toBe(carol);
  });

  it("does not remount a row when its role changes", () => {
    const { rerender } = renderWithProviders(
      <AuthorListEditor
        authors={[{ name: "Alice", role: "writer" }]}
        onChange={vi.fn()}
      />,
    );
    const before = nameInputs()[0];

    rerender(
      <AuthorListEditor
        authors={[{ name: "Alice", role: "penciller" }]}
        onChange={vi.fn()}
      />,
    );

    expect(nameInputs()[0]).toBe(before);
  });
});
