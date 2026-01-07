import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { BrowseResponse, FileSystemEntry } from "@/types/api";
import { AddLibraryModal } from "./AddLibraryModal";

vi.mock("@/api/filesystem");
vi.mock("@/api/libraries");

const mockDrives: FileSystemEntry[] = [
	{
		name: "Home Directory",
		path: "/home/user",
		is_directory: true,
		is_readable: true,
	},
	{
		name: "Root",
		path: "/",
		is_directory: true,
		is_readable: true,
	},
];

const mockBrowseResponse: BrowseResponse = {
	current_path: "/home/user",
	parent_path: "/home",
	entries: [
		{
			name: "Documents",
			path: "/home/user/Documents",
			is_directory: true,
			is_readable: true,
		},
		{
			name: "Comics",
			path: "/home/user/Comics",
			is_directory: true,
			is_readable: true,
		},
		{
			name: "file.txt",
			path: "/home/user/file.txt",
			is_directory: false,
			is_readable: true,
		},
	],
};

describe("AddLibraryModal", () => {
	const mockOnClose = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
		vi.mocked(filesystemApi.getDrives).mockResolvedValue(mockDrives);
		vi.mocked(filesystemApi.browse).mockResolvedValue(mockBrowseResponse);
	});

	it("should not render when closed", () => {
		renderWithProviders(
			<AddLibraryModal opened={false} onClose={mockOnClose} />,
		);

		expect(screen.queryByText("Add New Library")).not.toBeInTheDocument();
	});

	it("should render form fields when opened", async () => {
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Wait for modal to be fully rendered - check for title
		await waitFor(
			() => {
				expect(screen.getByText("Add New Library")).toBeInTheDocument();
			},
			{ timeout: 3000 },
		);

		// Find the modal dialog container
		const modal = await screen.findByRole("dialog", {}, { timeout: 2000 });
		const modalContent = within(modal);

		// Wait for the "Browse" button which confirms the form section is rendered
		// (not the path browser view) - this is a reliable indicator the form is there
		await waitFor(
			() => {
				expect(modalContent.getByText("Browse")).toBeInTheDocument();
			},
			{ timeout: 3000 },
		);

		// Verify inputs exist by placeholder (more reliable than labels in Mantine portals)
		// This is what actually matters - that users can interact with the form
		expect(
			modalContent.getByPlaceholderText("Enter library name"),
		).toBeInTheDocument();
		expect(
			modalContent.getByPlaceholderText("Select a path..."),
		).toBeInTheDocument();

		// Verify Scan Strategy select exists (it should have a label we can find)
		// If labels aren't accessible in the portal, that's a Mantine limitation
		// but the form is functional which is what we're testing
		const scanStrategy = modalContent.getByText("Scan Strategy");
		expect(scanStrategy).toBeInTheDocument();
	});

	it("should open path browser when Browse button is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		await waitFor(() => {
			expect(screen.getByText("Browse")).toBeInTheDocument();
		});

		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		await waitFor(() => {
			expect(screen.getByText("Select Library Path")).toBeInTheDocument();
			expect(filesystemApi.getDrives).toHaveBeenCalled();
		});
	});

	it("should display drives in path browser", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		await waitFor(() => {
			expect(screen.getByText("Browse")).toBeInTheDocument();
		});

		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		await waitFor(() => {
			expect(screen.getByText("Home Directory")).toBeInTheDocument();
			expect(screen.getByText("Root")).toBeInTheDocument();
		});
	});

	it("should browse directory when drive is selected", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Open browser
		const browseButton = await screen.findByText("Browse");
		await user.click(browseButton);

		// Click on a drive
		await waitFor(() => {
			expect(screen.getByText("Home Directory")).toBeInTheDocument();
		});

		const driveButton = screen.getByText("Home Directory");
		await user.click(driveButton);

		await waitFor(() => {
			expect(filesystemApi.browse).toHaveBeenCalledWith("/home/user");
			expect(screen.getByText("Documents")).toBeInTheDocument();
			expect(screen.getByText("Comics")).toBeInTheDocument();
		});
	});

	it("should only show directories, not files", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		const browseButton = await screen.findByText("Browse");
		await user.click(browseButton);

		await waitFor(() => {
			expect(screen.getByText("Home Directory")).toBeInTheDocument();
		});

		const driveButton = screen.getByText("Home Directory");
		await user.click(driveButton);

		await waitFor(() => {
			expect(screen.getByText("Documents")).toBeInTheDocument();
			expect(screen.getByText("Comics")).toBeInTheDocument();
			// File should not be visible
			expect(screen.queryByText("file.txt")).not.toBeInTheDocument();
		});
	});

	it("should select path and auto-generate library name", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Wait for modal and find form fields
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);
		const nameInput = await modalContent.findByPlaceholderText(
			"Enter library name",
			{},
			{ timeout: 3000 },
		);
		expect(nameInput).toBeInTheDocument();

		// Open browser and navigate to a directory
		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		await waitFor(() => {
			expect(screen.getByText("Select This Folder")).toBeInTheDocument();
		});

		const selectButton = screen.getByText("Select This Folder");
		await user.click(selectButton);

		await waitFor(() => {
			// Use placeholder text to find inputs since labels aren't accessible in portal
			const pathInput = modalContent.getByPlaceholderText(
				"Select a path...",
			) as HTMLInputElement;
			expect(pathInput.value).toBe("/home/user");

			const nameInput = modalContent.getByPlaceholderText(
				"Enter library name",
			) as HTMLInputElement;
			expect(nameInput.value).toBe("user");
		});
	});

	it("should navigate to parent directory", async () => {
		const user = userEvent.setup();
		const parentBrowseResponse: BrowseResponse = {
			current_path: "/home",
			parent_path: "/",
			entries: [
				{
					name: "user",
					path: "/home/user",
					is_directory: true,
					is_readable: true,
				},
			],
		};

		vi.mocked(filesystemApi.browse)
			.mockResolvedValueOnce(mockBrowseResponse)
			.mockResolvedValueOnce(parentBrowseResponse);

		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Navigate to a directory
		const browseButton = await screen.findByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		await waitFor(() => {
			expect(screen.getByText("Up One Level")).toBeInTheDocument();
		});

		// Click "Up One Level"
		const upButton = screen.getByText("Up One Level");
		await user.click(upButton);

		await waitFor(() => {
			expect(filesystemApi.browse).toHaveBeenCalledWith("/home");
		});
	});

	it("should create library with valid inputs", async () => {
		const user = userEvent.setup();
		const mockLibrary = {
			id: "1",
			name: "Test Library",
			path: "/home/user/Comics",
			isActive: true,
			createdAt: "2024-01-01T00:00:00Z",
			updatedAt: "2024-01-01T00:00:00Z",
		};

		vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Wait for modal and find form fields
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);
		const nameInput = await modalContent.findByPlaceholderText(
			"Enter library name",
			{},
			{ timeout: 3000 },
		);

		// Fill in form
		await user.clear(nameInput);
		await user.type(nameInput, "Test Library");

		// Set path directly (simulating browse selection)
		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		const selectButton = await screen.findByText("Select This Folder");
		await user.click(selectButton);

		// Submit form
		await waitFor(() => {
			const createButton = screen.getByText("Create Library");
			expect(createButton).not.toBeDisabled();
		});

		const createButton = screen.getByText("Create Library");
		await user.click(createButton);

		await waitFor(() => {
			expect(librariesApi.create).toHaveBeenCalledWith(
				expect.objectContaining({
					name: "Test Library",
					path: "/home/user",
					scanningConfig: expect.objectContaining({
						enabled: false,
						scanMode: "normal",
					}),
				}),
			);
		});
	});

	it("should show validation error when name is missing", async () => {
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Wait for modal and verify form is rendered
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);
		await modalContent.findByPlaceholderText(
			"Enter library name",
			{},
			{ timeout: 3000 },
		);

		// Don't fill in name or path, button should be disabled
		// The button is disabled when !libraryName || !selectedPath
		const createButton = modalContent.getByRole("button", {
			name: /create library/i,
		});
		expect(createButton).toBeDisabled();
	});

	it("should show validation error when path is missing", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Wait for modal and find form fields
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);
		const nameInput = await modalContent.findByPlaceholderText(
			"Enter library name",
			{},
			{ timeout: 3000 },
		);
		await user.type(nameInput, "Test Library");

		// Don't select path, button should be disabled
		const createButton = modalContent.getByRole("button", {
			name: /create library/i,
		});
		expect(createButton).toBeDisabled();
	});

	it("should close modal when Cancel is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		await waitFor(() => {
			expect(screen.getByText("Cancel")).toBeInTheDocument();
		});

		const cancelButton = screen.getByText("Cancel");
		await user.click(cancelButton);

		expect(mockOnClose).toHaveBeenCalled();
	});

	it("should navigate back to drives when breadcrumb is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<AddLibraryModal opened={true} onClose={mockOnClose} />,
		);

		// Navigate to a directory
		const browseButton = await screen.findByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		await waitFor(() => {
			expect(screen.getByText("Drives")).toBeInTheDocument();
		});

		// Click "Drives" breadcrumb
		const drivesLink = screen.getByText("Drives");
		await user.click(drivesLink);

		await waitFor(() => {
			expect(
				screen.getByText("Select a drive or location to browse:"),
			).toBeInTheDocument();
		});
	});
});
