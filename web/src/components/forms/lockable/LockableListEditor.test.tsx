import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import {
  type FieldConfig,
  type ListItem,
  LockableListEditor,
} from "./LockableListEditor";

describe("LockableListEditor", () => {
  const fields: FieldConfig[] = [
    { key: "label", label: "Label", placeholder: "Enter label" },
    { key: "url", label: "URL", placeholder: "Enter URL" },
  ];

  const mockItems: ListItem[] = [
    {
      id: "1",
      values: { label: "Link 1", url: "https://example.com" },
      locked: false,
    },
    {
      id: "2",
      values: { label: "Link 2", url: "https://test.com" },
      locked: true,
    },
  ];

  it("renders list items", () => {
    renderWithProviders(
      <LockableListEditor
        items={mockItems}
        onChange={vi.fn()}
        fields={fields}
      />,
    );

    expect(screen.getByDisplayValue("Link 1")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Link 2")).toBeInTheDocument();
    expect(screen.getByDisplayValue("https://example.com")).toBeInTheDocument();
    expect(screen.getByDisplayValue("https://test.com")).toBeInTheDocument();
  });

  it("shows lock icons for each row", () => {
    renderWithProviders(
      <LockableListEditor
        items={mockItems}
        onChange={vi.fn()}
        fields={fields}
      />,
    );

    // One unlocked, one locked
    expect(screen.getByLabelText("Lock row")).toBeInTheDocument();
    expect(screen.getByLabelText("Unlock row")).toBeInTheDocument();
  });

  it("toggles lock when lock icon clicked", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableListEditor
        items={[mockItems[0]]}
        onChange={onChange}
        fields={fields}
      />,
    );

    const lockButton = screen.getByLabelText("Lock row");
    await user.click(lockButton);

    expect(onChange).toHaveBeenCalledWith([{ ...mockItems[0], locked: true }]);
  });

  it("updates field value when typing", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableListEditor
        items={[{ id: "1", values: { label: "", url: "" }, locked: false }]}
        onChange={onChange}
        fields={fields}
      />,
    );

    const inputs = screen.getAllByRole("textbox");
    await user.type(inputs[0], "A");

    expect(onChange).toHaveBeenCalled();
    const calledWith = onChange.mock.calls[0][0];
    expect(calledWith[0].values.label).toBe("A");
  });

  it("deletes item when delete button clicked", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableListEditor
        items={mockItems}
        onChange={onChange}
        fields={fields}
      />,
    );

    const deleteButtons = screen.getAllByLabelText("Delete row");
    await user.click(deleteButtons[0]);

    expect(onChange).toHaveBeenCalledWith([mockItems[1]]);
  });

  it("adds new item when add button clicked", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    const generateId = vi.fn(() => "new-id");

    renderWithProviders(
      <LockableListEditor
        items={[]}
        onChange={onChange}
        fields={fields}
        generateId={generateId}
        addButtonLabel="Add Link"
      />,
    );

    const addButton = screen.getByText("Add Link");
    await user.click(addButton);

    expect(onChange).toHaveBeenCalledWith([
      { id: "new-id", values: { label: "", url: "" }, locked: false },
    ]);
  });

  it("auto-locks when field value changes from original", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    const items: ListItem[] = [
      { id: "1", values: { label: "Original", url: "" }, locked: false },
    ];

    renderWithProviders(
      <LockableListEditor
        items={items}
        onChange={onChange}
        fields={fields}
        originalItems={items}
        autoLock={true}
      />,
    );

    const inputs = screen.getAllByRole("textbox");
    await user.type(inputs[0], "X");

    expect(onChange).toHaveBeenCalled();
    const calledWith = onChange.mock.calls[0][0];
    expect(calledWith[0].locked).toBe(true);
  });

  it("does not auto-lock when autoLock is false", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    const items: ListItem[] = [
      { id: "1", values: { label: "Original", url: "" }, locked: false },
    ];

    renderWithProviders(
      <LockableListEditor
        items={items}
        onChange={onChange}
        fields={fields}
        originalItems={items}
        autoLock={false}
      />,
    );

    const inputs = screen.getAllByRole("textbox");
    await user.type(inputs[0], "X");

    expect(onChange).toHaveBeenCalled();
    const calledWith = onChange.mock.calls[0][0];
    expect(calledWith[0].locked).toBe(false);
  });
});
