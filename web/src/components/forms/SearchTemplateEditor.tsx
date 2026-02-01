import {
	Alert,
	Button,
	Card,
	Checkbox,
	Code,
	Divider,
	Group,
	Menu,
	Paper,
	Stack,
	Text,
	Textarea,
} from "@mantine/core";
import {
	IconAlertCircle,
	IconChevronDown,
	IconInfoCircle,
} from "@tabler/icons-react";
import { useCallback, useMemo, useState } from "react";

export interface SearchTemplateEditorProps {
	/** Current template string (Handlebars) */
	value: string;
	/** Callback when template changes */
	onChange: (template: string) => void;
	/** Whether to skip search when external ID exists */
	useExistingExternalId?: boolean;
	/** Callback when useExistingExternalId changes */
	onUseExistingExternalIdChange?: (value: boolean) => void;
	/** Whether the editor is disabled */
	disabled?: boolean;
	/** Label for the component */
	label?: string;
	/** Description for the component */
	description?: string;
}

/**
 * Common template examples.
 */
const TEMPLATE_EXAMPLES: {
	name: string;
	template: string;
	description: string;
}[] = [
	{
		name: "Basic (title only)",
		template: "{{title}}",
		description: "Just the series title",
	},
	{
		name: "Cleaned title",
		template: "{{clean title}}",
		description: "Title with common noise removed",
	},
	{
		name: "Title with year",
		template: "{{title}} {{year}}",
		description: "Include year for better matching",
	},
	{
		name: "Title + publisher",
		template: "{{title}} {{#if publisher}}site:{{publisher}}{{/if}}",
		description: "Add publisher as search filter",
	},
	{
		name: "Truncated title",
		template: "{{truncate title 50}}",
		description: "Limit title to 50 characters",
	},
	{
		name: "First word only",
		template: "{{first_word title}}",
		description: "Use only the first word of the title",
	},
	{
		name: "Normalize CJK",
		template: "{{normalize_cjk title}}",
		description: "Normalize CJK characters for better search",
	},
	{
		name: "Remove parentheses",
		template: "{{remove_parentheses title}}",
		description: "Remove all parenthetical content",
	},
];

/**
 * Available template helpers.
 */
const TEMPLATE_HELPERS: {
	name: string;
	syntax: string;
	description: string;
}[] = [
	{
		name: "clean",
		syntax: "{{clean field}}",
		description: "Remove common noise patterns (Digital, Complete, year, etc.)",
	},
	{
		name: "truncate",
		syntax: "{{truncate field length}}",
		description: "Truncate text to specified length",
	},
	{
		name: "first_word",
		syntax: "{{first_word field}}",
		description: "Extract only the first word",
	},
	{
		name: "normalize_cjk",
		syntax: "{{normalize_cjk field}}",
		description: "Normalize CJK (Chinese, Japanese, Korean) characters",
	},
	{
		name: "remove_parentheses",
		syntax: "{{remove_parentheses field}}",
		description: "Remove all parenthetical content",
	},
	{
		name: "lowercase",
		syntax: "{{lowercase field}}",
		description: "Convert text to lowercase",
	},
	{
		name: "uppercase",
		syntax: "{{uppercase field}}",
		description: "Convert text to uppercase",
	},
	{
		name: "if",
		syntax: "{{#if field}}...{{/if}}",
		description: "Conditional inclusion",
	},
	{
		name: "unless",
		syntax: "{{#unless field}}...{{/unless}}",
		description: "Conditional exclusion (opposite of if)",
	},
];

/**
 * Available context fields.
 *
 * Field paths use camelCase to match backend SeriesContext JSON output.
 * Top-level fields: seriesId, bookCount, metadata, externalIds, customMetadata
 */
const CONTEXT_FIELDS: { name: string; description: string }[] = [
	// Top-level fields
	{ name: "bookCount", description: "Number of books in series" },
	// Metadata fields (accessed via metadata.*)
	{ name: "metadata.title", description: "Series title" },
	{ name: "metadata.titleSort", description: "Title for sorting" },
	{ name: "metadata.year", description: "Publication year" },
	{ name: "metadata.publisher", description: "Publisher name" },
	{ name: "metadata.status", description: "Publication status" },
	{ name: "metadata.language", description: "Primary language" },
	{ name: "metadata.genres", description: "Genre names (array)" },
	{ name: "metadata.tags", description: "Tag names (array)" },
];

/**
 * Simple template validation (checks for balanced braces).
 */
function validateTemplate(template: string): string | null {
	if (!template.trim()) return null; // Empty is valid (uses default)

	// Check for balanced {{ }}
	const openCount = (template.match(/\{\{/g) || []).length;
	const closeCount = (template.match(/\}\}/g) || []).length;
	if (openCount !== closeCount) {
		return "Unbalanced template braces ({{ and }})";
	}

	// Check for balanced #if/#unless with /if//unless
	const ifOpens = (template.match(/\{\{#if\s/g) || []).length;
	const ifCloses = (template.match(/\{\{\/if\}\}/g) || []).length;
	if (ifOpens !== ifCloses) {
		return "Unbalanced {{#if}} and {{/if}} blocks";
	}

	const unlessOpens = (template.match(/\{\{#unless\s/g) || []).length;
	const unlessCloses = (template.match(/\{\{\/unless\}\}/g) || []).length;
	if (unlessOpens !== unlessCloses) {
		return "Unbalanced {{#unless}} and {{/unless}} blocks";
	}

	return null;
}

/**
 * Render a simple preview of the template with sample data.
 *
 * Uses the unified SeriesContext structure with camelCase field names.
 */
function renderPreview(template: string): string {
	if (!template.trim()) return "(default: series title)";

	// Sample data matching backend SeriesContext structure
	const sampleData = {
		bookCount: 105,
		metadata: {
			title: "One Piece (Digital)",
			titleSort: "One Piece",
			year: 1997,
			publisher: "Shueisha",
			status: "ongoing",
			language: "ja",
			genres: ["Action", "Adventure", "Comedy"],
			tags: ["pirates", "treasure", "manga"],
		},
	};

	// Very simple preview - just show the template with sample data highlighted
	let preview = template;

	// Replace top-level field references
	preview = preview.replace(/\{\{bookCount\}\}/g, String(sampleData.bookCount));

	// Replace metadata field references (metadata.*)
	for (const [key, value] of Object.entries(sampleData.metadata)) {
		const displayValue = Array.isArray(value)
			? value.join(", ")
			: String(value);
		preview = preview.replace(
			new RegExp(`\\{\\{metadata\\.${key}\\}\\}`, "g"),
			displayValue,
		);
	}

	// Legacy support: also replace short field names (title, year, etc.)
	// These map to metadata.* fields for backwards compatibility
	preview = preview.replace(/\{\{title\}\}/g, sampleData.metadata.title);
	preview = preview.replace(/\{\{year\}\}/g, String(sampleData.metadata.year));
	preview = preview.replace(
		/\{\{publisher\}\}/g,
		sampleData.metadata.publisher,
	);
	preview = preview.replace(/\{\{status\}\}/g, sampleData.metadata.status);
	preview = preview.replace(/\{\{language\}\}/g, sampleData.metadata.language);

	// Simplify helper calls for preview
	preview = preview.replace(/\{\{clean title\}\}/g, "One Piece");
	preview = preview.replace(/\{\{clean metadata\.title\}\}/g, "One Piece");
	preview = preview.replace(/\{\{truncate title \d+\}\}/g, "One Piece (D...");
	preview = preview.replace(
		/\{\{truncate metadata\.title \d+\}\}/g,
		"One Piece (D...",
	);
	preview = preview.replace(/\{\{first_word title\}\}/g, "One");
	preview = preview.replace(/\{\{first_word metadata\.title\}\}/g, "One");
	preview = preview.replace(
		/\{\{normalize_cjk title\}\}/g,
		"One Piece (Digital)",
	);
	preview = preview.replace(
		/\{\{normalize_cjk metadata\.title\}\}/g,
		"One Piece (Digital)",
	);
	preview = preview.replace(/\{\{remove_parentheses title\}\}/g, "One Piece");
	preview = preview.replace(
		/\{\{remove_parentheses metadata\.title\}\}/g,
		"One Piece",
	);
	preview = preview.replace(/\{\{lowercase title\}\}/g, "one piece (digital)");
	preview = preview.replace(
		/\{\{lowercase metadata\.title\}\}/g,
		"one piece (digital)",
	);
	preview = preview.replace(/\{\{uppercase title\}\}/g, "ONE PIECE (DIGITAL)");
	preview = preview.replace(
		/\{\{uppercase metadata\.title\}\}/g,
		"ONE PIECE (DIGITAL)",
	);

	// Handle conditionals (simplified)
	preview = preview.replace(/\{\{#if [\w.]+\}\}(.*?)\{\{\/if\}\}/g, "$1");
	preview = preview.replace(/\{\{#unless [\w.]+\}\}(.*?)\{\{\/unless\}\}/g, "");

	return preview || "(empty)";
}

/**
 * Editor for search query templates with Handlebars syntax.
 */
export function SearchTemplateEditor({
	value,
	onChange,
	useExistingExternalId,
	onUseExistingExternalIdChange,
	disabled = false,
	label = "Search Query Template",
	description = "Customize how series titles are transformed into search queries using Handlebars syntax.",
}: SearchTemplateEditorProps) {
	const [showHelp, setShowHelp] = useState(false);

	const error = useMemo(() => validateTemplate(value), [value]);
	const preview = useMemo(
		() => (error ? null : renderPreview(value)),
		[value, error],
	);

	const insertTemplate = useCallback(
		(template: string) => {
			onChange(template);
		},
		[onChange],
	);

	return (
		<Stack gap="md">
			{/* Header */}
			<Group justify="space-between" align="flex-start">
				<div>
					<Text fw={500} size="sm">
						{label}
					</Text>
					<Text size="xs" c="dimmed">
						{description}
					</Text>
				</div>
				<Group gap="xs">
					<Button
						size="xs"
						variant="subtle"
						onClick={() => setShowHelp(!showHelp)}
					>
						{showHelp ? "Hide Help" : "Show Help"}
					</Button>
					<Menu shadow="md" width={250} zIndex={1100}>
						<Menu.Target>
							<Button
								size="xs"
								variant="light"
								rightSection={<IconChevronDown size={14} />}
								disabled={disabled}
							>
								Templates
							</Button>
						</Menu.Target>
						<Menu.Dropdown>
							<Menu.Label>Example Templates</Menu.Label>
							{TEMPLATE_EXAMPLES.map((example) => (
								<Menu.Item
									key={example.name}
									onClick={() => insertTemplate(example.template)}
								>
									<Stack gap={2}>
										<Text size="sm">{example.name}</Text>
										<Text size="xs" c="dimmed">
											{example.description}
										</Text>
									</Stack>
								</Menu.Item>
							))}
							<Menu.Divider />
							<Menu.Item onClick={() => insertTemplate("")}>
								<Text size="sm" c="dimmed">
									Clear (use default)
								</Text>
							</Menu.Item>
						</Menu.Dropdown>
					</Menu>
				</Group>
			</Group>

			{/* Help section */}
			{showHelp && (
				<Card padding="sm" withBorder>
					<Stack gap="sm">
						<Text size="sm" fw={500}>
							Available Fields
						</Text>
						<Group gap="xs" wrap="wrap">
							{CONTEXT_FIELDS.map((field) => (
								<Code
									key={field.name}
									style={{ cursor: "pointer" }}
									onClick={() => {
										if (!disabled) {
											onChange(`${value}{{${field.name}}}`);
										}
									}}
								>
									{`{{${field.name}}}`}
								</Code>
							))}
						</Group>

						<Divider />

						<Text size="sm" fw={500}>
							Available Helpers
						</Text>
						<Stack gap="xs">
							{TEMPLATE_HELPERS.map((helper) => (
								<Group key={helper.name} gap="xs" align="flex-start">
									<Code style={{ flexShrink: 0 }}>{helper.syntax}</Code>
									<Text size="xs" c="dimmed">
										{helper.description}
									</Text>
								</Group>
							))}
						</Stack>
					</Stack>
				</Card>
			)}

			{/* Template input */}
			<Textarea
				placeholder="{{title}} (leave empty to use default)"
				value={value}
				onChange={(e) => onChange(e.currentTarget.value)}
				error={error}
				disabled={disabled}
				minRows={2}
				maxRows={5}
				autosize
				styles={{
					input: { fontFamily: "monospace" },
				}}
			/>

			{/* Preview */}
			{preview && (
				<Paper p="xs" withBorder bg="var(--mantine-color-dark-7)">
					<Group gap="xs">
						<Text size="xs" c="dimmed">
							Preview:
						</Text>
						<Text size="xs" ff="monospace">
							{preview}
						</Text>
					</Group>
				</Paper>
			)}

			{/* Use existing external ID option */}
			{onUseExistingExternalIdChange !== undefined && (
				<Checkbox
					label="Skip search when external ID exists for this plugin"
					description="If a series already has an external ID from this plugin, skip the search and use the existing ID"
					checked={useExistingExternalId ?? true}
					onChange={(e) =>
						onUseExistingExternalIdChange(e.currentTarget.checked)
					}
					disabled={disabled}
				/>
			)}

			{/* Info about preprocessing */}
			<Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
				<Text size="xs">
					The search query template is applied <strong>after</strong>{" "}
					preprocessing rules. If you have preprocessing rules configured at the
					library level, they will be applied first.
				</Text>
			</Alert>

			{/* Validation error */}
			{error && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="light"
					title="Template Error"
				>
					{error}
				</Alert>
			)}
		</Stack>
	);
}
