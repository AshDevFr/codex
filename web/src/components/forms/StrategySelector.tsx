import {
	Accordion,
	Alert,
	Code,
	Group,
	NumberInput,
	Select,
	Stack,
	Text,
	Textarea,
	TextInput,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import type { BookStrategy, NumberStrategy, SeriesStrategy } from "@/types";

// =============================================================================
// Series Strategy Configuration
// =============================================================================

export interface SeriesStrategyData {
	value: SeriesStrategy;
	label: string;
	description: string;
	example: string;
	hasConfig: boolean;
}

export const SERIES_STRATEGIES: SeriesStrategyData[] = [
	{
		value: "series_volume",
		label: "Series-Volume (Recommended)",
		description:
			"Direct child folders of library = series, files in folders = books. Best for comics and simple folder structures.",
		example: "/library/Batman/Batman #001.cbz → Series: Batman",
		hasConfig: false,
	},
	{
		value: "series_volume_chapter",
		label: "Series-Volume-Chapter",
		description:
			"Parent folder = series, child folders = volumes/arcs, files = chapters. Best for chapter-based manga.",
		example: "/library/One Piece/Volume 01/Chapter 001.cbz → Series: One Piece",
		hasConfig: false,
	},
	{
		value: "flat",
		label: "Flat Structure",
		description:
			"All files at library root, series detected from filename patterns or metadata. Best for automated downloaders.",
		example: "/library/[One Piece] v01.cbz → Series: One Piece",
		hasConfig: true,
	},
	{
		value: "publisher_hierarchy",
		label: "Publisher Hierarchy",
		description:
			"Skip first N levels (publisher/year folders), then apply series-volume rules. Best for collections organized by publisher.",
		example: "/library/Marvel/Spider-Man/Amazing #001.cbz → Series: Spider-Man",
		hasConfig: true,
	},
	{
		value: "calibre",
		label: "Calibre Library",
		description:
			"Author folder → Book title folder → book files. Direct Calibre library compatibility.",
		example:
			"/library/Brandon Sanderson/Mistborn (1)/Mistborn.epub → Book: Mistborn",
		hasConfig: true,
	},
	{
		value: "custom",
		label: "Custom (Advanced)",
		description:
			"User-defined regex patterns for series detection. For unique organizational patterns.",
		example: "Define your own regex pattern to match series names",
		hasConfig: true,
	},
];

// =============================================================================
// Book Strategy Configuration
// =============================================================================

export interface BookStrategyData {
	value: BookStrategy;
	label: string;
	description: string;
}

export const BOOK_STRATEGIES: BookStrategyData[] = [
	{
		value: "filename",
		label: "Filename (Recommended)",
		description:
			"Use filename without extension. Predictable and Komga-compatible.",
	},
	{
		value: "metadata_first",
		label: "Metadata First",
		description:
			"Use ComicInfo/EPUB metadata title if present, fallback to filename.",
	},
	{
		value: "smart",
		label: "Smart Detection",
		description:
			"Use metadata only if meaningful (not generic like 'Vol. 3'), otherwise use filename.",
	},
	{
		value: "series_name",
		label: "Generated Name",
		description:
			"Generate title from series name + position (e.g., 'One Piece v.01').",
	},
];

// =============================================================================
// Flat Strategy Config
// =============================================================================

export interface FlatStrategyConfig {
	filenamePatterns?: string[];
	requireMetadata?: boolean;
}

interface FlatConfigEditorProps {
	config: FlatStrategyConfig;
	onChange: (config: FlatStrategyConfig) => void;
	disabled?: boolean;
}

function FlatConfigEditor({
	config,
	onChange,
	disabled = false,
}: FlatConfigEditorProps) {
	const patternsText = (config.filenamePatterns || []).join("\n");

	return (
		<Stack gap="sm">
			<Textarea
				label="Filename Patterns"
				description="Regex patterns to extract series name from filename (one per line). First capture group = series name."
				placeholder={"\\[([^\\]]+)\\]\n^([^-]+) -\n^([^_]+)_"}
				value={patternsText}
				onChange={(e) =>
					onChange({
						...config,
						filenamePatterns: e.currentTarget.value
							.split("\n")
							.filter((p) => p.trim()),
					})
				}
				minRows={3}
				styles={{ input: { fontFamily: "monospace" } }}
				disabled={disabled}
			/>
			<Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
				<Text size="sm">
					Default patterns: <Code>[Series Name]</Code>,{" "}
					<Code>Series Name -</Code>, <Code>Series_Name</Code>
				</Text>
			</Alert>
		</Stack>
	);
}

// =============================================================================
// Publisher Hierarchy Config
// =============================================================================

export interface PublisherHierarchyConfig {
	skipDepth?: number;
	storeSkippedAs?: string;
}

interface PublisherHierarchyConfigEditorProps {
	config: PublisherHierarchyConfig;
	onChange: (config: PublisherHierarchyConfig) => void;
	disabled?: boolean;
}

function PublisherHierarchyConfigEditor({
	config,
	onChange,
	disabled = false,
}: PublisherHierarchyConfigEditorProps) {
	return (
		<Stack gap="sm">
			<NumberInput
				label="Skip Depth"
				description="Number of folder levels to skip before detecting series"
				placeholder="1"
				value={config.skipDepth ?? 1}
				onChange={(value) =>
					onChange({
						...config,
						skipDepth: typeof value === "number" ? value : 1,
					})
				}
				min={1}
				max={5}
				disabled={disabled}
			/>
			<TextInput
				label="Store Skipped As"
				description="Metadata field to store skipped folder name (e.g., 'publisher')"
				placeholder="publisher"
				value={config.storeSkippedAs || ""}
				onChange={(e) =>
					onChange({
						...config,
						storeSkippedAs: e.currentTarget.value || undefined,
					})
				}
				disabled={disabled}
			/>
		</Stack>
	);
}

// =============================================================================
// Calibre Strategy Config
// =============================================================================

export interface CalibreStrategyConfig {
	stripIdSuffix?: boolean;
	seriesMode?: "standalone" | "by_author" | "from_metadata";
	readOpfMetadata?: boolean;
	authorFromFolder?: boolean;
}

interface CalibreConfigEditorProps {
	config: CalibreStrategyConfig;
	onChange: (config: CalibreStrategyConfig) => void;
	disabled?: boolean;
}

function CalibreConfigEditor({
	config,
	onChange,
	disabled = false,
}: CalibreConfigEditorProps) {
	return (
		<Stack gap="sm">
			<Select
				label="Series Grouping Mode"
				description="How to group books into series"
				data={[
					{
						value: "standalone",
						label: "Standalone - Each book is its own series",
					},
					{
						value: "by_author",
						label: "By Author - Group all books by same author",
					},
					{
						value: "from_metadata",
						label:
							"From Metadata - Use series field from OPF/embedded metadata",
					},
				]}
				value={config.seriesMode || "from_metadata"}
				onChange={(value) =>
					onChange({
						...config,
						seriesMode:
							(value as CalibreStrategyConfig["seriesMode"]) || "from_metadata",
					})
				}
				comboboxProps={{ zIndex: 1001 }}
				disabled={disabled}
			/>
			<Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
				<Text size="sm">
					Calibre folder ID suffixes (e.g., " (123)") are automatically
					stripped. Author names are extracted from folder structure.
				</Text>
			</Alert>
		</Stack>
	);
}

// =============================================================================
// Custom Strategy Config
// =============================================================================

export interface CustomStrategyConfig {
	pattern?: string;
	seriesNameTemplate?: string;
}

interface CustomConfigEditorProps {
	config: CustomStrategyConfig;
	onChange: (config: CustomStrategyConfig) => void;
	disabled?: boolean;
}

function CustomConfigEditor({
	config,
	onChange,
	disabled = false,
}: CustomConfigEditorProps) {
	return (
		<Stack gap="sm">
			<TextInput
				label="Pattern"
				description="Regex pattern matched against relative path from library root. Use named groups."
				placeholder="^(?P<publisher>[^/]+)/(?P<series>[^/]+)/(?P<book>.+)\.(cbz|cbr|epub|pdf)$"
				value={config.pattern || ""}
				onChange={(e) =>
					onChange({
						...config,
						pattern: e.currentTarget.value || undefined,
					})
				}
				styles={{ input: { fontFamily: "monospace" } }}
				disabled={disabled}
			/>
			<TextInput
				label="Series Name Template"
				description="Template to construct series name from captured groups"
				placeholder="{publisher} - {series}"
				value={config.seriesNameTemplate || ""}
				onChange={(e) =>
					onChange({
						...config,
						seriesNameTemplate: e.currentTarget.value || undefined,
					})
				}
				styles={{ input: { fontFamily: "monospace" } }}
				disabled={disabled}
			/>
			<Accordion variant="separated">
				<Accordion.Item value="help">
					<Accordion.Control>Named Groups Reference</Accordion.Control>
					<Accordion.Panel>
						<Stack gap="xs">
							<Text size="sm">
								<Code>(?P&lt;series&gt;...)</Code> - Required: identifies the
								series name
							</Text>
							<Text size="sm">
								<Code>(?P&lt;publisher&gt;...)</Code> - Optional: stored as
								series metadata
							</Text>
							<Text size="sm">
								<Code>(?P&lt;book&gt;...)</Code> - Optional: identifies the book
								filename portion
							</Text>
						</Stack>
					</Accordion.Panel>
				</Accordion.Item>
			</Accordion>
		</Stack>
	);
}

// =============================================================================
// Series Strategy Selector Component
// =============================================================================

export interface SeriesStrategySelectorProps {
	value: SeriesStrategy;
	onChange: (value: SeriesStrategy) => void;
	config: Record<string, unknown>;
	onConfigChange: (config: Record<string, unknown>) => void;
	disabled?: boolean;
}

export function SeriesStrategySelector({
	value,
	onChange,
	config,
	onConfigChange,
	disabled = false,
}: SeriesStrategySelectorProps) {
	const selectedStrategy = SERIES_STRATEGIES.find((s) => s.value === value);

	const renderConfigEditor = () => {
		switch (value) {
			case "flat":
				return (
					<FlatConfigEditor
						config={config as FlatStrategyConfig}
						onChange={onConfigChange}
						disabled={disabled}
					/>
				);
			case "publisher_hierarchy":
				return (
					<PublisherHierarchyConfigEditor
						config={config as PublisherHierarchyConfig}
						onChange={onConfigChange}
						disabled={disabled}
					/>
				);
			case "calibre":
				return (
					<CalibreConfigEditor
						config={config as CalibreStrategyConfig}
						onChange={onConfigChange}
						disabled={disabled}
					/>
				);
			case "custom":
				return (
					<CustomConfigEditor
						config={config as CustomStrategyConfig}
						onChange={onConfigChange}
						disabled={disabled}
					/>
				);
			default:
				return null;
		}
	};

	return (
		<Stack gap="md">
			<Select
				label="Series Detection Strategy"
				description="How series are detected from your folder structure"
				data={SERIES_STRATEGIES.map((s) => ({
					value: s.value,
					label: s.label,
				}))}
				value={value}
				onChange={(v) => {
					if (v) {
						onChange(v as SeriesStrategy);
						// Reset config when strategy changes
						onConfigChange({});
					}
				}}
				disabled={disabled}
				comboboxProps={{ zIndex: 1001 }}
			/>

			{selectedStrategy && (
				<Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
					<Stack gap="xs">
						<Text size="sm">{selectedStrategy.description}</Text>
						<Group gap="xs">
							<Text size="xs" fw={500} c="dimmed">
								Example:
							</Text>
							<Code>{selectedStrategy.example}</Code>
						</Group>
					</Stack>
				</Alert>
			)}

			{selectedStrategy?.hasConfig && renderConfigEditor()}
		</Stack>
	);
}

// =============================================================================
// Book Strategy Selector Component
// =============================================================================

export interface BookStrategySelectorProps {
	value: BookStrategy;
	onChange: (value: BookStrategy) => void;
	disabled?: boolean;
}

export function BookStrategySelector({
	value,
	onChange,
	disabled = false,
}: BookStrategySelectorProps) {
	const selectedStrategy = BOOK_STRATEGIES.find((s) => s.value === value);

	return (
		<Stack gap="md">
			<Select
				label="Book Naming Strategy"
				description="How individual book titles are determined"
				data={BOOK_STRATEGIES.map((s) => ({
					value: s.value,
					label: s.label,
				}))}
				value={value}
				onChange={(v) => {
					if (v) {
						onChange(v as BookStrategy);
					}
				}}
				disabled={disabled}
				comboboxProps={{ zIndex: 1001 }}
			/>

			{selectedStrategy && (
				<Alert icon={<IconInfoCircle size={16} />} color="gray" variant="light">
					<Text size="sm">{selectedStrategy.description}</Text>
				</Alert>
			)}
		</Stack>
	);
}

// =============================================================================
// Number Strategy Configuration
// =============================================================================

export interface NumberStrategyData {
	value: NumberStrategy;
	label: string;
	description: string;
}

export const NUMBER_STRATEGIES: NumberStrategyData[] = [
	{
		value: "file_order",
		label: "File Order (Recommended)",
		description:
			"Book number = position in alphabetically sorted file list. Komga-compatible and works with any naming.",
	},
	{
		value: "metadata",
		label: "Metadata Only",
		description:
			"Use ComicInfo <Number> field only. Files without metadata get no number.",
	},
	{
		value: "filename",
		label: "Filename Patterns",
		description:
			"Parse number from filename patterns (#001, v01, c001, Chapter 001). Ignores metadata.",
	},
	{
		value: "smart",
		label: "Smart Detection",
		description:
			"Fallback chain: metadata → filename patterns → file order. Best coverage with graceful degradation.",
	},
];

// =============================================================================
// Number Strategy Selector Component
// =============================================================================

export interface NumberStrategySelectorProps {
	value: NumberStrategy;
	onChange: (value: NumberStrategy) => void;
	disabled?: boolean;
}

export function NumberStrategySelector({
	value,
	onChange,
	disabled = false,
}: NumberStrategySelectorProps) {
	const selectedStrategy = NUMBER_STRATEGIES.find((s) => s.value === value);

	return (
		<Stack gap="md">
			<Select
				label="Book Number Strategy"
				description="How book numbers (sort order) are determined"
				data={NUMBER_STRATEGIES.map((s) => ({
					value: s.value,
					label: s.label,
				}))}
				value={value}
				onChange={(v) => {
					if (v) {
						onChange(v as NumberStrategy);
					}
				}}
				disabled={disabled}
				comboboxProps={{ zIndex: 1001 }}
			/>

			{selectedStrategy && (
				<Alert icon={<IconInfoCircle size={16} />} color="gray" variant="light">
					<Text size="sm">{selectedStrategy.description}</Text>
				</Alert>
			)}
		</Stack>
	);
}
