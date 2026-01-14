import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { type ImageInfo, ImageUploader } from "./ImageUploader";

describe("ImageUploader", () => {
	const mockImage: ImageInfo = {
		url: "https://example.com/image.jpg",
		size: 128800,
		width: 460,
		height: 651,
		mimeType: "image/jpeg",
	};

	it("renders dropzone with label", () => {
		renderWithProviders(<ImageUploader value={null} onChange={vi.fn()} />);

		expect(
			screen.getByText("Choose an image - drag and drop"),
		).toBeInTheDocument();
	});

	it("renders custom label", () => {
		renderWithProviders(
			<ImageUploader value={null} onChange={vi.fn()} label="Upload poster" />,
		);

		expect(screen.getByText("Upload poster")).toBeInTheDocument();
	});

	it("shows image preview when value is set", () => {
		renderWithProviders(<ImageUploader value={mockImage} onChange={vi.fn()} />);

		expect(screen.getByAltText("Preview")).toBeInTheDocument();
	});

	it("shows image metadata", () => {
		renderWithProviders(<ImageUploader value={mockImage} onChange={vi.fn()} />);

		expect(screen.getByText("Size: 125.8 kB")).toBeInTheDocument();
		expect(screen.getByText("Dimensions: 460 × 651")).toBeInTheDocument();
		expect(screen.getByText("Type: image/jpeg")).toBeInTheDocument();
	});

	it("shows delete button when image is set", () => {
		renderWithProviders(<ImageUploader value={mockImage} onChange={vi.fn()} />);

		expect(screen.getByLabelText("Delete image")).toBeInTheDocument();
	});

	it("calls onChange with null when delete is clicked", async () => {
		const onChange = vi.fn();
		const user = userEvent.setup();

		renderWithProviders(
			<ImageUploader value={mockImage} onChange={onChange} />,
		);

		const deleteButton = screen.getByLabelText("Delete image");
		await user.click(deleteButton);

		expect(onChange).toHaveBeenCalledWith(null);
	});

	it("shows refresh button when onRefresh is provided", () => {
		renderWithProviders(
			<ImageUploader
				value={mockImage}
				onChange={vi.fn()}
				onRefresh={vi.fn()}
			/>,
		);

		expect(screen.getByLabelText("Reset to original")).toBeInTheDocument();
	});

	it("calls onRefresh when refresh is clicked", async () => {
		const onRefresh = vi.fn();
		const user = userEvent.setup();

		renderWithProviders(
			<ImageUploader
				value={mockImage}
				onChange={vi.fn()}
				onRefresh={onRefresh}
			/>,
		);

		const refreshButton = screen.getByLabelText("Reset to original");
		await user.click(refreshButton);

		expect(onRefresh).toHaveBeenCalled();
	});

	it("does not show refresh button when onRefresh is not provided", () => {
		renderWithProviders(<ImageUploader value={mockImage} onChange={vi.fn()} />);

		expect(
			screen.queryByLabelText("Reset to original"),
		).not.toBeInTheDocument();
	});
});
