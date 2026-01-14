import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { FilterGroupState } from "@/types";
import { FilterGroup } from "./FilterGroup";

describe("FilterGroup", () => {
	const defaultOptions = [
		{ value: "action", label: "Action", count: 10 },
		{ value: "comedy", label: "Comedy", count: 5 },
		{ value: "drama", label: "Drama", count: 3 },
	];

	const createState = (
		values: Record<string, "include" | "exclude"> = {},
		mode: "allOf" | "anyOf" = "anyOf",
	): FilterGroupState => ({
		mode,
		values: new Map(Object.entries(values)),
	});

	it("should render title", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		expect(screen.getByText("Genres")).toBeInTheDocument();
	});

	it("should render all options as chips", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		expect(screen.getByText("Action")).toBeInTheDocument();
		expect(screen.getByText("Comedy")).toBeInTheDocument();
		expect(screen.getByText("Drama")).toBeInTheDocument();
	});

	it("should render counts when provided", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		expect(screen.getByText("10")).toBeInTheDocument();
		expect(screen.getByText("5")).toBeInTheDocument();
		expect(screen.getByText("3")).toBeInTheDocument();
	});

	it("should render mode toggle", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		expect(screen.getByText("All")).toBeInTheDocument();
		expect(screen.getByText("Any")).toBeInTheDocument();
	});

	it("should not render mode toggle when showModeToggle is false", () => {
		renderWithProviders(
			<FilterGroup
				title="Status"
				options={defaultOptions}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
				showModeToggle={false}
			/>,
		);

		expect(screen.queryByText("All")).not.toBeInTheDocument();
		expect(screen.queryByText("Any")).not.toBeInTheDocument();
	});

	it("should call onValueChange when chip is clicked", async () => {
		const user = userEvent.setup();
		const onValueChange = vi.fn();

		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState()}
				onValueChange={onValueChange}
				onModeChange={vi.fn()}
			/>,
		);

		await user.click(screen.getByText("Action"));

		expect(onValueChange).toHaveBeenCalledWith("action", "include");
	});

	it("should call onModeChange when mode is toggled", async () => {
		const user = userEvent.setup();
		const onModeChange = vi.fn();

		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState({}, "anyOf")}
				onValueChange={vi.fn()}
				onModeChange={onModeChange}
			/>,
		);

		await user.click(screen.getByText("All"));

		expect(onModeChange).toHaveBeenCalledWith("allOf");
	});

	it("should show empty message when no options", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={[]}
				state={createState()}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		expect(screen.getByText("No options available")).toBeInTheDocument();
	});

	it("should reflect state of included values", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState({ action: "include" })}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		const actionBadge = screen.getByText("Action").closest("[data-state]");
		expect(actionBadge).toHaveAttribute("data-state", "include");
	});

	it("should reflect state of excluded values", () => {
		renderWithProviders(
			<FilterGroup
				title="Genres"
				options={defaultOptions}
				state={createState({ comedy: "exclude" })}
				onValueChange={vi.fn()}
				onModeChange={vi.fn()}
			/>,
		);

		const comedyBadge = screen.getByText("Comedy").closest("[data-state]");
		expect(comedyBadge).toHaveAttribute("data-state", "exclude");
	});

	describe("clear button", () => {
		it("should not show clear button when no active filters", () => {
			const onClear = vi.fn();

			renderWithProviders(
				<FilterGroup
					title="Genres"
					options={defaultOptions}
					state={createState()}
					onValueChange={vi.fn()}
					onModeChange={vi.fn()}
					onClear={onClear}
				/>,
			);

			expect(
				screen.queryByRole("button", { name: /clear genres/i }),
			).not.toBeInTheDocument();
		});

		it("should show clear button when there are active filters", () => {
			const onClear = vi.fn();

			renderWithProviders(
				<FilterGroup
					title="Genres"
					options={defaultOptions}
					state={createState({ action: "include" })}
					onValueChange={vi.fn()}
					onModeChange={vi.fn()}
					onClear={onClear}
				/>,
			);

			expect(
				screen.getByRole("button", { name: /clear genres/i }),
			).toBeInTheDocument();
		});

		it("should call onClear when clear button is clicked", async () => {
			const user = userEvent.setup();
			const onClear = vi.fn();

			renderWithProviders(
				<FilterGroup
					title="Genres"
					options={defaultOptions}
					state={createState({ action: "include", comedy: "exclude" })}
					onValueChange={vi.fn()}
					onModeChange={vi.fn()}
					onClear={onClear}
				/>,
			);

			await user.click(screen.getByRole("button", { name: /clear genres/i }));

			expect(onClear).toHaveBeenCalledTimes(1);
		});

		it("should not show clear button when onClear is not provided", () => {
			renderWithProviders(
				<FilterGroup
					title="Genres"
					options={defaultOptions}
					state={createState({ action: "include" })}
					onValueChange={vi.fn()}
					onModeChange={vi.fn()}
				/>,
			);

			expect(
				screen.queryByRole("button", { name: /clear genres/i }),
			).not.toBeInTheDocument();
		});
	});
});
