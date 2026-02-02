import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { LockableTextarea } from "./LockableTextarea";

describe("LockableTextarea", () => {
  it("renders with value", () => {
    renderWithProviders(
      <LockableTextarea
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
      <LockableTextarea
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
      <LockableTextarea
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
      <LockableTextarea
        value=""
        onChange={onChange}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "a");

    expect(onChange).toHaveBeenCalledWith("a");
  });

  it("toggles lock when lock icon clicked", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableTextarea
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
      <LockableTextarea
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue=""
        autoLock={true}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "a");

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("does not auto-lock when autoLock is false", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableTextarea
        value=""
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue=""
        autoLock={false}
      />,
    );

    const textarea = screen.getByRole("textbox");
    await user.type(textarea, "a");

    expect(onLockChange).not.toHaveBeenCalled();
  });
});
