import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { type AlphabetCounts, AlphabetFilter } from "./AlphabetFilter";

describe("AlphabetFilter", () => {
	it("renders all letters including ALL and #", () => {
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

		expect(screen.getByRole("button", { name: "ALL" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "#" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "A" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Z" })).toBeInTheDocument();
	});

	it("highlights ALL when selected is null", () => {
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

		const allButton = screen.getByRole("button", { name: "ALL" });
		expect(allButton).toHaveAttribute("data-selected");
	});

	it("highlights selected letter", () => {
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected="B" onSelect={onSelect} />);

		const bButton = screen.getByRole("button", { name: "B" });
		expect(bButton).toHaveAttribute("data-selected");

		const allButton = screen.getByRole("button", { name: "ALL" });
		expect(allButton).not.toHaveAttribute("data-selected");
	});

	it("calls onSelect with letter when clicking a letter", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

		await user.click(screen.getByRole("button", { name: "C" }));
		expect(onSelect).toHaveBeenCalledWith("C");
	});

	it("calls onSelect with null when clicking ALL", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected="C" onSelect={onSelect} />);

		await user.click(screen.getByRole("button", { name: "ALL" }));
		expect(onSelect).toHaveBeenCalledWith(null);
	});

	it("calls onSelect with null when clicking already selected letter (toggle off)", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected="D" onSelect={onSelect} />);

		await user.click(screen.getByRole("button", { name: "D" }));
		expect(onSelect).toHaveBeenCalledWith(null);
	});

	it("calls onSelect with # for number filter", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

		await user.click(screen.getByRole("button", { name: "#" }));
		expect(onSelect).toHaveBeenCalledWith("#");
	});

	it("disables letters with no count when counts are provided", () => {
		const onSelect = vi.fn();
		const counts: AlphabetCounts = new Map([
			["a", 5],
			["b", 10],
			// "c" has no count, should be disabled
		]);

		renderWithProviders(
			<AlphabetFilter
				selected={null}
				onSelect={onSelect}
				counts={counts}
				totalCount={15}
			/>,
		);

		// C button should be disabled (no count)
		const cButton = screen.getByRole("button", { name: "C" });
		expect(cButton).toBeDisabled();

		// A button should be enabled (has count)
		const aButton = screen.getByRole("button", { name: "A" });
		expect(aButton).not.toBeDisabled();
	});

	it("aggregates numeric counts for # button", () => {
		const onSelect = vi.fn();
		const counts: AlphabetCounts = new Map([
			["1", 3],
			["2", 2],
			["a", 5],
		]);

		renderWithProviders(
			<AlphabetFilter
				selected={null}
				onSelect={onSelect}
				counts={counts}
				totalCount={10}
			/>,
		);

		// # button should be enabled (sum of numeric counts = 5)
		const hashButton = screen.getByRole("button", { name: "#" });
		expect(hashButton).not.toBeDisabled();
	});
});
