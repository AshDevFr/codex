import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { TriStateChip } from "./TriStateChip";

describe("TriStateChip", () => {
  it("should render with label", () => {
    renderWithProviders(
      <TriStateChip label="Action" state="neutral" onChange={vi.fn()} />,
    );

    expect(screen.getByText("Action")).toBeInTheDocument();
  });

  it("should render with count when provided", () => {
    renderWithProviders(
      <TriStateChip
        label="Action"
        state="neutral"
        onChange={vi.fn()}
        count={42}
      />,
    );

    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  it("should cycle from neutral to include on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <TriStateChip label="Action" state="neutral" onChange={onChange} />,
    );

    await user.click(screen.getByText("Action"));

    expect(onChange).toHaveBeenCalledWith("include");
  });

  it("should cycle from include to exclude on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <TriStateChip label="Action" state="include" onChange={onChange} />,
    );

    await user.click(screen.getByText("Action"));

    expect(onChange).toHaveBeenCalledWith("exclude");
  });

  it("should cycle from exclude to neutral on click", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <TriStateChip label="Action" state="exclude" onChange={onChange} />,
    );

    await user.click(screen.getByText("Action"));

    expect(onChange).toHaveBeenCalledWith("neutral");
  });

  it("should not call onChange when disabled", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <TriStateChip
        label="Action"
        state="neutral"
        onChange={onChange}
        disabled
      />,
    );

    await user.click(screen.getByText("Action"));

    expect(onChange).not.toHaveBeenCalled();
  });

  it("should display checkmark icon for include state", () => {
    renderWithProviders(
      <TriStateChip label="Action" state="include" onChange={vi.fn()} />,
    );

    // The IconCheck component should be rendered
    const badge = screen.getByText("Action").closest("[data-state]");
    expect(badge).toHaveAttribute("data-state", "include");
  });

  it("should display X icon for exclude state", () => {
    renderWithProviders(
      <TriStateChip label="Action" state="exclude" onChange={vi.fn()} />,
    );

    const badge = screen.getByText("Action").closest("[data-state]");
    expect(badge).toHaveAttribute("data-state", "exclude");
  });

  it("should have neutral styling for neutral state", () => {
    renderWithProviders(
      <TriStateChip label="Action" state="neutral" onChange={vi.fn()} />,
    );

    const badge = screen.getByText("Action").closest("[data-state]");
    expect(badge).toHaveAttribute("data-state", "neutral");
  });
});
