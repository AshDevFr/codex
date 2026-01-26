import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { SAMPLE_METADATA_FOR_TEMPLATE } from "@/utils/templateUtils";
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

	describe("metadata section", () => {
		it("should show metadata section by default", () => {
			renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

			expect(screen.getByText("Series Metadata (Mock)")).toBeInTheDocument();
		});

		it("should show metadata hint text", () => {
			renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

			expect(
				screen.getByText(/Available as.*in templates/),
			).toBeInTheDocument();
		});

		it("should hide metadata section when showMetadataSection is false", () => {
			renderWithProviders(
				<TemplateEditor
					value=""
					onChange={() => {}}
					showMetadataSection={false}
				/>,
			);

			expect(
				screen.queryByText("Series Metadata (Mock)"),
			).not.toBeInTheDocument();
		});

		it("should expand metadata section on click", async () => {
			const user = userEvent.setup();
			renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

			// Click to expand the metadata section
			await user.click(screen.getByText("Series Metadata (Mock)"));

			// After expansion, should show the helpful text about metadata usage
			expect(
				screen.getByText(
					/This mock data represents the built-in series metadata/,
				),
			).toBeInTheDocument();
		});

		it("should display sample metadata when expanded", async () => {
			const user = userEvent.setup();
			renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

			// Click to expand the metadata section
			await user.click(screen.getByText("Series Metadata (Mock)"));

			// Should show the metadata root name
			expect(screen.getByText("metadata")).toBeInTheDocument();
		});

		it("should use custom metadataTestData when provided", async () => {
			const customMetadata = {
				...SAMPLE_METADATA_FOR_TEMPLATE,
				title: "Custom Test Title For Metadata",
			};

			const user = userEvent.setup();
			const { container } = renderWithProviders(
				<TemplateEditor
					value=""
					onChange={() => {}}
					metadataTestData={customMetadata}
				/>,
			);

			// Click to expand the metadata section
			await user.click(screen.getByText("Series Metadata (Mock)"));

			// The JsonEditor renders the data - check that the custom title is in the DOM
			expect(container.textContent).toContain("Custom Test Title For Metadata");
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

			// Should show the basic syntax help
			expect(screen.getByText("Basic Syntax")).toBeInTheDocument();
			expect(screen.getByText("Available Helpers")).toBeInTheDocument();
		});

		it("should show metadata fields documentation", async () => {
			const user = userEvent.setup();
			renderWithProviders(<TemplateEditor value="" onChange={() => {}} />);

			// Click to expand the help section
			await user.click(screen.getByText("Template Syntax Help"));

			// Should show the data sources section
			expect(screen.getByText("Available Data Sources")).toBeInTheDocument();
			// Should show the metadata fields section
			expect(screen.getByText("Metadata Fields")).toBeInTheDocument();
		});

		it("should document both custom_metadata and metadata sources", async () => {
			const user = userEvent.setup();
			const { container } = renderWithProviders(
				<TemplateEditor value="" onChange={() => {}} />,
			);

			// Click to expand the help section
			await user.click(screen.getByText("Template Syntax Help"));

			// Should mention both data sources in the text content
			expect(container.textContent).toContain("custom_metadata.*");
			expect(container.textContent).toContain("metadata.*");
		});
	});

	describe("preview with metadata", () => {
		it("should render template with metadata fields", () => {
			renderWithProviders(
				<TemplateEditor value="# {{metadata.title}}" onChange={() => {}} />,
			);

			// The preview should render the template with the sample metadata
			expect(screen.getByText("Attack on Titan")).toBeInTheDocument();
		});

		it("should render template with custom metadata fields", () => {
			renderWithProviders(
				<TemplateEditor
					value="Status: {{custom_metadata.status}}"
					onChange={() => {}}
				/>,
			);

			// The preview should render with the default sample custom metadata
			expect(screen.getByText(/Status:.*In Progress/)).toBeInTheDocument();
		});

		it("should render template combining both metadata sources", () => {
			renderWithProviders(
				<TemplateEditor
					value="**{{metadata.title}}** - {{custom_metadata.status}}"
					onChange={() => {}}
				/>,
			);

			// Should render both the series title and custom status
			expect(screen.getByText("Attack on Titan")).toBeInTheDocument();
			// "In Progress" appears in multiple places (test data section too), so use getAllByText
			expect(screen.getAllByText(/In Progress/).length).toBeGreaterThan(0);
		});

		it("should render metadata genres array", () => {
			renderWithProviders(
				<TemplateEditor
					value='Genres: {{join metadata.genres ", "}}'
					onChange={() => {}}
				/>,
			);

			// Should render the joined genres from sample metadata
			expect(
				screen.getByText(/Genres:.*Action.*Dark Fantasy.*Post-Apocalyptic/),
			).toBeInTheDocument();
		});
	});

	describe("template validation", () => {
		it("should show valid indicator for valid templates", () => {
			renderWithProviders(
				<TemplateEditor
					value="Hello {{custom_metadata.name}}"
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
