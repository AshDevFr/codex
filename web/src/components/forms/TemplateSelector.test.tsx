import { waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { TemplateSelector } from "./TemplateSelector";
import { EXAMPLE_TEMPLATES } from "@/data/exampleTemplates";

describe("TemplateSelector", () => {
	it("should render the button to open selector", () => {
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);
		expect(
			screen.getByRole("button", { name: /choose example template/i })
		).toBeInTheDocument();
	});

	it("should open modal when button is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});
		expect(screen.getByText("Example Templates")).toBeInTheDocument();
	});

	it("should display all example templates in the modal", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Check that all template names are displayed
		// Use getAllByText because the preview may contain the same text
		for (const template of EXAMPLE_TEMPLATES) {
			const elements = screen.getAllByText(template.name);
			expect(elements.length).toBeGreaterThan(0);
		}
	});

	it("should call onSelect with the template when Use Template is clicked", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		renderWithProviders(<TemplateSelector onSelect={onSelect} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Click the first template card
		const firstTemplate = EXAMPLE_TEMPLATES[0];
		await user.click(screen.getByText(firstTemplate.name));

		// Click Use Template button
		await user.click(screen.getByRole("button", { name: /use template/i }));

		expect(onSelect).toHaveBeenCalledWith(firstTemplate.template, firstTemplate.sampleData);
	});

	it("should disable Use Template button when no template is selected", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		const useButton = screen.getByRole("button", { name: /use template/i });
		expect(useButton).toBeDisabled();
	});

	it("should close modal when Cancel is clicked", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		await user.click(screen.getByRole("button", { name: /cancel/i }));

		await waitFor(() => {
			expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		});
	});

	it("should close modal after selecting a template", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Click a template card
		await user.click(screen.getByText(EXAMPLE_TEMPLATES[0].name));

		// Click Use Template
		await user.click(screen.getByRole("button", { name: /use template/i }));

		// Modal should be closed
		await waitFor(() => {
			expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		});
	});

	it("should display template descriptions", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Check that template descriptions are displayed
		for (const template of EXAMPLE_TEMPLATES) {
			expect(screen.getByText(template.description)).toBeInTheDocument();
		}
	});

	it("should display template tags", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Check that at least some tags are displayed (first 3 per template)
		const firstTemplate = EXAMPLE_TEMPLATES[0];
		for (const tag of firstTemplate.tags.slice(0, 3)) {
			expect(screen.getByText(tag)).toBeInTheDocument();
		}
	});

	it("should show Current badge when currentTemplate matches", async () => {
		const user = userEvent.setup();
		const currentTemplate = EXAMPLE_TEMPLATES[0].template;
		renderWithProviders(
			<TemplateSelector onSelect={() => {}} currentTemplate={currentTemplate} />
		);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		expect(screen.getByText("Current")).toBeInTheDocument();
	});

	it("should not show Current badge when currentTemplate does not match", async () => {
		const user = userEvent.setup();
		renderWithProviders(
			<TemplateSelector
				onSelect={() => {}}
				currentTemplate="some other template"
			/>
		);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		expect(screen.queryByText("Current")).not.toBeInTheDocument();
	});

	it("should enable Use Template button when template is selected", async () => {
		const user = userEvent.setup();
		renderWithProviders(<TemplateSelector onSelect={() => {}} />);

		await user.click(
			screen.getByRole("button", { name: /choose example template/i })
		);

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeInTheDocument();
		});

		// Click on a template to select it
		await user.click(screen.getByText(EXAMPLE_TEMPLATES[1].name));

		// Use Template button should now be enabled
		await waitFor(() => {
			expect(screen.getByRole("button", { name: /use template/i })).not.toBeDisabled();
		});
	});
});
