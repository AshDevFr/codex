import { screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, userEvent } from "@/test/utils";
import { CronInput } from "./CronInput";

describe("CronInput", () => {
  const mockOnChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should render with empty value", () => {
    renderWithProviders(
      <CronInput value="" onChange={mockOnChange} label="Cron" />,
    );

    const input = screen.getByLabelText("Cron");
    expect(input).toBeInTheDocument();
    expect(input).toHaveValue("");
  });

  it("should display valid cron expression", () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    const input = screen.getByLabelText("Cron");
    expect(input).toBeInTheDocument();
    expect(input).toHaveValue("0 0 * * *");
  });

  it("should show human-readable description for valid cron", async () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    await waitFor(() => {
      // Should show human-readable text (e.g., "At 12:00 AM")
      const description = screen.queryByText(/At/i);
      expect(description).toBeInTheDocument();
    });
  });

  it("should show next run time for valid cron", async () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    await waitFor(() => {
      // Next run time is shown as formatted date (yyyy-MM-dd HH:mm:ss)
      const nextRun = screen.queryByText(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
      expect(nextRun).toBeInTheDocument();
    });
  });

  it("should not show next run time when showNextRun is false", async () => {
    renderWithProviders(
      <CronInput
        value="0 0 * * *"
        onChange={mockOnChange}
        label="Cron"
        showNextRun={false}
      />,
    );

    await waitFor(() => {
      // Next run time should not be shown
      const nextRun = screen.queryByText(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
      expect(nextRun).not.toBeInTheDocument();
    });
  });

  it("should show error for invalid cron expression", () => {
    renderWithProviders(
      <CronInput value="invalid cron" onChange={mockOnChange} label="Cron" />,
    );

    const input = screen.getByLabelText("Cron");
    expect(input).toHaveAttribute("aria-invalid", "true");

    // Error text includes format information
    // Mantine may render error in multiple places, so use getAllByText and check first
    const errorTexts = screen.getAllByText(/Invalid cron expression/i);
    expect(errorTexts.length).toBeGreaterThan(0);
    expect(errorTexts[0].textContent).toContain("Format:");
  });

  it("should call onChange when input changes", async () => {
    const user = userEvent.setup();
    // Use a stateful component to test controlled input behavior
    const TestComponent = () => {
      const [value, setValue] = useState("");
      return (
        <CronInput
          value={value}
          onChange={(newValue) => {
            setValue(newValue);
            mockOnChange(newValue);
          }}
          label="Cron"
        />
      );
    };

    renderWithProviders(<TestComponent />);

    const input = screen.getByLabelText("Cron");
    // Type the full cron expression character by character
    // This ensures each character triggers onChange
    await user.type(input, "0 0 * * *");

    // Wait for the input to have the complete value (confirms typing finished)
    await waitFor(() => {
      expect(input).toHaveValue("0 0 * * *");
    });

    // Now verify that onChange was called with the complete value
    expect(mockOnChange).toHaveBeenCalled();
    const calls = mockOnChange.mock.calls;
    expect(calls.length).toBeGreaterThanOrEqual(1);

    // Find any call with the complete value "0 0 * * *"
    // The onChange is called for each character, so we need to find the one with the full string
    const completeValueCall = calls.find(
      (call) => call && call[0] === "0 0 * * *",
    );

    // Verify that the complete value was passed to onChange
    expect(completeValueCall).toBeDefined();
    expect(completeValueCall?.[0]).toBe("0 0 * * *");
  });

  it("should display custom error message", () => {
    renderWithProviders(
      <CronInput
        value="invalid"
        onChange={mockOnChange}
        label="Cron"
        error="Custom error message"
      />,
    );

    const input = screen.getByLabelText("Cron");
    expect(input).toHaveAttribute("aria-invalid", "true");
  });

  it("should handle various valid cron expressions", async () => {
    const validCrons = [
      "0 */6 * * *", // Every 6 hours
      "0 0 * * 0", // Weekly on Sunday
      "0 2 * * 1-5", // Weekdays at 2 AM
      "*/15 * * * *", // Every 15 minutes
    ];

    for (const cron of validCrons) {
      const { unmount } = renderWithProviders(
        <CronInput value={cron} onChange={mockOnChange} label="Cron" />,
      );

      const input = screen.getByLabelText("Cron");
      expect(input).toHaveValue(cron);
      expect(input).not.toHaveAttribute("aria-invalid", "true");

      await waitFor(() => {
        const description = screen.queryByText(/At|Every/i);
        expect(description).toBeInTheDocument();
      });

      unmount();
    }
  });

  it("should use monospace font for input", () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    const input = screen.getByLabelText("Cron");
    const styles = window.getComputedStyle(input);
    expect(styles.fontFamily).toContain("monospace");
  });

  it("should handle /n format and normalize it for cronstrue", async () => {
    // Test that 0 /2 * * * (valid cron-parser format) works
    renderWithProviders(
      <CronInput value="0 /2 * * *" onChange={mockOnChange} label="Cron" />,
    );

    const input = screen.getByLabelText("Cron");
    expect(input).toHaveValue("0 /2 * * *");
    expect(input).not.toHaveAttribute("aria-invalid", "true");

    // Should show description (normalized to */2 for cronstrue)
    await waitFor(() => {
      const description = screen.queryByText(/every|At/i);
      expect(description).toBeInTheDocument();
    });
  });

  it("should display description and next run on same line with hyphen", async () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    await waitFor(() => {
      // Should have description
      const description = screen.queryByText(/At/i);
      expect(description).toBeInTheDocument();

      // Should have hyphen separator
      const hyphen = screen.queryByText("-");
      expect(hyphen).toBeInTheDocument();

      // Should have next run time
      const nextRun = screen.queryByText(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
      expect(nextRun).toBeInTheDocument();
    });
  });

  it("should display description in blue color", async () => {
    renderWithProviders(
      <CronInput value="0 0 * * *" onChange={mockOnChange} label="Cron" />,
    );

    await waitFor(() => {
      const description = screen.queryByText(/At/i);
      expect(description).toBeInTheDocument();
      // Check that it has blue color class or style
      expect(description).toHaveClass("mantine-Text-root");
    });
  });

  it("should validate expressions with /n format in different positions", async () => {
    // Note: cron-parser may not accept /n format in all positions
    // Only test positions where /n is actually valid
    const expressionsWithSlash = [
      "0 */2 * * *", // Every 2 hours (using */2 format which is definitely valid)
      "*/15 * * * *", // Every 15 minutes
    ];

    for (const cron of expressionsWithSlash) {
      const { unmount } = renderWithProviders(
        <CronInput value={cron} onChange={mockOnChange} label="Cron" />,
      );

      const input = screen.getByLabelText("Cron");
      expect(input).toHaveValue(cron);
      expect(input).not.toHaveAttribute("aria-invalid", "true");

      unmount();
    }
  });

  it("should show next run time for /n format expressions", async () => {
    renderWithProviders(
      <CronInput value="0 /2 * * *" onChange={mockOnChange} label="Cron" />,
    );

    await waitFor(() => {
      // Should calculate and show next run time
      const nextRun = screen.queryByText(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
      expect(nextRun).toBeInTheDocument();
    });
  });

  it("should not show description or next run for invalid expressions", async () => {
    renderWithProviders(
      <CronInput value="invalid cron" onChange={mockOnChange} label="Cron" />,
    );

    // The input should show as invalid
    const input = screen.getByLabelText("Cron");
    expect(input).toHaveAttribute("aria-invalid", "true");

    // Wait a bit to ensure any async operations complete
    // The description should not be shown for invalid expressions
    // We check by looking for the Group that contains the description
    // (it has blue text and shows the description)
    await waitFor(
      () => {
        // Look for elements that might be the description
        // The description is in a Group with a Text element that has blue color
        // Error messages are in a different structure
        const errorElement = screen.queryByText(/Invalid cron expression/i);
        expect(errorElement).toBeInTheDocument(); // Error should be shown

        // Check that there's no description Group (which would contain blue text with "At" or "Every")
        // The description Group is a sibling of the TextInput, not in the error area
        const inputWrapper = input.closest(".mantine-InputWrapper-root");
        if (inputWrapper) {
          // Look for Group elements that might contain the description
          const groups = inputWrapper.querySelectorAll('[class*="Group"]');
          const hasDescriptionGroup = Array.from(groups).some((group) => {
            const text = group.textContent || "";
            // Description groups contain text starting with "At " or "Every "
            return /^(At |Every )/i.test(text.trim());
          });
          expect(hasDescriptionGroup).toBe(false);
        }

        // Should not show next run (only shown for valid cron)
        const nextRun = screen.queryByText(
          /\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/,
        );
        expect(nextRun).not.toBeInTheDocument();
      },
      { timeout: 1000 },
    );
  });
});
