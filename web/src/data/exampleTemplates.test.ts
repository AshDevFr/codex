import { describe, expect, it } from "vitest";
import { renderTemplate, validateTemplate } from "@/utils/templateEngine";
import {
	EXAMPLE_TEMPLATES,
	getDefaultTemplate,
	getTemplateById,
	validateAllTemplates,
} from "./exampleTemplates";

describe("exampleTemplates", () => {
	describe("template validation", () => {
		it("should have at least 5 example templates", () => {
			expect(EXAMPLE_TEMPLATES.length).toBeGreaterThanOrEqual(5);
		});

		it("should have unique template IDs", () => {
			const ids = EXAMPLE_TEMPLATES.map((t) => t.id);
			const uniqueIds = new Set(ids);
			expect(uniqueIds.size).toBe(ids.length);
		});

		it("should have all required fields for each template", () => {
			for (const template of EXAMPLE_TEMPLATES) {
				expect(template.id).toBeTruthy();
				expect(template.name).toBeTruthy();
				expect(template.description).toBeTruthy();
				expect(template.template).toBeTruthy();
				expect(template.sampleData).toBeDefined();
				expect(Array.isArray(template.tags)).toBe(true);
				expect(template.tags.length).toBeGreaterThan(0);
			}
		});

		it("should validate all templates successfully", () => {
			const result = validateAllTemplates();
			if (!result.valid) {
				console.error("Template validation errors:", result.errors);
			}
			expect(result.valid).toBe(true);
			expect(result.errors).toHaveLength(0);
		});

		it.each(
			EXAMPLE_TEMPLATES,
		)("template '$name' should have valid syntax", (template) => {
			const result = validateTemplate(template.template);
			expect(result.valid).toBe(true);
		});
	});

	describe("template rendering", () => {
		it.each(
			EXAMPLE_TEMPLATES,
		)("template '$name' should render with its sample data", (template) => {
			const result = renderTemplate(template.template, {
				custom_metadata: template.sampleData,
			});
			expect(result.success).toBe(true);
			expect(result.output).toBeDefined();
		});

		it.each(
			EXAMPLE_TEMPLATES,
		)("template '$name' should handle empty custom_metadata gracefully", (template) => {
			const result = renderTemplate(template.template, {
				custom_metadata: null,
			});
			expect(result.success).toBe(true);
			// Output should be empty or just whitespace when no data
			expect(result.output.trim()).toBe("");
		});

		it.each(
			EXAMPLE_TEMPLATES,
		)("template '$name' should handle empty object gracefully", (template) => {
			const result = renderTemplate(template.template, {
				custom_metadata: {},
			});
			expect(result.success).toBe(true);
			// Output should render without errors - may have headers for some templates
			// since Handlebars treats empty objects as truthy
			expect(typeof result.output).toBe("string");
		});
	});

	describe("getTemplateById", () => {
		it("should return the template when ID exists", () => {
			const template = getTemplateById("default");
			expect(template).toBeDefined();
			expect(template?.name).toBe("Simple List");
		});

		it("should return undefined when ID does not exist", () => {
			const template = getTemplateById("nonexistent");
			expect(template).toBeUndefined();
		});

		it("should find all templates by their ID", () => {
			for (const template of EXAMPLE_TEMPLATES) {
				const found = getTemplateById(template.id);
				expect(found).toBeDefined();
				expect(found?.id).toBe(template.id);
			}
		});
	});

	describe("getDefaultTemplate", () => {
		it("should return the default template", () => {
			const template = getDefaultTemplate();
			expect(template).toBeDefined();
			expect(template.id).toBe("default");
		});

		it("should have default template in EXAMPLE_TEMPLATES", () => {
			const defaultTemplate = getDefaultTemplate();
			const found = EXAMPLE_TEMPLATES.find((t) => t.id === "default");
			expect(found).toBe(defaultTemplate);
		});
	});

	describe("specific template content", () => {
		it("default template should produce bulleted list", () => {
			const template = getTemplateById("default");
			const result = renderTemplate(template!.template, {
				custom_metadata: { key1: "value1", key2: "value2" },
			});
			expect(result.success).toBe(true);
			expect(result.output).toContain("- **key1**: value1");
			expect(result.output).toContain("- **key2**: value2");
		});

		it("reading-list template should format dates", () => {
			const template = getTemplateById("reading-list");
			const result = renderTemplate(template!.template, {
				custom_metadata: {
					status: "In Progress",
					started_date: "2024-01-15",
				},
			});
			expect(result.success).toBe(true);
			expect(result.output).toContain("In Progress");
			expect(result.output).toContain("Jan 15, 2024");
		});

		it("external-links template should render links as markdown", () => {
			const template = getTemplateById("external-links");
			const result = renderTemplate(template!.template, {
				custom_metadata: {
					links: [{ name: "Test Site", url: "https://example.com" }],
				},
			});
			expect(result.success).toBe(true);
			expect(result.output).toContain("[Test Site](https://example.com)");
		});

		it("personal-notes template should join tags", () => {
			const template = getTemplateById("personal-notes");
			const result = renderTemplate(template!.template, {
				custom_metadata: {
					tags: ["tag1", "tag2", "tag3"],
				},
			});
			expect(result.success).toBe(true);
			expect(result.output).toContain("tag1, tag2, tag3");
		});

		it("minimal template should produce compact output", () => {
			const template = getTemplateById("minimal");
			const result = renderTemplate(template!.template, {
				custom_metadata: {
					status: "Reading",
					rating: 8,
				},
			});
			expect(result.success).toBe(true);
			// Should be a single line (after trimming)
			const lines = result.output
				.trim()
				.split("\n")
				.filter((l) => l.trim());
			expect(lines.length).toBe(1);
		});
	});
});
