import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import {
  SAMPLE_BOOK_CONTEXT,
  SAMPLE_METADATA_FOR_TEMPLATE,
  SAMPLE_SERIES_CONTEXT,
} from "@/utils/templateUtils";
import { TemplateEditor } from "./TemplateEditor";

describe("TemplateEditor", () => {
  describe("rendering", () => {
    it("should render the template editor with default label", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // The label "Template" appears twice - once as the main label and once in the editor section
      expect(screen.getAllByText("Template").length).toBeGreaterThanOrEqual(1);
    });

    it("should render with custom label and description", () => {
      renderWithProviders(
        <TemplateEditor
          value=""
          onChange={() => {}}
          label="Custom Template"
          description="Enter your template here"
        />,
      );

      expect(screen.getByText("Custom Template")).toBeInTheDocument();
      expect(screen.getByText("Enter your template here")).toBeInTheDocument();
    });

    it("should show test data section by default", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      expect(screen.getByText("Test Data")).toBeInTheDocument();
    });

    it("should show live preview section", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      expect(screen.getByText("Live Preview")).toBeInTheDocument();
    });
  });

  describe("context section", () => {
    it("should show series context section by default", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      expect(screen.getByText("Series Context (Mock)")).toBeInTheDocument();
    });

    it("should show context hint text with available variables", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // The hint text includes variable names like seriesId, bookCount, metadata.*, externalIds.*
      // Use getAllByText since seriesId appears in multiple places
      expect(screen.getAllByText(/seriesId/).length).toBeGreaterThan(0);
    });

    it("should expand context section on click", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Click to expand the context section
      await user.click(screen.getByText("Series Context (Mock)"));

      // After expansion, should show the helpful text about context usage
      expect(
        screen.getByText(
          /This mock data represents the series context available in templates/,
        ),
      ).toBeInTheDocument();
    });

    it("should display sample context data when expanded", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Click to expand the context section
      await user.click(screen.getByText("Series Context (Mock)"));

      // Should show context root name
      expect(screen.getByText("context")).toBeInTheDocument();
    });

    it("should use custom initialContext when provided", () => {
      const customContext = {
        ...SAMPLE_SERIES_CONTEXT,
        customMetadata: {
          ...SAMPLE_SERIES_CONTEXT.customMetadata,
          uniqueCustomField: "Custom Test Value Here",
        },
      };

      const { container } = renderWithProviders(
        <TemplateEditor
          value=""
          onChange={() => {}}
          initialContext={customContext}
        />,
      );

      // The Test Data section shows customMetadata which should include our unique field
      // Check that the custom metadata value appears in the tree view
      expect(container.textContent).toContain("Custom Test Value Here");
    });
  });

  describe("context type switching", () => {
    it("should show series/book toggle in context section", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // The segmented control should have both options
      expect(screen.getByText("Series")).toBeInTheDocument();
      expect(screen.getByText("Book")).toBeInTheDocument();
    });

    it("should switch to book context when Book is clicked", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Click the Book option in the segmented control
      await user.click(screen.getByText("Book"));

      // Title should change to "Book Context (Mock)"
      expect(screen.getByText("Book Context (Mock)")).toBeInTheDocument();
    });

    it("should show book-specific hint text when book context is selected", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      await user.click(screen.getByText("Book"));

      // Book context hints should mention series.* for cross-referencing
      expect(screen.getAllByText(/series\.\*/).length).toBeGreaterThan(0);
    });

    it("should show book context helper text when expanded", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Switch to book context
      await user.click(screen.getByText("Book"));
      // Expand the context section
      await user.click(screen.getByText("Book Context (Mock)"));

      expect(
        screen.getByText(
          /This mock data represents the book context available in templates/,
        ),
      ).toBeInTheDocument();
    });

    it("should render preview with book context data", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <TemplateEditor value="# {{metadata.title}}" onChange={() => {}} />,
      );

      // Switch to book context
      await user.click(screen.getByText("Book"));

      // Preview should now render with SAMPLE_BOOK_CONTEXT title ("The Martian")
      expect(screen.getByText("The Martian")).toBeInTheDocument();
    });

    it("should switch back to series context", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <TemplateEditor value="# {{metadata.title}}" onChange={() => {}} />,
      );

      // Switch to book, then back to series
      await user.click(screen.getByText("Book"));
      expect(screen.getByText("The Martian")).toBeInTheDocument();

      await user.click(screen.getByText("Series"));
      expect(screen.getByText("One Piece")).toBeInTheDocument();
    });

    it("should preserve customMetadata when switching context types", async () => {
      const user = userEvent.setup();
      const { container } = renderWithProviders(
        <TemplateEditor
          value="{{customMetadata.myField}}"
          onChange={() => {}}
        />,
      );

      // Default series context has customMetadata.myField = "preserved as-is"
      expect(container.textContent).toContain("preserved as-is");

      // Switch to book — customMetadata should carry over
      await user.click(screen.getByText("Book"));
      expect(container.textContent).toContain("preserved as-is");
    });

    it("should start in book mode when initialContext is a book context", () => {
      renderWithProviders(
        <TemplateEditor
          value="# {{metadata.title}}"
          onChange={() => {}}
          initialContext={SAMPLE_BOOK_CONTEXT}
        />,
      );

      expect(screen.getByText("Book Context (Mock)")).toBeInTheDocument();
      expect(screen.getByText("The Martian")).toBeInTheDocument();
    });
  });

  describe("help section", () => {
    it("should show template syntax help section", () => {
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      expect(screen.getByText("Template Syntax Help")).toBeInTheDocument();
    });

    it("should expand help section on click", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Click to expand the help section
      await user.click(screen.getByText("Template Syntax Help"));

      // Should show the basic syntax help and helpers section
      expect(screen.getByText("Basic Syntax")).toBeInTheDocument();
      expect(screen.getByText(/Available Helpers/)).toBeInTheDocument();
    });

    it("should show helper example when a helper pill is clicked", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Expand help section
      await user.click(screen.getByText("Template Syntax Help"));

      // Click the "oneOf" helper pill
      await user.click(screen.getByText("oneOf"));

      // Should show the oneOf description and example
      expect(
        screen.getByText("Check if a value matches any of the given options"),
      ).toBeInTheDocument();
    });

    it("should toggle helper example off when clicked again", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      await user.click(screen.getByText("Template Syntax Help"));
      await user.click(screen.getByText("join"));

      // Example should be visible
      expect(
        screen.getByText("Join array items with a separator"),
      ).toBeInTheDocument();

      // Click again to close
      await user.click(screen.getByText("join"));

      expect(
        screen.queryByText("Join array items with a separator"),
      ).not.toBeInTheDocument();
    });

    it("should show metadata fields documentation", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      // Click to expand the help section
      await user.click(screen.getByText("Template Syntax Help"));

      // Should show the available variables section
      expect(screen.getByText("Available Variables")).toBeInTheDocument();
      // Should show both series and book metadata field sections
      expect(screen.getByText("Series Metadata Fields")).toBeInTheDocument();
      expect(screen.getByText("Book Metadata Fields")).toBeInTheDocument();
    });

    it("should show type-aware template documentation", async () => {
      const user = userEvent.setup();
      renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

      await user.click(screen.getByText("Template Syntax Help"));

      expect(screen.getByText("Type-Aware Templates")).toBeInTheDocument();
    });

    it("should document both customMetadata and metadata sources", async () => {
      const user = userEvent.setup();
      const { container } = renderWithProviders(
        <TemplateEditor value="" onChange={() => {}} />,
      );

      // Click to expand the help section
      await user.click(screen.getByText("Template Syntax Help"));

      // Should mention both data sources in the text content
      expect(container.textContent).toContain("customMetadata.*");
      expect(container.textContent).toContain("metadata.*");
    });
  });

  describe("preview with metadata", () => {
    it("should render template with metadata fields", () => {
      renderWithProviders(
        <TemplateEditor value="# {{metadata.title}}" onChange={() => {}} />,
      );

      // The preview should render the template with the sample metadata
      // SAMPLE_SERIES_CONTEXT uses "One Piece" as the title
      expect(screen.getByText("One Piece")).toBeInTheDocument();
    });

    it("should render template with custom metadata fields", () => {
      renderWithProviders(
        <TemplateEditor
          value="MyField: {{customMetadata.myField}}"
          onChange={() => {}}
        />,
      );

      // The preview should render with the default sample custom metadata
      // SAMPLE_SERIES_CONTEXT.customMetadata.myField = "preserved as-is"
      expect(screen.getByText(/MyField:.*preserved as-is/)).toBeInTheDocument();
    });

    it("should render template combining both metadata sources", () => {
      renderWithProviders(
        <TemplateEditor
          value="**{{metadata.title}}** - {{customMetadata.myField}}"
          onChange={() => {}}
        />,
      );

      // Should render both the series title and custom metadata
      expect(screen.getByText("One Piece")).toBeInTheDocument();
      // "preserved as-is" appears in multiple places (test data section too), so use getAllByText
      expect(screen.getAllByText(/preserved as-is/).length).toBeGreaterThan(0);
    });

    it("should render metadata genres array", () => {
      renderWithProviders(
        <TemplateEditor
          value='Genres: {{join metadata.genres ", "}}'
          onChange={() => {}}
        />,
      );

      // Should render the joined genres from sample metadata
      // SAMPLE_SERIES_CONTEXT.metadata.genres = ["Action", "Adventure", "Comedy", "Fantasy"]
      expect(
        screen.getByText(/Genres:.*Action.*Adventure.*Comedy.*Fantasy/),
      ).toBeInTheDocument();
    });
  });

  describe("template validation", () => {
    it("should show valid indicator for valid templates", () => {
      renderWithProviders(
        <TemplateEditor
          value="Hello {{customMetadata.myField}}"
          onChange={() => {}}
        />,
      );

      expect(screen.getByText("Valid")).toBeInTheDocument();
    });

    it("should show valid indicator for empty templates", () => {
      const { container } = renderWithProviders(
        <TemplateEditor value="" onChange={() => {}} />,
      );

      // Empty template is valid but shouldn't show the "Valid" indicator
      // because there's no content to validate
      const validIndicator = container.querySelector(
        '[class*="Alert"][class*="red"]',
      );
      expect(validIndicator).toBeNull();
    });

    it("should show empty output message when template renders to nothing", () => {
      renderWithProviders(
        <TemplateEditor
          value="{{#if missing}}content{{/if}}"
          onChange={() => {}}
        />,
      );

      expect(
        screen.getByText("Template rendered but produced empty output"),
      ).toBeInTheDocument();
    });

    it("should show validation error for invalid syntax", () => {
      renderWithProviders(
        <TemplateEditor
          value={"{{#if condition}}content{{/if}"}
          onChange={() => {}}
        />,
      );

      // Malformed closing tag causes validation failure
      expect(
        screen.getByText("Fix template errors to see preview"),
      ).toBeInTheDocument();
    });
  });

  describe("onChange callback", () => {
    it("should call onChange when template value changes", async () => {
      const onChange = vi.fn();
      const user = userEvent.setup();

      renderWithProviders(<TemplateEditor value="" onChange={onChange} />);

      // Find the textarea and type in it
      const editor = document.querySelector(".template-editor-textarea");
      expect(editor).not.toBeNull();

      if (editor) {
        await user.click(editor);
        await user.type(editor, "test");

        // onChange should have been called
        expect(onChange).toHaveBeenCalled();
      }
    });
  });
});

describe("SAMPLE_METADATA_FOR_TEMPLATE", () => {
  it("should have all required fields", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.title).toBe("Attack on Titan");
    expect(SAMPLE_METADATA_FOR_TEMPLATE.summary).toBeTruthy();
    expect(SAMPLE_METADATA_FOR_TEMPLATE.publisher).toBe("Kodansha");
    expect(SAMPLE_METADATA_FOR_TEMPLATE.year).toBe(2009);
    expect(SAMPLE_METADATA_FOR_TEMPLATE.status).toBe("ended");
  });

  it("should have sample genres array", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.genres).toContain("Action");
    expect(SAMPLE_METADATA_FOR_TEMPLATE.genres).toContain("Dark Fantasy");
    expect(SAMPLE_METADATA_FOR_TEMPLATE.genres.length).toBeGreaterThan(0);
  });

  it("should have sample tags array", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.tags).toContain("manga");
    expect(SAMPLE_METADATA_FOR_TEMPLATE.tags.length).toBeGreaterThan(0);
  });

  it("should have sample external ratings", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.externalRatings.length).toBeGreaterThan(
      0,
    );
    const firstRating = SAMPLE_METADATA_FOR_TEMPLATE.externalRatings[0];
    expect(firstRating.source).toBe("MyAnimeList");
    expect(firstRating.rating).toBe(8.54);
    expect(firstRating.votes).toBe(1250000);
  });

  it("should have sample external links", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.externalLinks.length).toBeGreaterThan(
      0,
    );
    const firstLink = SAMPLE_METADATA_FOR_TEMPLATE.externalLinks[0];
    expect(firstLink.source).toBe("MyAnimeList");
    expect(firstLink.url).toContain("myanimelist.net");
  });

  it("should have sample alternate titles", () => {
    expect(SAMPLE_METADATA_FOR_TEMPLATE.alternateTitles.length).toBeGreaterThan(
      0,
    );
    const romaji = SAMPLE_METADATA_FOR_TEMPLATE.alternateTitles.find(
      (t) => t.label === "Romaji",
    );
    expect(romaji?.title).toBe("Shingeki no Kyojin");
  });
});
