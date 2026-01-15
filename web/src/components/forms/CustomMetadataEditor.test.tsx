import { fireEvent, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, userEvent } from "@/test/utils";
import { CustomMetadataEditor } from "./CustomMetadataEditor";

describe("CustomMetadataEditor", () => {
	const mockOnChange = vi.fn();
	const mockOnLockChange = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("should render with null value", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		expect(screen.getByText("Custom Metadata")).toBeInTheDocument();
	});

	it("should render with empty object value", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={{}}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		expect(screen.getByText("Custom Metadata")).toBeInTheDocument();
		// Should show empty state hint
		expect(
			screen.getByText(/No custom metadata yet/i),
		).toBeInTheDocument();
	});

	it("should render with existing metadata", () => {
		const testData = {
			notes: "Test notes",
			rating: 5,
			tags: ["tag1", "tag2"],
		};

		renderWithProviders(
			<CustomMetadataEditor
				value={testData}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		expect(screen.getByText("Custom Metadata")).toBeInTheDocument();
		// The JsonEditor should render the data
		expect(screen.queryByText(/No custom metadata yet/i)).not.toBeInTheDocument();
	});

	it("should show locked state correctly", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={true}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Should show lock icon when locked
		const lockButton = screen.getByRole("button", { name: /unlock field/i });
		expect(lockButton).toBeInTheDocument();
	});

	it("should show unlocked state correctly", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Should show unlock icon when unlocked
		const lockButton = screen.getByRole("button", { name: /lock field/i });
		expect(lockButton).toBeInTheDocument();
	});

	it("should toggle lock state when lock button is clicked", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const lockButton = screen.getByRole("button", { name: /lock field/i });
		await user.click(lockButton);

		expect(mockOnLockChange).toHaveBeenCalledWith(true);
	});

	it("should switch between tree and JSON view modes", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={{ test: "value" }}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Should start in tree view (default)
		expect(
			screen.getByText(/Click on values to edit them/i),
		).toBeInTheDocument();

		// Click on JSON tab
		const jsonTab = screen.getByRole("radio", { name: /json/i });
		await user.click(jsonTab);

		// Should now show JSON view help text
		await waitFor(() => {
			expect(
				screen.getByText(/Edit the raw JSON directly/i),
			).toBeInTheDocument();
		});
	});

	it("should show clear button when metadata exists", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={{ test: "value" }}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const clearButton = screen.getByRole("button", {
			name: /clear custom metadata/i,
		});
		expect(clearButton).toBeInTheDocument();
	});

	it("should not show clear button when metadata is empty", () => {
		renderWithProviders(
			<CustomMetadataEditor
				value={{}}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const clearButton = screen.queryByRole("button", {
			name: /clear custom metadata/i,
		});
		expect(clearButton).not.toBeInTheDocument();
	});

	it("should call onChange with null when clear button is clicked", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={{ test: "value" }}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const clearButton = screen.getByRole("button", {
			name: /clear custom metadata/i,
		});
		await user.click(clearButton);

		expect(mockOnChange).toHaveBeenCalledWith(null);
	});

	it("should show Load Example menu in empty state", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const loadExampleButton = screen.getByRole("button", {
			name: /load example/i,
		});
		expect(loadExampleButton).toBeInTheDocument();

		// Open the menu
		await user.click(loadExampleButton);

		// Should show menu items - wait for dropdown to appear
		await waitFor(() => {
			expect(screen.getByText("Minimal (status + rating)")).toBeInTheDocument();
		});
		expect(screen.getByText("Reading List")).toBeInTheDocument();
		expect(screen.getByText("External Links")).toBeInTheDocument();
		expect(screen.getByText("Collection Info")).toBeInTheDocument();
		expect(screen.getByText("Simple Key-Value")).toBeInTheDocument();
	});

	it("should load minimal example when selected from menu", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const loadExampleButton = screen.getByRole("button", {
			name: /load example/i,
		});
		await user.click(loadExampleButton);

		// Wait for menu to open and click on Minimal example
		await waitFor(() => {
			expect(screen.getByText("Minimal (status + rating)")).toBeInTheDocument();
		});
		await user.click(screen.getByText("Minimal (status + rating)"));

		// Should call onChange with minimal metadata
		expect(mockOnChange).toHaveBeenCalledWith(
			expect.objectContaining({
				status: expect.any(String),
				rating: expect.any(Number),
			})
		);
	});

	it("should load simple key-value example when selected from menu", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		const loadExampleButton = screen.getByRole("button", {
			name: /load example/i,
		});
		await user.click(loadExampleButton);

		// Wait for menu to open and click on Simple Key-Value
		await waitFor(() => {
			expect(screen.getByText("Simple Key-Value")).toBeInTheDocument();
		});
		await user.click(screen.getByText("Simple Key-Value"));

		expect(mockOnChange).toHaveBeenCalledWith({ example: "value" });
	});

	it("should auto-lock when value changes from original in JSON view", async () => {
		const user = userEvent.setup();

		// Use a controlled component to simulate the change
		const TestComponent = () => {
			const [value, setValue] = useState<Record<string, unknown> | null>(null);
			const [locked, setLocked] = useState(false);

			return (
				<CustomMetadataEditor
					value={value}
					onChange={(newValue) => {
						setValue(newValue);
						mockOnChange(newValue);
					}}
					locked={locked}
					onLockChange={(newLocked) => {
						setLocked(newLocked);
						mockOnLockChange(newLocked);
					}}
					originalValue={null}
					autoLock={true}
				/>
			);
		};

		renderWithProviders(<TestComponent />);

		// Switch to JSON view
		const jsonTab = screen.getByRole("radio", { name: /json/i });
		await user.click(jsonTab);

		// Find the textarea and enter valid JSON that differs from original
		const textarea = screen.getByRole("textbox");
		fireEvent.change(textarea, { target: { value: '{"test": "value"}' } });

		// Should auto-lock because value changed from original (null)
		await waitFor(() => {
			expect(mockOnLockChange).toHaveBeenCalledWith(true);
		});
	});

	it("should not auto-lock when autoLock is false", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={null}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
				originalValue={null}
				autoLock={false}
			/>,
		);

		// Click Load Example menu and select Simple Key-Value
		const loadExampleButton = screen.getByRole("button", {
			name: /load example/i,
		});
		await user.click(loadExampleButton);

		// Wait for menu to open
		await waitFor(() => {
			expect(screen.getByText("Simple Key-Value")).toBeInTheDocument();
		});
		await user.click(screen.getByText("Simple Key-Value"));

		// Should not auto-lock because autoLock is false
		expect(mockOnLockChange).not.toHaveBeenCalled();
	});

	it("should show JSON validation error for invalid JSON in raw view", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={{ test: "value" }}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Switch to JSON view
		const jsonTab = screen.getByRole("radio", { name: /json/i });
		await user.click(jsonTab);

		// Find the textarea and enter invalid JSON
		const textarea = screen.getByRole("textbox");
		// Use fireEvent.change to directly set the value and trigger onChange
		fireEvent.change(textarea, { target: { value: "{invalid json" } });

		// Should show error - the Alert title is "Invalid JSON"
		await waitFor(() => {
			// The Alert shows with title "Invalid JSON" and body contains the actual parse error
			expect(screen.getByRole("alert")).toBeInTheDocument();
		});
	});

	it("should show error for non-object JSON in raw view", async () => {
		const user = userEvent.setup();

		renderWithProviders(
			<CustomMetadataEditor
				value={{ test: "value" }}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Switch to JSON view
		const jsonTab = screen.getByRole("radio", { name: /json/i });
		await user.click(jsonTab);

		// Find the textarea and enter an array (not an object)
		const textarea = screen.getByRole("textbox");
		// Use fireEvent.change to directly set the value
		fireEvent.change(textarea, {
			target: { value: '["array", "not", "object"]' },
		});

		// Should show error about requiring object
		await waitFor(() => {
			expect(
				screen.getByText(/Custom metadata must be a JSON object/i),
			).toBeInTheDocument();
		});
	});

	it("should handle complex nested objects", () => {
		const complexData = {
			level1: {
				level2: {
					level3: {
						value: "deep",
					},
				},
			},
			array: [1, 2, 3],
			mixed: {
				string: "text",
				number: 42,
				boolean: true,
				null: null,
			},
		};

		renderWithProviders(
			<CustomMetadataEditor
				value={complexData}
				onChange={mockOnChange}
				locked={false}
				onLockChange={mockOnLockChange}
			/>,
		);

		// Should render without errors
		expect(screen.getByText("Custom Metadata")).toBeInTheDocument();
	});
});
