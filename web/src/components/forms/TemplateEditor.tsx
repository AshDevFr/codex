import {
	Alert,
	Box,
	Button,
	Card,
	Collapse,
	Grid,
	Group,
	SegmentedControl,
	Stack,
	Text,
	useMantineColorScheme,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
	IconAlertCircle,
	IconChevronDown,
	IconChevronRight,
	IconCheck,
	IconCode,
	IconHelp,
	IconLayoutColumns,
	IconLayoutRows,
	IconTree,
} from "@tabler/icons-react";
import {
	JsonEditor,
	githubDarkTheme,
	githubLightTheme,
} from "json-edit-react";
import { useEffect, useMemo, useState } from "react";
import Editor from "react-simple-code-editor";
import Prism from "prismjs";
// Load dependencies in correct order for handlebars syntax highlighting
import "prismjs/components/prism-markup";
import "prismjs/components/prism-markup-templating";
import "prismjs/components/prism-handlebars";
import "prismjs/components/prism-markdown";
import { CustomMetadataDisplay } from "@/components/series";
import {
	getAvailableHelpers,
	validateTemplate,
} from "@/utils/templateEngine";

// Sample data for preview - comprehensive example showcasing various data types
const SAMPLE_METADATA = {
	// Reading tracking
	status: "In Progress",
	priority: 8,
	rating: 9.2,
	started_date: "2024-01-15",
	last_read: "2024-12-20",
	current_volume: 12,
	total_volumes: 25,

	// Personal notes
	notes:
		"One of my all-time favorites! The character development is incredible and the plot twists are unexpected.",
	highlights: [
		"Amazing art style in volume 5",
		"Plot twist in chapter 42 was mind-blowing",
		"Character development for the protagonist",
	],

	// Categorization
	tags: ["action", "adventure", "fantasy", "completed-anime"],
	genres: ["Shonen", "Action", "Adventure"],
	themes: ["Friendship", "Coming of Age", "Good vs Evil"],

	// External links and IDs
	links: [
		{ name: "MyAnimeList", url: "https://myanimelist.net/manga/13" },
		{ name: "AniList", url: "https://anilist.co/manga/30013" },
		{ name: "MangaUpdates", url: "https://mangaupdates.com/series/abc123" },
	],
	external_ids: {
		mal_id: "30013",
		anilist_id: "30013",
		isbn: "978-1-421-50096-9",
		comicvine_id: "4050-12345",
	},

	// Collection info
	collection: {
		format: "Paperback",
		edition: "Viz 3-in-1 Edition",
		condition: "Near Mint",
		location: "Shelf 2, Row 3",
		purchase_date: "2023-06-15",
		purchase_price: 14.99,
		signed: false,
	},

	// Flags
	is_favorite: true,
	is_complete: false,
	has_anime: true,
};

type LayoutMode = "side-by-side" | "stacked";
type TestDataViewMode = "tree" | "json";

export interface TemplateEditorProps {
	/**
	 * The current template value
	 */
	value: string;
	/**
	 * Callback when the template value changes
	 */
	onChange: (value: string) => void;
	/**
	 * Whether the editor is disabled
	 */
	disabled?: boolean;
	/**
	 * Label for the editor
	 */
	label?: string;
	/**
	 * Description text
	 */
	description?: string;
	/**
	 * Initial sample data for preview (defaults to SAMPLE_METADATA)
	 */
	initialSampleData?: Record<string, unknown>;
	/**
	 * Externally controlled test data (when provided, overrides internal state)
	 */
	testData?: Record<string, unknown>;
	/**
	 * Callback when test data changes (for external control)
	 */
	onTestDataChange?: (data: Record<string, unknown>) => void;
}

/**
 * A code editor for Handlebars templates with syntax highlighting and live preview
 */
export function TemplateEditor({
	value,
	onChange,
	disabled = false,
	label = "Template",
	description,
	initialSampleData = SAMPLE_METADATA,
	testData: externalTestData,
	onTestDataChange,
}: TemplateEditorProps) {
	const { colorScheme } = useMantineColorScheme();
	const [helpOpened, { toggle: toggleHelp }] = useDisclosure(false);
	const [layoutMode, setLayoutMode] = useState<LayoutMode>("side-by-side");
	const [testDataViewMode, setTestDataViewMode] =
		useState<TestDataViewMode>("tree");
	const [localValue, setLocalValue] = useState(value);
	const [rawJson, setRawJson] = useState<string>("");
	const [jsonError, setJsonError] = useState<string | null>(null);

	// Test data state - use external if provided, otherwise internal
	const [internalTestData, setInternalTestData] =
		useState<Record<string, unknown>>(initialSampleData);

	// Use external test data if provided, otherwise use internal
	const testData = externalTestData ?? internalTestData;
	const setTestData = (data: Record<string, unknown>) => {
		if (onTestDataChange) {
			onTestDataChange(data);
		} else {
			setInternalTestData(data);
		}
	};

	// Sync local value with prop
	useEffect(() => {
		setLocalValue(value);
	}, [value]);

	// Sync rawJson with testData when switching to JSON view or when testData changes
	useEffect(() => {
		setRawJson(JSON.stringify(testData, null, 2));
		setJsonError(null);
	}, [testData]);

	// Validate template
	const validation = useMemo(() => {
		return validateTemplate(localValue);
	}, [localValue]);

	// Handle value changes
	const handleChange = (newValue: string) => {
		setLocalValue(newValue);
		onChange(newValue);
	};

	// Handle test data changes from JsonEditor (tree view)
	const handleTestDataChange = (newData: unknown) => {
		setTestData(newData as Record<string, unknown>);
	};

	// Handle raw JSON changes (JSON view)
	const handleRawJsonChange = (newJson: string) => {
		setRawJson(newJson);
		try {
			const parsed = JSON.parse(newJson) as Record<string, unknown>;
			if (typeof parsed !== "object" || Array.isArray(parsed)) {
				setJsonError("Test data must be a JSON object");
				return;
			}
			setTestData(parsed);
			setJsonError(null);
		} catch (e) {
			setJsonError(e instanceof Error ? e.message : "Invalid JSON");
		}
	};

	// Syntax highlighting function for Prism
	const highlight = (code: string) => {
		return Prism.highlight(code, Prism.languages.handlebars, "handlebars");
	};

	const helpers = getAvailableHelpers();
	const jsonTheme = colorScheme === "dark" ? githubDarkTheme : githubLightTheme;

	// Fixed height for both editors to ensure visual alignment
	const editorHeight = layoutMode === "side-by-side" ? 400 : 300;

	// Template editor component
	const templateEditorSection = (
		<Box>
			<Group justify="space-between" mb="xs">
				<Text fw={500} size="sm">
					Template
				</Text>
				{validation.valid && localValue.trim() && (
					<Group gap="xs">
						<IconCheck size={14} color="var(--mantine-color-green-6)" />
						<Text size="xs" c="green">
							Valid
						</Text>
					</Group>
				)}
			</Group>
			<Box
				style={{
					border:
						colorScheme === "dark"
							? "1px solid var(--mantine-color-dark-4)"
							: "1px solid var(--mantine-color-gray-4)",
					borderRadius: "var(--mantine-radius-sm)",
					overflow: "auto",
					height: editorHeight,
				}}
			>
				<Editor
					value={localValue}
					onValueChange={handleChange}
					highlight={highlight}
					disabled={disabled}
					padding={12}
					style={{
						fontFamily:
							'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
						fontSize: 13,
						lineHeight: 1.5,
						minHeight: editorHeight - 2, // Account for border
						backgroundColor:
							colorScheme === "dark"
								? disabled
									? "var(--mantine-color-dark-7)"
									: "var(--mantine-color-dark-6)"
								: disabled
									? "var(--mantine-color-gray-2)"
									: "var(--mantine-color-gray-0)",
						color: "var(--mantine-color-text)",
					}}
					textareaClassName="template-editor-textarea"
				/>
			</Box>
			{!validation.valid && (
				<Alert icon={<IconAlertCircle size={16} />} color="red" mt="xs">
					{validation.error || "Invalid template syntax"}
				</Alert>
			)}
		</Box>
	);

	// Test data editor component
	const testDataSection = (
		<Box>
			<Group justify="space-between" mb="xs">
				<Text fw={500} size="sm">
					Test Data
				</Text>
				<Group gap="xs">
					<SegmentedControl
						size="xs"
						value={testDataViewMode}
						onChange={(v) => setTestDataViewMode(v as TestDataViewMode)}
						data={[
							{
								value: "tree",
								label: (
									<Group gap={4} wrap="nowrap" align="center">
										<IconTree size={14} style={{ flexShrink: 0 }} />
										<Text size="xs" lh={1}>
											Tree
										</Text>
									</Group>
								),
							},
							{
								value: "json",
								label: (
									<Group gap={4} wrap="nowrap" align="center">
										<IconCode size={14} style={{ flexShrink: 0 }} />
										<Text size="xs" lh={1}>
											JSON
										</Text>
									</Group>
								),
							},
						]}
					/>
					<Button
						variant="subtle"
						size="xs"
						onClick={() => {
							setTestData(initialSampleData);
							// Also reset rawJson directly since the effect may not trigger
							// if testData value doesn't change (e.g., when external state was undefined)
							setRawJson(JSON.stringify(initialSampleData, null, 2));
							setJsonError(null);
						}}
					>
						Reset
					</Button>
				</Group>
			</Group>
			{jsonError && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="light"
					mb="xs"
				>
					{jsonError}
				</Alert>
			)}
			<Box
				style={{
					border:
						colorScheme === "dark"
							? "1px solid var(--mantine-color-dark-4)"
							: "1px solid var(--mantine-color-gray-4)",
					borderRadius: "var(--mantine-radius-sm)",
					overflow: "auto",
					height: editorHeight,
					backgroundColor:
						colorScheme === "dark"
							? "var(--mantine-color-dark-6)"
							: "var(--mantine-color-gray-0)",
				}}
			>
				{testDataViewMode === "tree" ? (
					<JsonEditor
						data={testData}
						setData={handleTestDataChange}
						theme={{
							...jsonTheme,
							styles: {
								...jsonTheme.styles,
								container: {
									...(typeof jsonTheme.styles?.container === "object"
										? jsonTheme.styles.container
										: {}),
									fontSize: 12,
									fontFamily:
										'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
								},
							},
						}}
						rootName="custom_metadata"
						collapse={2}
						enableClipboard={false}
						restrictEdit={false}
						restrictDelete={false}
						restrictAdd={false}
						restrictTypeSelection={false}
					/>
				) : (
					<textarea
						value={rawJson}
						onChange={(e) => handleRawJsonChange(e.target.value)}
						style={{
							width: "100%",
							height: "100%",
							fontFamily:
								'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
							fontSize: "12px",
							lineHeight: 1.5,
							padding: "12px",
							border: "none",
							outline: "none",
							resize: "none",
							backgroundColor: "transparent",
							color: "inherit",
						}}
						placeholder="{}"
						spellCheck={false}
					/>
				)}
			</Box>
			<Text size="xs" c="dimmed" mt="xs">
				{testDataViewMode === "tree"
					? "Click on values to edit them. Use the + button to add new fields."
					: "Edit the raw JSON directly. Changes are validated automatically."}
			</Text>
		</Box>
	);

	// Preview component - uses CustomMetadataDisplay to show exactly how it will render
	const previewSection = (
		<Box>
			<Text fw={500} size="sm" mb="xs">
				Live Preview
			</Text>
			<Box
				style={{
					backgroundColor:
						colorScheme === "dark"
							? "var(--mantine-color-dark-6)"
							: "var(--mantine-color-gray-0)",
					padding: 12,
					borderRadius: "var(--mantine-radius-sm)",
					border:
						colorScheme === "dark"
							? "1px solid var(--mantine-color-dark-4)"
							: "1px solid var(--mantine-color-gray-4)",
					minHeight: 100,
					maxHeight: 300,
					overflow: "auto",
				}}
			>
				{!validation.valid ? (
					<Alert icon={<IconAlertCircle size={16} />} color="orange">
						{validation.error || "Invalid template syntax"}
					</Alert>
				) : (
					<CustomMetadataDisplay
						customMetadata={testData}
						template={localValue}
						showErrors
					/>
				)}
			</Box>
		</Box>
	);

	return (
		<Stack gap="md">
			{/* Label, description, and layout toggle */}
			<Group justify="space-between" align="flex-start">
				{label && (
					<Box>
						<Text fw={500} size="sm" mb={4}>
							{label}
						</Text>
						{description && (
							<Text size="xs" c="dimmed">
								{description}
							</Text>
						)}
					</Box>
				)}
				<SegmentedControl
					size="xs"
					value={layoutMode}
					onChange={(v) => setLayoutMode(v as LayoutMode)}
					data={[
						{
							value: "side-by-side",
							label: (
								<Group gap={4} wrap="nowrap" align="center">
									<IconLayoutColumns size={14} style={{ flexShrink: 0 }} />
									<Text size="xs" lh={1}>Side by Side</Text>
								</Group>
							),
						},
						{
							value: "stacked",
							label: (
								<Group gap={4} wrap="nowrap" align="center">
									<IconLayoutRows size={14} style={{ flexShrink: 0 }} />
									<Text size="xs" lh={1}>Stacked</Text>
								</Group>
							),
						},
					]}
				/>
			</Group>

			{/* Main editor area */}
			{layoutMode === "side-by-side" ? (
				<Stack gap="md">
					<Grid gutter="md">
						<Grid.Col span={6}>{templateEditorSection}</Grid.Col>
						<Grid.Col span={6}>{testDataSection}</Grid.Col>
					</Grid>
					{previewSection}
				</Stack>
			) : (
				<Stack gap="md">
					{templateEditorSection}
					{testDataSection}
					{previewSection}
				</Stack>
			)}

			{/* Help section */}
			<Card withBorder padding="sm">
				<Group
					onClick={toggleHelp}
					style={{ cursor: "pointer" }}
					justify="space-between"
				>
					<Group gap="xs">
						<IconHelp size={16} />
						<Text size="sm" fw={500}>
							Template Syntax Help
						</Text>
					</Group>
					{helpOpened ? (
						<IconChevronDown size={16} />
					) : (
						<IconChevronRight size={16} />
					)}
				</Group>
				<Collapse in={helpOpened}>
					<Stack gap="md" mt="md">
						<Box>
							<Text size="sm" fw={500} mb="xs">
								Basic Syntax
							</Text>
							<Text size="xs" c="dimmed" component="div">
								<ul style={{ margin: 0, paddingLeft: 20 }}>
									<li>
										<code>{"{{field}}"}</code> - Output a field value
									</li>
									<li>
										<code>{"{{custom_metadata.field}}"}</code> - Access nested
										fields
									</li>
									<li>
										<code>{"{{#if field}}...{{/if}}"}</code> - Conditional
										rendering
									</li>
									<li>
										<code>{"{{#each array}}...{{/each}}"}</code> - Loop over
										arrays
									</li>
									<li>
										<code>{"{{@key}}"}</code> - Current key when iterating
										objects
									</li>
									<li>
										<code>{"{{this}}"}</code> - Current value in loops
									</li>
								</ul>
							</Text>
						</Box>

						<Box>
							<Text size="sm" fw={500} mb="xs">
								Available Helpers
							</Text>
							<Group gap="xs">
								{helpers.map((helper) => (
									<Text
										key={helper}
										size="xs"
										style={{
											fontFamily: "monospace",
											backgroundColor:
												colorScheme === "dark"
													? "var(--mantine-color-dark-6)"
													: "var(--mantine-color-gray-2)",
											padding: "2px 6px",
											borderRadius: 4,
										}}
									>
										{helper}
									</Text>
								))}
							</Group>
						</Box>

						<Box>
							<Text size="sm" fw={500} mb="xs">
								Helper Examples
							</Text>
							<Text size="xs" c="dimmed" component="div">
								<ul style={{ margin: 0, paddingLeft: 20 }}>
									<li>
										<code>{'{{formatDate started_date "MMM d, yyyy"}}'}</code> -
										Format dates
									</li>
									<li>
										<code>{'{{truncate notes 50 "..."}}'}</code> - Truncate text
									</li>
									<li>
										<code>{'{{join tags ", "}}'}</code> - Join array items
									</li>
									<li>
										<code>{"{{json custom_metadata}}"}</code> - Output as JSON
									</li>
									<li>
										<code>{"{{#ifEquals status \"active\"}}...{{/ifEquals}}"}</code>{" "}
										- Compare values
									</li>
									<li>
										<code>{'{{default rating "N/A"}}'}</code> - Default for
										missing values
									</li>
								</ul>
							</Text>
						</Box>
					</Stack>
				</Collapse>
			</Card>
		</Stack>
	);
}
