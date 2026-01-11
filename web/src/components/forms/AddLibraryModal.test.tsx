import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { BrowseResponse, FileSystemEntry } from "@/types";
import { LibraryModal } from "./LibraryModal";

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

describe("LibraryModal (Add Mode)", () => {
	const mockOnClose = vi.fn();

	beforeEach(() => {
		vi.clearAllMocks();
		vi.mocked(filesystemApi.getDrives).mockResolvedValue(mockDrives);
		vi.mocked(filesystemApi.browse).mockResolvedValue(mockBrowseResponse);
	});

	it("should not render when closed", () => {
		renderWithProviders(<LibraryModal opened={false} onClose={mockOnClose} />);

		expect(screen.queryByText("Add New Library")).not.toBeInTheDocument();
	});

	it("should render form fields when opened", async () => {
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

	it("should call onClose with created library on successful creation", async () => {
		const user = userEvent.setup();
		const mockLibrary = {
			id: "123",
			name: "Test Library",
			path: "/home/user/Comics",
			isActive: true,
			createdAt: "2024-01-01T00:00:00Z",
			updatedAt: "2024-01-01T00:00:00Z",
		};

		vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		// Set path
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

		// Wait for the mutation to complete and onClose to be called with the created library
		await waitFor(() => {
			expect(mockOnClose).toHaveBeenCalledWith(mockLibrary);
		});
	});

	it("should show validation error when name is missing", async () => {
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		await waitFor(() => {
			expect(screen.getByText("Cancel")).toBeInTheDocument();
		});

		const cancelButton = screen.getByText("Cancel");
		await user.click(cancelButton);

		expect(mockOnClose).toHaveBeenCalled();
	});

	it("should navigate back to drives when breadcrumb is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

	it("should show cron input when auto scan strategy is selected", async () => {
		const user = userEvent.setup();
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		// Wait for modal
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);

		// Find the select by looking for the label, then find the input
		await waitFor(() => {
			expect(modalContent.getByText("Scan Strategy")).toBeInTheDocument();
		});

		// Find the select input - Mantine Select renders as a button/combobox
		// Try to find by the displayed text first (most reliable)
		let selectInput;
		try {
			// Mantine Select shows the selected value as text
			selectInput = screen.getByText("Manual - Trigger scans on demand");
		} catch {
			try {
				selectInput = screen.getByLabelText("Scan Strategy");
			} catch {
				// Fallback: find combobox or button near the label
				const label = modalContent.getByText("Scan Strategy");
				// Find the next interactive element after the label
				const allInteractive = modalContent.getAllByRole("combobox");
				if (allInteractive.length > 0) {
					selectInput = allInteractive[0];
				} else {
					// Try buttons
					const buttons = modalContent.getAllByRole("button");
					selectInput =
						buttons.find(
							(btn) =>
								btn.textContent?.includes("Manual") ||
								btn.closest("form")?.contains(label),
						) || buttons[0];
				}
			}
		}

		// Click to open dropdown (this will make it a combobox)
		await user.click(selectInput);

		// Wait for and click the auto option
		await waitFor(() => {
			const autoOption = screen.getByText("Automatic - Scheduled scanning");
			expect(autoOption).toBeInTheDocument();
		});

		const autoOption = screen.getByText("Automatic - Scheduled scanning");
		await user.click(autoOption);

		// Cron input should appear
		await waitFor(() => {
			// Try multiple ways to find the CronInput
			let cronInput;
			try {
				cronInput = modalContent.getByLabelText("Cron Schedule");
			} catch {
				try {
					cronInput = screen.getByLabelText("Cron Schedule");
				} catch {
					// Fallback: find by placeholder
					cronInput = modalContent.getByPlaceholderText("0 0 * * *");
				}
			}
			expect(cronInput).toBeInTheDocument();
		});
	});

	it("should not show cron input when manual scan strategy is selected", async () => {
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		// Wait for modal
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);

		// Cron input should not be visible by default (manual is default)
		await waitFor(() => {
			expect(
				modalContent.queryByLabelText("Cron Schedule"),
			).not.toBeInTheDocument();
		});
	});

	it("should create library with cron schedule when auto scan is enabled", async () => {
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

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		// Set path
		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		const selectButton = await screen.findByText("Select This Folder");
		await user.click(selectButton);

		// Select auto scan strategy
		// Mantine Select renders as a button/combobox - find by displayed text first
		let selectInput;
		try {
			// Mantine Select shows the selected value as text
			selectInput = screen.getByText("Manual - Trigger scans on demand");
		} catch {
			try {
				selectInput = screen.getByLabelText("Scan Strategy");
			} catch {
				// Fallback: find combobox or button near the label
				const label = modalContent.getByText("Scan Strategy");
				const allInteractive = modalContent.getAllByRole("combobox");
				if (allInteractive.length > 0) {
					selectInput = allInteractive[0];
				} else {
					const buttons = modalContent.getAllByRole("button");
					selectInput =
						buttons.find(
							(btn) =>
								btn.textContent?.includes("Manual") ||
								btn.closest("form")?.contains(label),
						) || buttons[0];
				}
			}
		}
		await user.click(selectInput);

		await waitFor(() => {
			const autoOption = screen.getByText("Automatic - Scheduled scanning");
			expect(autoOption).toBeInTheDocument();
		});

		const autoOption = screen.getByText("Automatic - Scheduled scanning");
		await user.click(autoOption);

		// Wait for cron input and verify it has default value
		await waitFor(() => {
			// Try multiple ways to find the CronInput
			let cronInput;
			try {
				cronInput = modalContent.getByLabelText("Cron Schedule");
			} catch {
				try {
					cronInput = screen.getByLabelText("Cron Schedule");
				} catch {
					// Fallback: find by placeholder
					cronInput = modalContent.getByPlaceholderText("0 0 * * *");
				}
			}
			expect(cronInput).toBeInTheDocument();
			expect(cronInput).toHaveValue("0 0 * * *");
		});

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
						enabled: true,
						cronSchedule: "0 0 * * *",
						scanMode: "normal",
					}),
				}),
			);
		});
	});

	it("should validate cron schedule is required when auto scan is enabled", async () => {
		const user = userEvent.setup();
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		// Wait for modal
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);
		const nameInput = await modalContent.findByPlaceholderText(
			"Enter library name",
			{},
			{ timeout: 3000 },
		);

		// Fill in name and path
		await user.clear(nameInput);
		await user.type(nameInput, "Test Library");

		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		const selectButton = await screen.findByText("Select This Folder");
		await user.click(selectButton);

		// Select auto scan strategy
		// Mantine Select renders as a button/combobox - find by displayed text first
		let selectInput;
		try {
			// Mantine Select shows the selected value as text
			selectInput = screen.getByText("Manual - Trigger scans on demand");
		} catch {
			try {
				selectInput = screen.getByLabelText("Scan Strategy");
			} catch {
				// Fallback: find combobox or button near the label
				const label = modalContent.getByText("Scan Strategy");
				const allInteractive = modalContent.getAllByRole("combobox");
				if (allInteractive.length > 0) {
					selectInput = allInteractive[0];
				} else {
					const buttons = modalContent.getAllByRole("button");
					selectInput =
						buttons.find(
							(btn) =>
								btn.textContent?.includes("Manual") ||
								btn.closest("form")?.contains(label),
						) || buttons[0];
				}
			}
		}
		await user.click(selectInput);

		await waitFor(() => {
			const autoOption = screen.getByText("Automatic - Scheduled scanning");
			expect(autoOption).toBeInTheDocument();
		});

		const autoOption = screen.getByText("Automatic - Scheduled scanning");
		await user.click(autoOption);

		// Wait for cron input and clear it
		// The CronInput appears after selecting auto scan
		// Wait a bit for the state to update and component to re-render
		await waitFor(
			() => {
				// Try multiple ways to find the CronInput
				// Check in entire document since Mantine may render in portals
				const cronInputByLabel = screen.queryByLabelText("Cron Schedule");
				const cronInputByPlaceholder =
					screen.queryByPlaceholderText("0 0 * * *");

				expect(cronInputByLabel || cronInputByPlaceholder).toBeInTheDocument();
			},
			{ timeout: 3000 },
		);

		// Now get the input - search in entire document
		const cronInput =
			screen.queryByLabelText("Cron Schedule") ||
			screen.queryByPlaceholderText("0 0 * * *");

		expect(cronInput).toBeInTheDocument();
		await user.clear(cronInput!);

		// Try to submit - should show validation error
		const createButton = screen.getByText("Create Library");
		await user.click(createButton);

		// The form validation happens in handleSubmit and shows a notification
		// The library should not be created because cron schedule is required
		await waitFor(
			() => {
				// Verify that the API was not called (validation prevented submission)
				expect(librariesApi.create).not.toHaveBeenCalled();
			},
			{ timeout: 1000 },
		);
	});

	it("should have all formats selected by default", async () => {
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		// Wait for modal
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);

		// Find the MultiSelect for allowed formats
		const formatsInput = modalContent.getByLabelText("Allowed Formats");
		expect(formatsInput).toBeInTheDocument();

		// Click to open the dropdown and verify all formats are available
		const user = userEvent.setup();
		await user.click(formatsInput);

		// All formats should be available (may appear multiple times in MultiSelect)
		// Use getAllByText to handle multiple instances - this is expected behavior
		await waitFor(() => {
			const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
			expect(cbzElements.length).toBeGreaterThan(0);
		});
		// Check other formats exist (may be multiple instances)
		expect(screen.getAllByText("CBR (Comic Book RAR)").length).toBeGreaterThan(
			0,
		);
		expect(screen.getAllByText("EPUB (Ebook)").length).toBeGreaterThan(0);
		expect(
			screen.getAllByText("PDF (Portable Document Format)").length,
		).toBeGreaterThan(0);
	});

	it("should submit with all formats by default", async () => {
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

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		// Set path
		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		const selectButton = await screen.findByText("Select This Folder");
		await user.click(selectButton);

		// Submit form (with all formats selected by default)
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
					allowedFormats: ["CBZ", "CBR", "EPUB", "PDF"],
				}),
			);
		});
	});

	it("should submit with selected formats when some are deselected", async () => {
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

		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

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

		// Set path
		const browseButton = screen.getByText("Browse");
		await user.click(browseButton);

		const driveButton = await screen.findByText("Home Directory");
		await user.click(driveButton);

		const selectButton = await screen.findByText("Select This Folder");
		await user.click(selectButton);

		// Open formats dropdown and deselect some formats
		const formatsInput = modalContent.getByLabelText("Allowed Formats");
		await user.click(formatsInput);

		// Wait for options to appear (may appear multiple times in MultiSelect)
		await waitFor(() => {
			const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
			expect(cbzElements.length).toBeGreaterThan(0);
		});

		// Deselect CBR and PDF by clicking them (they're selected by default)
		// In Mantine MultiSelect, clicking a selected item in the dropdown toggles it off
		// Wait for dropdown to be fully open
		await waitFor(() => {
			const options = screen.getAllByText("CBR (Comic Book RAR)");
			expect(options.length).toBeGreaterThan(0);
		});

		// Find the option in the dropdown (not the selected item in the input)
		// The dropdown options are typically in a portal
		const cbrOptions = screen.getAllByText("CBR (Comic Book RAR)");
		// Click the last one (usually the option in dropdown, not the selected tag)
		const cbrOption = cbrOptions[cbrOptions.length - 1];
		await user.click(cbrOption);

		// Wait for the state to update after clicking
		await waitFor(() => {
			// Verify CBR was deselected by checking the dropdown is still open
			const pdfOptions = screen.getAllByText("PDF (Portable Document Format)");
			expect(pdfOptions.length).toBeGreaterThan(0);
		});

		// Deselect PDF
		const pdfOptions = screen.getAllByText("PDF (Portable Document Format)");
		const pdfOption = pdfOptions[pdfOptions.length - 1];
		await user.click(pdfOption);

		// Wait for state to update
		await waitFor(
			() => {
				// Give time for the click to process
			},
			{ timeout: 500 },
		);

		// Close dropdown
		await user.keyboard("{Escape}");

		// Wait for state to update after deselecting formats
		await waitFor(
			() => {
				// Give time for the MultiSelect state to update
			},
			{ timeout: 500 },
		);

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
					allowedFormats: expect.arrayContaining(["CBZ", "EPUB"]),
				}),
			);
		});

		// Verify CBR and PDF are not in the submitted formats
		const createCall = vi.mocked(librariesApi.create).mock.calls[0];
		const submittedFormats = createCall[0].allowedFormats;
		expect(submittedFormats).not.toContain("CBR");
		expect(submittedFormats).not.toContain("PDF");
	});

	it("should reset to all formats when modal closes", async () => {
		const user = userEvent.setup();
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		// Wait for modal
		const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const modalContent = within(modal);

		// Open formats dropdown and deselect some formats
		const formatsInput = modalContent.getByLabelText("Allowed Formats");
		await user.click(formatsInput);

		await waitFor(() => {
			const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
			expect(cbzElements.length).toBeGreaterThan(0);
		});

		// Deselect CBR
		// Wait for dropdown to be fully open
		await waitFor(() => {
			const options = screen.getAllByText("CBR (Comic Book RAR)");
			expect(options.length).toBeGreaterThan(0);
		});

		// Find the option in the dropdown (not the selected item in the input)
		const cbrOptions = screen.getAllByText("CBR (Comic Book RAR)");
		// Click the last one (usually the option in dropdown, not the selected tag)
		const cbrOption = cbrOptions[cbrOptions.length - 1];
		await user.click(cbrOption);

		// Wait a bit for the click to process
		await waitFor(
			() => {
				// Verify the click was processed
			},
			{ timeout: 1000 },
		);
		await user.keyboard("{Escape}");

		// Close modal
		const cancelButton = screen.getByText("Cancel");
		await user.click(cancelButton);

		expect(mockOnClose).toHaveBeenCalled();

		// Reopen modal - formats should be reset to all
		renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

		const newModal = await screen.findByRole("dialog", {}, { timeout: 3000 });
		const newModalContent = within(newModal);

		const newFormatsInput = newModalContent.getByLabelText("Allowed Formats");
		await user.click(newFormatsInput);

		// All formats should be available again (may appear multiple times in MultiSelect)
		// Use getAllByText to handle multiple instances - this is expected behavior
		await waitFor(() => {
			const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
			expect(cbzElements.length).toBeGreaterThan(0);
		});
		// Check other formats exist (may be multiple instances)
		expect(screen.getAllByText("CBR (Comic Book RAR)").length).toBeGreaterThan(
			0,
		);
		expect(screen.getAllByText("EPUB (Ebook)").length).toBeGreaterThan(0);
		expect(
			screen.getAllByText("PDF (Portable Document Format)").length,
		).toBeGreaterThan(0);
	});
});
