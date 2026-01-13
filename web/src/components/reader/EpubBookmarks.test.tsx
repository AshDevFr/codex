import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/utils";

import { EpubBookmarks } from "./EpubBookmarks";
import type { EpubBookmark } from "./hooks/useEpubBookmarks";

const createBookmark = (overrides: Partial<EpubBookmark> = {}): EpubBookmark => ({
	id: `bookmark-${Math.random().toString(36).substring(7)}`,
	cfi: "epubcfi(/6/4!/4/2/1:0)",
	percentage: 0.25,
	note: "",
	createdAt: Date.now(),
	...overrides,
});

describe("EpubBookmarks", () => {
	const defaultProps = {
		bookmarks: [] as EpubBookmark[],
		isCurrentLocationBookmarked: false,
		opened: false,
		onToggle: vi.fn(),
		onAddBookmark: vi.fn(),
		onRemoveCurrentBookmark: vi.fn(),
		onUpdateNote: vi.fn(),
		onRemoveBookmark: vi.fn(),
		onNavigate: vi.fn(),
	};

	describe("bookmark toggle button", () => {
		it("should render add bookmark button when not bookmarked", () => {
			renderWithProviders(
				<EpubBookmarks {...defaultProps} isCurrentLocationBookmarked={false} />
			);

			expect(screen.getByLabelText("Add bookmark")).toBeInTheDocument();
		});

		it("should render remove bookmark button when bookmarked", () => {
			renderWithProviders(
				<EpubBookmarks {...defaultProps} isCurrentLocationBookmarked={true} />
			);

			expect(screen.getByLabelText("Remove bookmark")).toBeInTheDocument();
		});

		it("should call onAddBookmark when clicking add button", async () => {
			const user = userEvent.setup();
			const onAddBookmark = vi.fn();

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					isCurrentLocationBookmarked={false}
					onAddBookmark={onAddBookmark}
				/>
			);

			await user.click(screen.getByLabelText("Add bookmark"));

			expect(onAddBookmark).toHaveBeenCalledTimes(1);
		});

		it("should call onRemoveCurrentBookmark when clicking remove button", async () => {
			const user = userEvent.setup();
			const onRemoveCurrentBookmark = vi.fn();

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					isCurrentLocationBookmarked={true}
					onRemoveCurrentBookmark={onRemoveCurrentBookmark}
				/>
			);

			await user.click(screen.getByLabelText("Remove bookmark"));

			expect(onRemoveCurrentBookmark).toHaveBeenCalledTimes(1);
		});
	});

	describe("bookmark count button", () => {
		it("should not render count button when no bookmarks", () => {
			renderWithProviders(<EpubBookmarks {...defaultProps} bookmarks={[]} />);

			expect(screen.queryByLabelText("View bookmarks")).not.toBeInTheDocument();
		});

		it("should render count button when there are bookmarks", () => {
			const bookmarks = [createBookmark()];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} />
			);

			expect(screen.getByLabelText("View bookmarks")).toBeInTheDocument();
			expect(screen.getByText("1")).toBeInTheDocument();
		});

		it("should show correct bookmark count", () => {
			const bookmarks = [
				createBookmark({ id: "1" }),
				createBookmark({ id: "2" }),
				createBookmark({ id: "3" }),
			];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} />
			);

			expect(screen.getByText("3")).toBeInTheDocument();
		});

		it("should call onToggle when clicking count button", async () => {
			const user = userEvent.setup();
			const onToggle = vi.fn();
			const bookmarks = [createBookmark()];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					onToggle={onToggle}
				/>
			);

			await user.click(screen.getByLabelText("View bookmarks"));

			expect(onToggle).toHaveBeenCalledTimes(1);
		});
	});

	describe("bookmarks drawer", () => {
		it("should show drawer when opened", () => {
			const bookmarks = [createBookmark({ note: "Test note" })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByRole("dialog")).toBeInTheDocument();
			expect(screen.getByText("Bookmarks (1)")).toBeInTheDocument();
		});

		it("should show empty state when no bookmarks", () => {
			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={[]} opened={true} />
			);

			expect(
				screen.getByText("No bookmarks yet. Click the bookmark icon to add one.")
			).toBeInTheDocument();
		});

		it("should display bookmark with chapter title", () => {
			const bookmarks = [createBookmark({ chapterTitle: "Chapter 1" })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByText("Chapter 1")).toBeInTheDocument();
		});

		it("should display bookmark percentage", () => {
			const bookmarks = [createBookmark({ percentage: 0.42 })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByText(/42%/)).toBeInTheDocument();
		});

		it("should display bookmark excerpt", () => {
			const bookmarks = [createBookmark({ excerpt: "This is a test excerpt" })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByText(/"This is a test excerpt"/)).toBeInTheDocument();
		});

		it("should display bookmark note", () => {
			const bookmarks = [createBookmark({ note: "My important note" })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByText("My important note")).toBeInTheDocument();
		});

		it("should sort bookmarks by percentage", () => {
			const bookmarks = [
				createBookmark({ id: "1", percentage: 0.75, chapterTitle: "Later" }),
				createBookmark({ id: "2", percentage: 0.25, chapterTitle: "Earlier" }),
				createBookmark({ id: "3", percentage: 0.50, chapterTitle: "Middle" }),
			];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			const items = screen.getAllByText(/Chapter|Earlier|Middle|Later/);
			expect(items[0]).toHaveTextContent("Earlier");
			expect(items[1]).toHaveTextContent("Middle");
			expect(items[2]).toHaveTextContent("Later");
		});
	});

	describe("bookmark navigation", () => {
		it("should call onNavigate when clicking bookmark item", async () => {
			const user = userEvent.setup();
			const onNavigate = vi.fn();
			const bookmarks = [
				createBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					chapterTitle: "Chapter 1",
				}),
			];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					opened={true}
					onNavigate={onNavigate}
				/>
			);

			await user.click(screen.getByText("Chapter 1"));

			expect(onNavigate).toHaveBeenCalledWith("epubcfi(/6/4!/4/2/1:0)");
		});

		it("should call onToggle to close drawer after navigation", async () => {
			const user = userEvent.setup();
			const onToggle = vi.fn();
			const bookmarks = [createBookmark({ chapterTitle: "Chapter 1" })];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					opened={true}
					onToggle={onToggle}
				/>
			);

			await user.click(screen.getByText("Chapter 1"));

			expect(onToggle).toHaveBeenCalled();
		});
	});

	describe("bookmark removal", () => {
		it("should call onRemoveBookmark when clicking remove button", async () => {
			const user = userEvent.setup();
			const onRemoveBookmark = vi.fn();
			const bookmarks = [createBookmark({ id: "test-id" })];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					opened={true}
					onRemoveBookmark={onRemoveBookmark}
				/>
			);

			await user.click(screen.getByLabelText("Remove bookmark"));

			expect(onRemoveBookmark).toHaveBeenCalledWith("test-id");
		});
	});

	describe("note editing", () => {
		it("should show edit button for each bookmark", () => {
			const bookmarks = [createBookmark()];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			expect(screen.getByLabelText("Edit note")).toBeInTheDocument();
		});

		it("should show textarea when clicking edit button", async () => {
			const user = userEvent.setup();
			const bookmarks = [createBookmark({ note: "Existing note" })];

			renderWithProviders(
				<EpubBookmarks {...defaultProps} bookmarks={bookmarks} opened={true} />
			);

			await user.click(screen.getByLabelText("Edit note"));

			expect(screen.getByRole("textbox")).toBeInTheDocument();
			expect(screen.getByRole("textbox")).toHaveValue("Existing note");
		});

		it("should call onUpdateNote when saving edited note", async () => {
			const user = userEvent.setup();
			const onUpdateNote = vi.fn();
			const bookmarks = [createBookmark({ id: "test-id", note: "" })];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					opened={true}
					onUpdateNote={onUpdateNote}
				/>
			);

			await user.click(screen.getByLabelText("Edit note"));
			await user.type(screen.getByRole("textbox"), "New note content");
			await user.click(screen.getByText("Save"));

			expect(onUpdateNote).toHaveBeenCalledWith("test-id", "New note content");
		});

		it("should discard changes when clicking cancel", async () => {
			const user = userEvent.setup();
			const onUpdateNote = vi.fn();
			const bookmarks = [createBookmark({ note: "Original note" })];

			renderWithProviders(
				<EpubBookmarks
					{...defaultProps}
					bookmarks={bookmarks}
					opened={true}
					onUpdateNote={onUpdateNote}
				/>
			);

			await user.click(screen.getByLabelText("Edit note"));
			await user.clear(screen.getByRole("textbox"));
			await user.type(screen.getByRole("textbox"), "Changed note");
			await user.click(screen.getByText("Cancel"));

			expect(onUpdateNote).not.toHaveBeenCalled();
			// Should still show original note
			expect(screen.getByText("Original note")).toBeInTheDocument();
		});
	});
});
