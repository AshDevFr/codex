import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/utils";
import {
  BOOK_STRATEGIES,
  BookStrategySelector,
  NUMBER_STRATEGIES,
  NumberStrategySelector,
  SERIES_STRATEGIES,
  SeriesStrategySelector,
} from "./StrategySelector";

describe("SeriesStrategySelector", () => {
  it("renders with default series_volume strategy selected", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="series_volume"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.getByText("Series Detection Strategy")).toBeInTheDocument();
    // Mantine Select displays the label in the textbox, not the value
    expect(screen.getByRole("textbox", { hidden: true })).toHaveValue(
      "Series-Volume (Recommended)",
    );
  });

  it("displays description for selected strategy", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="series_volume"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    const seriesVolumeStrategy = SERIES_STRATEGIES.find(
      (s) => s.value === "series_volume",
    );
    expect(seriesVolumeStrategy).toBeDefined();
    expect(
      screen.getByText(seriesVolumeStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });

  it("calls onChange when strategy is changed", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="series_volume"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    // Open the select dropdown
    const select = screen.getByRole("textbox", { hidden: true });
    await user.click(select.parentElement!);

    // Select a different strategy
    await waitFor(() => {
      expect(screen.getByText("Series-Volume-Chapter")).toBeInTheDocument();
    });
    await user.click(screen.getByText("Series-Volume-Chapter"));

    expect(onChange).toHaveBeenCalledWith("series_volume_chapter");
    expect(onConfigChange).toHaveBeenCalledWith({}); // Config reset on strategy change
  });

  it("shows config editor for flat strategy", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="flat"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.getByText("Filename Patterns")).toBeInTheDocument();
  });

  it("shows config editor for publisher_hierarchy strategy", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="publisher_hierarchy"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.getByText("Skip Depth")).toBeInTheDocument();
    expect(screen.getByText("Store Skipped As")).toBeInTheDocument();
  });

  it("shows config editor for calibre strategy", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="calibre"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.getByText("Series Grouping Mode")).toBeInTheDocument();
  });

  it("shows config editor for custom strategy", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="custom"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.getByText("Pattern")).toBeInTheDocument();
    expect(screen.getByText("Series Name Template")).toBeInTheDocument();
  });

  it("does not show config editor for series_volume (no config needed)", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="series_volume"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    expect(screen.queryByText("Filename Patterns")).not.toBeInTheDocument();
    expect(screen.queryByText("Skip Depth")).not.toBeInTheDocument();
    expect(screen.queryByText("Pattern")).not.toBeInTheDocument();
  });

  it("disables select when disabled prop is true", () => {
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="series_volume"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
        disabled
      />,
    );

    const select = screen.getByRole("textbox", { hidden: true });
    expect(select).toBeDisabled();
  });

  it("updates flat config when filename patterns change", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="flat"
        onChange={onChange}
        config={{}}
        onConfigChange={onConfigChange}
      />,
    );

    const patternsTextarea = screen.getByPlaceholderText(/\\\[/);
    await user.type(patternsTextarea, "test pattern");

    expect(onConfigChange).toHaveBeenCalled();
  });

  it("updates publisher_hierarchy config when skip depth changes", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const onConfigChange = vi.fn();

    renderWithProviders(
      <SeriesStrategySelector
        value="publisher_hierarchy"
        onChange={onChange}
        config={{ skipDepth: 1 }}
        onConfigChange={onConfigChange}
      />,
    );

    const skipDepthInput = screen.getByRole("textbox", {
      name: /skip depth/i,
    });
    await user.clear(skipDepthInput);
    await user.type(skipDepthInput, "2");

    expect(onConfigChange).toHaveBeenCalled();
  });
});

describe("BookStrategySelector", () => {
  it("renders with default filename strategy selected", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <BookStrategySelector value="filename" onChange={onChange} />,
    );

    expect(screen.getByText("Book Naming Strategy")).toBeInTheDocument();
    // Mantine Select displays the label in the textbox, not the value
    expect(screen.getByRole("textbox", { hidden: true })).toHaveValue(
      "Filename (Recommended)",
    );
  });

  it("displays description for selected strategy", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <BookStrategySelector value="filename" onChange={onChange} />,
    );

    const filenameStrategy = BOOK_STRATEGIES.find(
      (s) => s.value === "filename",
    );
    expect(filenameStrategy).toBeDefined();
    expect(
      screen.getByText(filenameStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });

  it("calls onChange when strategy is changed", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <BookStrategySelector value="filename" onChange={onChange} />,
    );

    // Open the select dropdown
    const select = screen.getByRole("textbox", { hidden: true });
    await user.click(select.parentElement!);

    // Select a different strategy
    await waitFor(() => {
      expect(screen.getByText("Metadata First")).toBeInTheDocument();
    });
    await user.click(screen.getByText("Metadata First"));

    expect(onChange).toHaveBeenCalledWith("metadata_first");
  });

  it("disables select when disabled prop is true", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <BookStrategySelector value="filename" onChange={onChange} disabled />,
    );

    const select = screen.getByRole("textbox", { hidden: true });
    expect(select).toBeDisabled();
  });

  it("shows all book strategy options", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <BookStrategySelector value="filename" onChange={onChange} />,
    );

    // Open the select dropdown
    const select = screen.getByRole("textbox", { hidden: true });
    await user.click(select.parentElement!);

    await waitFor(() => {
      expect(screen.getByText("Filename (Recommended)")).toBeInTheDocument();
      expect(screen.getByText("Metadata First")).toBeInTheDocument();
      expect(screen.getByText("Smart Detection")).toBeInTheDocument();
      expect(screen.getByText("Generated Name")).toBeInTheDocument();
    });
  });
});

describe("NumberStrategySelector", () => {
  it("renders with default file_order strategy selected", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="file_order" onChange={onChange} />,
    );

    expect(screen.getByText("Book Number Strategy")).toBeInTheDocument();
    // Mantine Select displays the label in the textbox, not the value
    expect(screen.getByRole("textbox", { hidden: true })).toHaveValue(
      "File Order (Recommended)",
    );
  });

  it("displays description for selected strategy", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="file_order" onChange={onChange} />,
    );

    const fileOrderStrategy = NUMBER_STRATEGIES.find(
      (s) => s.value === "file_order",
    );
    expect(fileOrderStrategy).toBeDefined();
    expect(
      screen.getByText(fileOrderStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });

  it("calls onChange when strategy is changed", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="file_order" onChange={onChange} />,
    );

    // Open the select dropdown
    const select = screen.getByRole("textbox", { hidden: true });
    await user.click(select.parentElement!);

    // Select a different strategy
    await waitFor(() => {
      expect(screen.getByText("Metadata Only")).toBeInTheDocument();
    });
    await user.click(screen.getByText("Metadata Only"));

    expect(onChange).toHaveBeenCalledWith("metadata");
  });

  it("disables select when disabled prop is true", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector
        value="file_order"
        onChange={onChange}
        disabled
      />,
    );

    const select = screen.getByRole("textbox", { hidden: true });
    expect(select).toBeDisabled();
  });

  it("shows all number strategy options", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="file_order" onChange={onChange} />,
    );

    // Open the select dropdown
    const select = screen.getByRole("textbox", { hidden: true });
    await user.click(select.parentElement!);

    await waitFor(() => {
      expect(screen.getByText("File Order (Recommended)")).toBeInTheDocument();
      expect(screen.getByText("Metadata Only")).toBeInTheDocument();
      expect(screen.getByText("Filename Patterns")).toBeInTheDocument();
      expect(screen.getByText("Smart Detection")).toBeInTheDocument();
    });
  });

  it("displays correct description for metadata strategy", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="metadata" onChange={onChange} />,
    );

    const metadataStrategy = NUMBER_STRATEGIES.find(
      (s) => s.value === "metadata",
    );
    expect(metadataStrategy).toBeDefined();
    expect(
      screen.getByText(metadataStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });

  it("displays correct description for filename strategy", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="filename" onChange={onChange} />,
    );

    const filenameStrategy = NUMBER_STRATEGIES.find(
      (s) => s.value === "filename",
    );
    expect(filenameStrategy).toBeDefined();
    expect(
      screen.getByText(filenameStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });

  it("displays correct description for smart strategy", () => {
    const onChange = vi.fn();

    renderWithProviders(
      <NumberStrategySelector value="smart" onChange={onChange} />,
    );

    const smartStrategy = NUMBER_STRATEGIES.find((s) => s.value === "smart");
    expect(smartStrategy).toBeDefined();
    expect(
      screen.getByText(smartStrategy?.description ?? ""),
    ).toBeInTheDocument();
  });
});

describe("Strategy data constants", () => {
  it("SERIES_STRATEGIES contains all expected strategies", () => {
    const strategyValues = SERIES_STRATEGIES.map((s) => s.value);
    expect(strategyValues).toContain("series_volume");
    expect(strategyValues).toContain("series_volume_chapter");
    expect(strategyValues).toContain("flat");
    expect(strategyValues).toContain("publisher_hierarchy");
    expect(strategyValues).toContain("calibre");
    expect(strategyValues).toContain("custom");
    expect(SERIES_STRATEGIES).toHaveLength(6);
  });

  it("BOOK_STRATEGIES contains all expected strategies", () => {
    const strategyValues = BOOK_STRATEGIES.map((s) => s.value);
    expect(strategyValues).toContain("filename");
    expect(strategyValues).toContain("metadata_first");
    expect(strategyValues).toContain("smart");
    expect(strategyValues).toContain("series_name");
    expect(BOOK_STRATEGIES).toHaveLength(4);
  });

  it("NUMBER_STRATEGIES contains all expected strategies", () => {
    const strategyValues = NUMBER_STRATEGIES.map((s) => s.value);
    expect(strategyValues).toContain("file_order");
    expect(strategyValues).toContain("metadata");
    expect(strategyValues).toContain("filename");
    expect(strategyValues).toContain("smart");
    expect(NUMBER_STRATEGIES).toHaveLength(4);
  });

  it("all series strategies have required fields", () => {
    for (const strategy of SERIES_STRATEGIES) {
      expect(strategy.value).toBeTruthy();
      expect(strategy.label).toBeTruthy();
      expect(strategy.description).toBeTruthy();
      expect(strategy.example).toBeTruthy();
      expect(typeof strategy.hasConfig).toBe("boolean");
    }
  });

  it("all book strategies have required fields", () => {
    for (const strategy of BOOK_STRATEGIES) {
      expect(strategy.value).toBeTruthy();
      expect(strategy.label).toBeTruthy();
      expect(strategy.description).toBeTruthy();
    }
  });

  it("all number strategies have required fields", () => {
    for (const strategy of NUMBER_STRATEGIES) {
      expect(strategy.value).toBeTruthy();
      expect(strategy.label).toBeTruthy();
      expect(strategy.description).toBeTruthy();
    }
  });
});
