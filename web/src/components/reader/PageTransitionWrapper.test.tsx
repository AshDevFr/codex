import { MantineProvider } from "@mantine/core";
import { act, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getSlideDirection, PageTransitionWrapper } from "./PageTransitionWrapper";

function Wrapper({ children }: { children: ReactNode }) {
	return <MantineProvider>{children}</MantineProvider>;
}

describe("PageTransitionWrapper", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	describe("getSlideDirection", () => {
		it("should return right for LTR + next", () => {
			expect(getSlideDirection("next", "ltr")).toBe("right");
		});

		it("should return left for LTR + prev", () => {
			expect(getSlideDirection("prev", "ltr")).toBe("left");
		});

		it("should return left for RTL + next (reversed)", () => {
			expect(getSlideDirection("next", "rtl")).toBe("left");
		});

		it("should return right for RTL + prev (reversed)", () => {
			expect(getSlideDirection("prev", "rtl")).toBe("right");
		});

		it("should return right when navigation direction is null for LTR/RTL", () => {
			expect(getSlideDirection(null, "ltr")).toBe("right");
			expect(getSlideDirection(null, "rtl")).toBe("right");
		});

		it("should return down for TTB + next (vertical)", () => {
			expect(getSlideDirection("next", "ttb")).toBe("down");
		});

		it("should return up for TTB + prev (vertical)", () => {
			expect(getSlideDirection("prev", "ttb")).toBe("up");
		});

		it("should return down when navigation direction is null for TTB", () => {
			expect(getSlideDirection(null, "ttb")).toBe("down");
		});
	});

	describe("transition none", () => {
		it("should render children directly without wrapper", () => {
			render(
				<PageTransitionWrapper
					pageKey="page-1"
					transition="none"
					duration={200}
					navigationDirection={null}
					readingDirection="ltr"
				>
					<div data-testid="page-content">Page 1</div>
				</PageTransitionWrapper>,
				{ wrapper: Wrapper },
			);

			expect(screen.getByTestId("page-content")).toBeInTheDocument();
			expect(screen.getByText("Page 1")).toBeInTheDocument();
		});

		it("should instantly switch content when pageKey changes", () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="none"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div data-testid="page-content">Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			expect(screen.getByText("Page 1")).toBeInTheDocument();

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="none"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div data-testid="page-content">Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			expect(screen.getByText("Page 2")).toBeInTheDocument();
			expect(screen.queryByText("Page 1")).not.toBeInTheDocument();
		});
	});

	describe("transition fade", () => {
		it("should render children", () => {
			render(
				<PageTransitionWrapper
					pageKey="page-1"
					transition="fade"
					duration={200}
					navigationDirection={null}
					readingDirection="ltr"
				>
					<div data-testid="page-content">Page 1</div>
				</PageTransitionWrapper>,
				{ wrapper: Wrapper },
			);

			expect(screen.getByText("Page 1")).toBeInTheDocument();
		});

		it("should show both old and new content during transition", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="fade"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Advance past the initial frame delay
			vi.advanceTimersByTime(20);

			// Both pages should be visible during transition
			expect(screen.getByText("Page 1")).toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();
		});

		it("should remove old content after transition completes", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="fade"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Advance past transition duration + frame delay
			await act(async () => {
				vi.advanceTimersByTime(220);
			});

			// Only new page should remain
			expect(screen.queryByText("Page 1")).not.toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();
		});
	});

	describe("transition slide", () => {
		it("should render children", () => {
			render(
				<PageTransitionWrapper
					pageKey="page-1"
					transition="slide"
					duration={200}
					navigationDirection={null}
					readingDirection="ltr"
				>
					<div data-testid="page-content">Page 1</div>
				</PageTransitionWrapper>,
				{ wrapper: Wrapper },
			);

			expect(screen.getByText("Page 1")).toBeInTheDocument();
		});

		it("should show both old and new content during transition", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="slide"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="slide"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Advance past the initial frame delay
			vi.advanceTimersByTime(20);

			// Both pages should be visible during transition
			expect(screen.getByText("Page 1")).toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();
		});

		it("should remove old content after transition completes", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="slide"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="slide"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Advance past transition duration + frame delay
			await act(async () => {
				vi.advanceTimersByTime(220);
			});

			// Only new page should remain
			expect(screen.queryByText("Page 1")).not.toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();
		});
	});

	describe("rapid navigation", () => {
		it("should handle rapid page changes", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Change to page 2
			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="fade"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			await act(async () => {
				vi.advanceTimersByTime(50); // Only 50ms into 200ms transition
			});

			// Change to page 3 before first transition completes
			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-3"
						transition="fade"
						duration={200}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 3</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			await act(async () => {
				vi.advanceTimersByTime(20);
			});

			// Page 3 should be visible
			expect(screen.getByText("Page 3")).toBeInTheDocument();

			// Complete all transitions
			await act(async () => {
				vi.advanceTimersByTime(300);
			});

			// Only page 3 should remain
			expect(screen.queryByText("Page 1")).not.toBeInTheDocument();
			expect(screen.queryByText("Page 2")).not.toBeInTheDocument();
			expect(screen.getByText("Page 3")).toBeInTheDocument();
		});
	});

	describe("content updates without page change", () => {
		it("should update content without transition when pageKey stays the same", () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Content A</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			expect(screen.getByText("Content A")).toBeInTheDocument();

			// Same pageKey, different content
			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={200}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Content B</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// Should immediately show new content without transition
			expect(screen.getByText("Content B")).toBeInTheDocument();
			expect(screen.queryByText("Content A")).not.toBeInTheDocument();
		});
	});

	describe("different durations", () => {
		it("should respect custom duration", async () => {
			const { rerender } = render(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-1"
						transition="fade"
						duration={500}
						navigationDirection={null}
						readingDirection="ltr"
					>
						<div>Page 1</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			rerender(
				<Wrapper>
					<PageTransitionWrapper
						pageKey="page-2"
						transition="fade"
						duration={500}
						navigationDirection="next"
						readingDirection="ltr"
					>
						<div>Page 2</div>
					</PageTransitionWrapper>
				</Wrapper>,
			);

			// At 300ms, transition should still be in progress
			await act(async () => {
				vi.advanceTimersByTime(300);
			});
			expect(screen.getByText("Page 1")).toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();

			// At 520ms, transition should be complete
			await act(async () => {
				vi.advanceTimersByTime(220);
			});
			expect(screen.queryByText("Page 1")).not.toBeInTheDocument();
			expect(screen.getByText("Page 2")).toBeInTheDocument();
		});
	});
});
