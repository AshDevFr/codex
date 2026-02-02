import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { LockableChipInput } from "./LockableChipInput";

describe("LockableChipInput", () => {
  it("renders with values", () => {
    renderWithProviders(
      <LockableChipInput
        value={["tag1", "tag2"]}
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByText("tag1")).toBeInTheDocument();
    expect(screen.getByText("tag2")).toBeInTheDocument();
  });

  it("shows unlocked icon when not locked", () => {
    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={vi.fn()}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Lock field")).toBeInTheDocument();
  });

  it("shows locked icon when locked", () => {
    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={vi.fn()}
        locked={true}
        onLockChange={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Unlock field")).toBeInTheDocument();
  });

  it("toggles lock when lock icon clicked", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
      />,
    );

    const lockButton = screen.getByLabelText("Lock field");
    await user.click(lockButton);

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("calls onChange when adding a tag", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={onChange}
        locked={false}
        onLockChange={vi.fn()}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "newtag{enter}");

    expect(onChange).toHaveBeenCalledWith(["newtag"]);
  });

  it("auto-locks when value differs from original", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue={[]}
        autoLock={true}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "newtag{enter}");

    expect(onLockChange).toHaveBeenCalledWith(true);
  });

  it("does not auto-lock when autoLock is false", async () => {
    const onLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <LockableChipInput
        value={[]}
        onChange={vi.fn()}
        locked={false}
        onLockChange={onLockChange}
        originalValue={[]}
        autoLock={false}
      />,
    );

    const input = screen.getByRole("textbox");
    await user.type(input, "newtag{enter}");

    expect(onLockChange).not.toHaveBeenCalled();
  });
});
