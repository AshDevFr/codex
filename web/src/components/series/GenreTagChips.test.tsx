import { describe, expect, it } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { GenreTagChips } from "./GenreTagChips";

describe("GenreTagChips", () => {
  const mockGenres = [
    {
      id: "genre-1",
      name: "Action",
      seriesCount: 10,
      createdAt: "2024-01-01T00:00:00Z",
    },
    {
      id: "genre-2",
      name: "Adventure",
      seriesCount: 5,
      createdAt: "2024-01-01T00:00:00Z",
    },
    {
      id: "genre-3",
      name: "Comedy",
      seriesCount: 8,
      createdAt: "2024-01-01T00:00:00Z",
    },
  ];

  const mockTags = [
    {
      id: "tag-1",
      name: "Favorite",
      seriesCount: 15,
      createdAt: "2024-01-01T00:00:00Z",
    },
    {
      id: "tag-2",
      name: "Completed",
      seriesCount: 20,
      createdAt: "2024-01-01T00:00:00Z",
    },
  ];

  it("should render nothing when no genres or tags", () => {
    renderWithProviders(<GenreTagChips />);

    // No badges should be rendered
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("should render genres as blue badges", () => {
    renderWithProviders(<GenreTagChips genres={mockGenres} />);

    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    expect(screen.getByText("Comedy")).toBeInTheDocument();
  });

  it("should render tags as gray badges", () => {
    renderWithProviders(<GenreTagChips tags={mockTags} />);

    expect(screen.getByText("Favorite")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("should render both genres and tags together", () => {
    renderWithProviders(<GenreTagChips genres={mockGenres} tags={mockTags} />);

    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Favorite")).toBeInTheDocument();
  });

  it("should have correct href on genre link when clickable", () => {
    renderWithProviders(<GenreTagChips genres={mockGenres} clickable={true} />);

    const actionLink = screen.getByText("Action").closest("a");
    expect(actionLink).toHaveAttribute(
      "href",
      "/libraries/all/series?gf=any:Action",
    );
  });

  it("should have href with libraryId in path when provided", () => {
    renderWithProviders(
      <GenreTagChips
        genres={mockGenres}
        libraryId="lib-123"
        clickable={true}
      />,
    );

    const actionLink = screen.getByText("Action").closest("a");
    expect(actionLink).toHaveAttribute(
      "href",
      "/libraries/lib-123/series?gf=any:Action",
    );
  });

  it("should have correct href on tag link when clickable", () => {
    renderWithProviders(<GenreTagChips tags={mockTags} clickable={true} />);

    const favoriteLink = screen.getByText("Favorite").closest("a");
    expect(favoriteLink).toHaveAttribute(
      "href",
      "/libraries/all/series?tf=any:Favorite",
    );
  });

  it("should not render as links when clickable is false", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <GenreTagChips genres={mockGenres} tags={mockTags} clickable={false} />,
    );

    // Badges should not be links
    const actionBadge = screen.getByText("Action");
    expect(actionBadge.closest("a")).not.toBeInTheDocument();

    const favoriteBadge = screen.getByText("Favorite");
    expect(favoriteBadge.closest("a")).not.toBeInTheDocument();

    // Clicking should not cause errors (no navigation)
    await user.click(actionBadge);
    await user.click(favoriteBadge);
  });

  it("should limit display items with maxDisplay", () => {
    renderWithProviders(
      <GenreTagChips genres={mockGenres} tags={mockTags} maxDisplay={3} />,
    );

    // Should show first 3 items (all genres in this case)
    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    expect(screen.getByText("Comedy")).toBeInTheDocument();

    // Tags should be hidden
    expect(screen.queryByText("Favorite")).not.toBeInTheDocument();

    // Should show "+2 more"
    expect(screen.getByText("+2 more")).toBeInTheDocument();
  });

  it("should encode special characters in genre name for URL", () => {
    const genresWithSpecialChars = [
      {
        id: "genre-1",
        name: "Sci-Fi & Fantasy",
        seriesCount: 5,
        createdAt: "2024-01-01T00:00:00Z",
      },
    ];

    renderWithProviders(
      <GenreTagChips genres={genresWithSpecialChars} clickable={true} />,
    );

    const link = screen.getByText("Sci-Fi & Fantasy").closest("a");
    expect(link).toHaveAttribute(
      "href",
      "/libraries/all/series?gf=any:Sci-Fi%20%26%20Fantasy",
    );
  });

  it("should show tags when genres don't fill maxDisplay", () => {
    const singleGenre = [
      {
        id: "genre-1",
        name: "Action",
        seriesCount: 10,
        createdAt: "2024-01-01T00:00:00Z",
      },
    ];

    renderWithProviders(
      <GenreTagChips genres={singleGenre} tags={mockTags} maxDisplay={3} />,
    );

    // Should show 1 genre and 2 tags
    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Favorite")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("should not show '+X more' when all items are displayed", () => {
    const singleGenre = [
      {
        id: "genre-1",
        name: "Action",
        seriesCount: 10,
        createdAt: "2024-01-01T00:00:00Z",
      },
    ];
    const singleTag = [
      {
        id: "tag-1",
        name: "Favorite",
        seriesCount: 15,
        createdAt: "2024-01-01T00:00:00Z",
      },
    ];

    renderWithProviders(
      <GenreTagChips genres={singleGenre} tags={singleTag} maxDisplay={5} />,
    );

    expect(screen.queryByText(/more/)).not.toBeInTheDocument();
  });

  it("should render badges as proper links when clickable", () => {
    renderWithProviders(<GenreTagChips genres={mockGenres} clickable={true} />);

    // Verify badges are rendered as links
    const adventureLink = screen.getByText("Adventure").closest("a");
    expect(adventureLink).toHaveAttribute(
      "href",
      "/libraries/all/series?gf=any:Adventure",
    );
  });

  it("should expand collapsed items when clicking '+X more'", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <GenreTagChips genres={mockGenres} tags={mockTags} maxDisplay={3} />,
    );

    // Initially collapsed: tags should be hidden
    expect(screen.queryByText("Favorite")).not.toBeInTheDocument();
    expect(screen.getByText("+2 more")).toBeInTheDocument();

    // Click to expand
    await user.click(screen.getByText("+2 more"));

    // All items should now be visible
    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    expect(screen.getByText("Comedy")).toBeInTheDocument();
    expect(screen.getByText("Favorite")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();

    // "+2 more" should be gone, "Show less" should appear
    expect(screen.queryByText("+2 more")).not.toBeInTheDocument();
    expect(screen.getByText("Show less")).toBeInTheDocument();
  });

  it("should collapse expanded items when clicking 'Show less'", async () => {
    const user = userEvent.setup();

    renderWithProviders(
      <GenreTagChips genres={mockGenres} tags={mockTags} maxDisplay={3} />,
    );

    // Expand first
    await user.click(screen.getByText("+2 more"));
    expect(screen.getByText("Favorite")).toBeInTheDocument();

    // Click to collapse
    await user.click(screen.getByText("Show less"));

    // Tags should be hidden again
    expect(screen.queryByText("Favorite")).not.toBeInTheDocument();
    expect(screen.getByText("+2 more")).toBeInTheDocument();
  });

  describe("generic groups", () => {
    const mockSubjects = [
      { id: "s1", name: "Mathematics" },
      { id: "s2", name: "Physics" },
      { id: "s3", name: "Chemistry" },
      { id: "s4", name: "Biology" },
      { id: "s5", name: "History" },
      { id: "s6", name: "Geography" },
      { id: "s7", name: "Literature" },
    ];

    it("should render generic groups with custom color", () => {
      renderWithProviders(
        <GenreTagChips
          groups={[{ items: mockSubjects.slice(0, 3), color: "teal" }]}
        />,
      );

      expect(screen.getByText("Mathematics")).toBeInTheDocument();
      expect(screen.getByText("Physics")).toBeInTheDocument();
      expect(screen.getByText("Chemistry")).toBeInTheDocument();
    });

    it("should render nothing when groups have no items", () => {
      renderWithProviders(
        <GenreTagChips groups={[{ items: [], color: "teal" }]} />,
      );

      expect(screen.queryByRole("group")).not.toBeInTheDocument();
    });

    it("should not render generic groups as links", () => {
      renderWithProviders(
        <GenreTagChips
          groups={[{ items: mockSubjects.slice(0, 2), color: "teal" }]}
        />,
      );

      const badge = screen.getByText("Mathematics");
      expect(badge.closest("a")).not.toBeInTheDocument();
    });

    it("should collapse generic groups with maxDisplay", () => {
      renderWithProviders(
        <GenreTagChips
          groups={[{ items: mockSubjects, color: "teal" }]}
          maxDisplay={5}
        />,
      );

      expect(screen.getByText("Mathematics")).toBeInTheDocument();
      expect(screen.getByText("History")).toBeInTheDocument();
      expect(screen.queryByText("Geography")).not.toBeInTheDocument();
      expect(screen.getByText("+2 more")).toBeInTheDocument();
    });

    it("should expand and collapse generic groups", async () => {
      const user = userEvent.setup();

      renderWithProviders(
        <GenreTagChips
          groups={[{ items: mockSubjects, color: "teal" }]}
          maxDisplay={5}
        />,
      );

      expect(screen.queryByText("Literature")).not.toBeInTheDocument();

      await user.click(screen.getByText("+2 more"));
      expect(screen.getByText("Literature")).toBeInTheDocument();
      expect(screen.getByText("Show less")).toBeInTheDocument();

      await user.click(screen.getByText("Show less"));
      expect(screen.queryByText("Literature")).not.toBeInTheDocument();
    });

    it("should mix genres, tags, and generic groups", () => {
      renderWithProviders(
        <GenreTagChips
          genres={mockGenres.slice(0, 1)}
          tags={mockTags.slice(0, 1)}
          groups={[{ items: mockSubjects.slice(0, 1), color: "teal" }]}
        />,
      );

      expect(screen.getByText("Action")).toBeInTheDocument();
      expect(screen.getByText("Favorite")).toBeInTheDocument();
      expect(screen.getByText("Mathematics")).toBeInTheDocument();
    });

    it("should respect maxDisplay across genres, tags, and groups", () => {
      renderWithProviders(
        <GenreTagChips
          genres={mockGenres}
          tags={mockTags}
          groups={[{ items: mockSubjects.slice(0, 3), color: "teal" }]}
          maxDisplay={4}
        />,
      );

      // 3 genres shown, 1 tag shown = 4 total
      expect(screen.getByText("Action")).toBeInTheDocument();
      expect(screen.getByText("Adventure")).toBeInTheDocument();
      expect(screen.getByText("Comedy")).toBeInTheDocument();
      expect(screen.getByText("Favorite")).toBeInTheDocument();

      // Rest hidden
      expect(screen.queryByText("Completed")).not.toBeInTheDocument();
      expect(screen.queryByText("Mathematics")).not.toBeInTheDocument();

      // 4 hidden: 1 tag + 3 subjects
      expect(screen.getByText("+4 more")).toBeInTheDocument();
    });

    it("should render generic groups with custom getUrl as links", () => {
      renderWithProviders(
        <GenreTagChips
          groups={[
            {
              items: mockSubjects.slice(0, 2),
              color: "teal",
              getUrl: (item) => `/subjects/${item.id}`,
            },
          ]}
        />,
      );

      const link = screen.getByText("Mathematics").closest("a");
      expect(link).toHaveAttribute("href", "/subjects/s1");
    });
  });
});
