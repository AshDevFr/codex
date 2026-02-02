import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { LockableInput } from "./LockableInput";

describe("LockableInput", () => {
  it("renders with value", () => {
    renderWithProviders(
      <LockableInput
        value="Test value"
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByDisplayValue("Test value")).toBeInTheDocument();
  });

  it("shows unlocked icon when not locked", () => {
    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Lock field")).toBeInTheDocument();
  });

  it("shows locked icon when locked", () => {
    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={true}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Unlock field")).toBeInTheDocument();
  });

  it("calls onChange when value changes", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={onChange}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "a");

    expect(onChange).toHaveBeenCalledWith("a");
  });

  it("toggles lock when lock icon clicked", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
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
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue=""
        autoLock={true}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "a");

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("does not auto-lock when autoLock is false", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue=""
        autoLock={false}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "a");

    expect(onLockChange).not.toHaveBeenCalled();
  });

  it("does not auto-lock when already locked", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={true}
        onLockChange={onLockChange}
        originalValue=""
        autoLock={true}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "a");

    expect(onLockChange).not.toHaveBeenCalled();
  });

  it("shows lock tooltip on hover", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    const lockButton = screen.getByLabelText("Lock field");
    await user.hover(lockButton);

    // Tooltip text should appear
    expect(
      await screen.findByText("Unlocked: Can be updated automatically"),
    ).toBeInTheDocument();
  });

  it("shows locked tooltip when locked", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <LockableInput
        value=""
        onChange={vi.fn()}
        locked={true}
        onLockChange={vi.fn()}
      />,
    );

    const lockButton = screen.getByLabelText("Unlock field");
    await user.hover(lockButton);

    expect(
      await screen.findByText("Locked: Protected from automatic updates"),
    ).toBeInTheDocument();
  });
});
