import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { LockableSelect } from "./LockableSelect";

describe("LockableSelect", () => {
  const options = [
    { value: "option1", label: "Option 1" },
    { value: "option2", label: "Option 2" },
    { value: "option3", label: "Option 3" },
  ];

  it("renders with value", () => {
    renderWithProviders(
      <LockableSelect
        value="option1"
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
        data={options}
      />,
    );

    expect(screen.getByRole("textbox")).toHaveValue("Option 1");
  });

  it("shows unlocked icon when not locked", () => {
    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
        data={options}
      />,
    );

    expect(screen.getByLabelText("Lock field")).toBeInTheDocument();
  });

  it("shows locked icon when locked", () => {
    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={vi.fn()}
        locked={true}
        onLockChange={vi.fn()}
        data={options}
      />,
    );

    expect(screen.getByLabelText("Unlock field")).toBeInTheDocument();
  });

  it("calls onChange when value changes", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={onChange}
        locked={false}
        onLockChange={vi.fn()}
        data={options}
      />,
    );

    const combobox = screen.getByRole("textbox");
    await user.click(combobox);
    await user.click(screen.getByText("Option 2"));

    expect(onChange).toHaveBeenCalledWith("option2");
  });

  it("toggles lock when lock icon clicked", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        data={options}
      />,
    );

    const lockButton = screen.getByLabelText("Lock field");
    await user.click(lockButton);

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("auto-locks when value differs from original", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue={null}
        autoLock={true}
        data={options}
      />,
    );

    const combobox = screen.getByRole("textbox");
    await user.click(combobox);
    await user.click(screen.getByText("Option 1"));

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("does not auto-lock when autoLock is false", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableSelect
        value={null}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue={null}
        autoLock={false}
        data={options}
      />,
    );

    const combobox = screen.getByRole("textbox");
    await user.click(combobox);
    await user.click(screen.getByText("Option 1"));

    expect(onLockChange).not.toHaveBeenCalled();
  });
});
