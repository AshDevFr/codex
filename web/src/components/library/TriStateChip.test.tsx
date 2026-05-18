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

  it("should mark the badge with data-state for include", () => {
    renderWithProviders(
      <TriStateChip label="Action" state="include" onChange={vi.fn()} />,
    );

    const badge = screen.getByText("Action").closest("[data-state]");
    expect(badge).toHaveAttribute("data-state", "include");
  });

  it("should mark the badge with data-state for exclude", () => {
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

  describe("variants (Phase 9)", () => {
    it("defaults to the metadata variant", () => {
      renderWithProviders(
        <TriStateChip label="Action" state="neutral" onChange={vi.fn()} />,
      );

      const badge = screen.getByText("Action").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-variant", "metadata");
    });

    it("renders the status variant with a leading category dot", () => {
      const { container } = renderWithProviders(
        <TriStateChip
          label="Ongoing"
          state="neutral"
          onChange={vi.fn()}
          variant="status"
          decorationKey="ongoing"
        />,
      );

      const badge = screen.getByText("Ongoing").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-variant", "status");
      // The leading slot is rendered; the dot lives inside it.
      const dot = container.querySelector('[class*="statusDot"]');
      expect(dot).not.toBeNull();
    });

    it("renders the progress variant with the eye icon for unread", () => {
      renderWithProviders(
        <TriStateChip
          label="Unread"
          state="neutral"
          onChange={vi.fn()}
          variant="progress"
          decorationKey="unread"
        />,
      );

      const badge = screen.getByText("Unread").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-variant", "progress");
      // The progress variant always shows its leading slot.
      expect(badge).toHaveAttribute("data-has-leading", "true");
    });

    it("renders the neutral variant without a leading slot at rest", () => {
      renderWithProviders(
        <TriStateChip
          label="Has Rating"
          state="neutral"
          onChange={vi.fn()}
          variant="neutral"
        />,
      );

      const badge = screen.getByText("Has Rating").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-variant", "neutral");
      // No leading slot at rest because variant has no decoration.
      expect(badge).not.toHaveAttribute("data-has-leading");
    });

    it("substitutes the leading dot for a checkmark when status is included", () => {
      const { container } = renderWithProviders(
        <TriStateChip
          label="Ongoing"
          state="include"
          onChange={vi.fn()}
          variant="status"
          decorationKey="ongoing"
        />,
      );

      // The status dot is hidden once the chip is selected; the leading
      // slot shows the check icon instead.
      const dot = container.querySelector('[class*="statusDot"]');
      expect(dot).toBeNull();
      const badge = screen.getByText("Ongoing").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-state", "include");
    });

    it("adds a leading slot on the neutral variant when selected", () => {
      const { rerender } = renderWithProviders(
        <TriStateChip
          label="Has Rating"
          state="neutral"
          onChange={vi.fn()}
          variant="neutral"
        />,
      );

      let badge = screen.getByText("Has Rating").closest("[data-variant]");
      expect(badge).not.toHaveAttribute("data-has-leading");

      rerender(
        <TriStateChip
          label="Has Rating"
          state="include"
          onChange={vi.fn()}
          variant="neutral"
        />,
      );

      badge = screen.getByText("Has Rating").closest("[data-variant]");
      expect(badge).toHaveAttribute("data-has-leading", "true");
    });
  });

  describe("count rendering (Phase 9)", () => {
    it("renders the count as a plain span, not a nested Mantine Badge", () => {
      renderWithProviders(
        <TriStateChip
          label="Action"
          state="neutral"
          onChange={vi.fn()}
          count={15}
        />,
      );

      const count = screen.getByTestId("tri-state-chip-count");
      expect(count.tagName).toBe("SPAN");
      // The count must not be wrapped in a Mantine Badge anymore: that
      // gave us a boxy decoration where the plan calls for a soft inline
      // numeral.
      expect(count.closest(".mantine-Badge-root")).toBeNull();
    });

    it("uses tabular numerals on the badge so counts align vertically", () => {
      renderWithProviders(
        <TriStateChip
          label="Action"
          state="neutral"
          onChange={vi.fn()}
          count={12}
        />,
      );

      const badge = screen.getByText("Action").closest("[data-state]");
      expect(badge).not.toBeNull();
      // jsdom doesn't evaluate CSS module class hashes; assert the
      // badge picks up the styled className so the
      // font-variant-numeric: tabular-nums rule from the module
      // applies. The visual outcome is covered separately by the
      // Playwright snapshot pass.
      expect(badge?.className).toMatch(/badge/i);
    });
  });
});
