import type { NavItem } from "epubjs";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders, screen, userEvent } from "@/test/utils";

import {
  EpubTableOfContentsDrawer,
  EpubTableOfContentsTrigger,
} from "./EpubTableOfContents";

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

describe("EpubTableOfContentsTrigger", () => {
  it("renders TOC toggle button", () => {
    renderWithProviders(<EpubTableOfContentsTrigger onToggle={vi.fn()} />);

    expect(
      screen.getByRole("button", { name: /table of contents/i }),
    ).toBeInTheDocument();
  });

  it("calls onToggle when button is clicked", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();

    renderWithProviders(<EpubTableOfContentsTrigger onToggle={onToggle} />);

    await user.click(
      screen.getByRole("button", { name: /table of contents/i }),
    );

    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});

describe("EpubTableOfContentsDrawer", () => {
  describe("Drawer", () => {
    it("does not show drawer content when closed", () => {
      renderWithProviders(
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={false}
          onClose={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(
        screen.queryByText("Chapter 1: Introduction"),
      ).not.toBeInTheDocument();
    });

    it("shows drawer content when opened", () => {
      renderWithProviders(
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={true}
          onClose={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(screen.getByText("Table of Contents")).toBeInTheDocument();
      expect(screen.getByText("Chapter 1: Introduction")).toBeInTheDocument();
    });
  });

  describe("TOC items", () => {
    it("renders all top-level TOC items", () => {
      renderWithProviders(
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={true}
          onClose={vi.fn()}
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
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={true}
          onClose={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      expect(screen.getByText("2.1 Setup")).toBeInTheDocument();
      expect(screen.getByText("2.2 Configuration")).toBeInTheDocument();
    });

    it("calls onNavigate with href and closes drawer when TOC item is clicked", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();
      const onClose = vi.fn();

      renderWithProviders(
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={true}
          onClose={onClose}
          onNavigate={onNavigate}
        />,
      );

      await user.click(screen.getByText("Chapter 1: Introduction"));

      expect(onNavigate).toHaveBeenCalledWith("chapter1.xhtml");
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("calls onNavigate with nested item href", async () => {
      const user = userEvent.setup();
      const onNavigate = vi.fn();

      renderWithProviders(
        <EpubTableOfContentsDrawer
          toc={mockToc}
          opened={true}
          onClose={vi.fn()}
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
        <EpubTableOfContentsDrawer
          toc={[]}
          opened={true}
          onClose={vi.fn()}
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
        <EpubTableOfContentsDrawer
          toc={mockToc}
          currentHref="chapter2.xhtml"
          opened={true}
          onClose={vi.fn()}
          onNavigate={vi.fn()}
        />,
      );

      const chapter2Link = screen.getByText("Chapter 2: Getting Started");
      expect(chapter2Link).toBeInTheDocument();
    });
  });
});
