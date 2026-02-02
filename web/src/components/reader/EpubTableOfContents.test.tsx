import type { NavItem } from "epubjs";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders, screen, userEvent } from "@/test/utils";

import { EpubTableOfContents } from "./EpubTableOfContents";

const mockToc: NavItem[] = [
  {
    id: "chapter-1",
    href: "chapter1.xhtml",
    label: "Chapter 1: Introduction",
    subitems: [],
  },
  {
    id: "chapter-2",
    href: "chapter2.xhtml",
    label: "Chapter 2: Getting Started",
    subitems: [
      {
        id: "chapter-2-1",
        href: "chapter2-1.xhtml",
        label: "2.1 Setup",
        subitems: [],
      },
      {
        id: "chapter-2-2",
        href: "chapter2-2.xhtml",
        label: "2.2 Configuration",
        subitems: [],
      },
    ],
  },
  {
    id: "chapter-3",
    href: "chapter3.xhtml",
    label: "Chapter 3: Advanced Topics",
    subitems: [],
  },
];

describe("EpubTableOfContents", () => {
  describe("Toggle button", () => {
    it("renders TOC toggle button", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={false}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(
        screen.getByRole("button", { name: /table of contents/i }),
      ).toBeInTheDocument();
    });

    it("calls onToggle when button is clicked", async () => {
      const user = userEvent.setup();
      const onToggle = vi.fn();

      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={false}
          onToggle={onToggle}
          onNavigate={vi.fn()}
        />,
      );

      await user.click(
        screen.getByRole("button", { name: /table of contents/i }),
      );

      expect(onToggle).toHaveBeenCalledTimes(1);
    });
  });

  describe("Drawer", () => {
    it("does not show drawer content when closed", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={false}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      // When drawer is closed, TOC items should not be visible
      expect(
        screen.queryByText("Chapter 1: Introduction"),
      ).not.toBeInTheDocument();
    });

    it("shows drawer content when opened", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={true}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      // Drawer title and content should be visible
      expect(screen.getByText("Table of Contents")).toBeInTheDocument();
      expect(screen.getByText("Chapter 1: Introduction")).toBeInTheDocument();
    });
  });

  describe("TOC items", () => {
    it("renders all top-level TOC items", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={true}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(screen.getByText("Chapter 1: Introduction")).toBeInTheDocument();
      expect(
        screen.getByText("Chapter 2: Getting Started"),
      ).toBeInTheDocument();
      expect(
        screen.getByText("Chapter 3: Advanced Topics"),
      ).toBeInTheDocument();
    });

    it("renders nested TOC items (subitems)", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={true}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(screen.getByText("2.1 Setup")).toBeInTheDocument();
      expect(screen.getByText("2.2 Configuration")).toBeInTheDocument();
    });

    it("calls onNavigate with href when TOC item is clicked", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();
      const onToggle = vi.fn();

      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={true}
          onToggle={onToggle}
          onNavigate={onNavigate}
        />,
      );

      await user.click(screen.getByText("Chapter 1: Introduction"));

      expect(onNavigate).toHaveBeenCalledWith("chapter1.xhtml");
      expect(onToggle).toHaveBeenCalledTimes(1); // Should close drawer
    });

    it("calls onNavigate with nested item href", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();

      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          opened={true}
          onToggle={vi.fn()}
          onNavigate={onNavigate}
        />,
      );

      await user.click(screen.getByText("2.1 Setup"));

      expect(onNavigate).toHaveBeenCalledWith("chapter2-1.xhtml");
    });
  });

  describe("Empty state", () => {
    it("shows empty message when TOC is empty", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={[]}
          opened={true}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(
        screen.getByText("No table of contents available"),
      ).toBeInTheDocument();
    });
  });

  describe("Current chapter highlighting", () => {
    it("renders current chapter link", () => {
      renderWithProviders(
        <EpubTableOfContents
          toc={mockToc}
          currentHref="chapter2.xhtml"
          opened={true}
          onToggle={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      // The NavLink component handles active state internally
      // We verify the link is rendered correctly
      const chapter2Link = screen.getByText("Chapter 2: Getting Started");
      expect(chapter2Link).toBeInTheDocument();
    });
  });
});
