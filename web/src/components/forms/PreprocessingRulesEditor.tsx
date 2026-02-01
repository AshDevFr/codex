import {
	ActionIcon,
	Alert,
	Button,
	Card,
	Checkbox,
	Group,
	Stack,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import {
	IconAlertCircle,
	IconArrowDown,
	IconArrowUp,
	IconCheck,
	IconPlus,
	IconTrash,
	IconX,
} from "@tabler/icons-react";
import { useCallback, useMemo, useState } from "react";

/**
 * A preprocessing rule that transforms text using regex patterns.
 */
export interface PreprocessingRule {
	/** Regex pattern to match */
	pattern: string;
	/** Replacement string (supports $1, $2, etc.) */
	replacement: string;
	/** Optional description for UI display */
	description?: string;
	/** Whether this rule is enabled (default: true) */
	enabled?: boolean;
}

export interface PreprocessingRulesEditorProps {
	/** Current rules array */
	value: PreprocessingRule[];
	/** Callback when rules change */
	onChange: (rules: PreprocessingRule[]) => void;
	/** Optional test input for previewing rule effects */
	testInput?: string;
	/** Callback when test input changes */
	onTestInputChange?: (input: string) => void;
	/** Whether the editor is disabled */
	disabled?: boolean;
	/** Label for the component */
	label?: string;
	/** Description for the component */
	description?: string;
}

/**
 * Validate a regex pattern.
 * Returns null if valid, error message if invalid.
 */
function validatePattern(pattern: string): string | null {
	if (!pattern.trim()) return "Pattern is required";
	try {
		new RegExp(pattern);
		return null;
	} catch (e) {
		return e instanceof Error ? e.message : "Invalid regex pattern";
	}
}

/**
 * Apply a single preprocessing rule to input text.
 */
function applyRule(input: string, rule: PreprocessingRule): string {
	if (!rule.enabled) return input;
	try {
		const regex = new RegExp(rule.pattern, "g");
		return input.replace(regex, rule.replacement);
	} catch {
		return input;
	}
}

/**
 * Apply all preprocessing rules to input text in order.
 */
function applyAllRules(input: string, rules: PreprocessingRule[]): string {
	return rules.reduce((current, rule) => {
		if (rule.enabled !== false) {
			return applyRule(current, rule);
		}
		return current;
	}, input);
}

/**
 * Common preprocessing patterns for quick addition.
 */
const COMMON_PATTERNS: {
	pattern: string;
	replacement: string;
	description: string;
}[] = [
	{
		pattern: "\\s*\\(Digital\\)$",
		replacement: "",
		description: "Remove (Digital) suffix",
	},
	{
		pattern: "\\s*\\(\\d{4}\\)$",
		replacement: "",
		description: "Remove year suffix like (2023)",
	},
	{
		pattern: "\\s*\\(Complete\\)$",
		replacement: "",
		description: "Remove (Complete) suffix",
	},
	{
		pattern: "_",
		replacement: " ",
		description: "Replace underscores with spaces",
	},
];

/**
 * Editor for preprocessing rules with add/remove/reorder functionality,
 * regex validation, and live preview.
 */
export function PreprocessingRulesEditor({
	value,
	onChange,
	testInput = "",
	onTestInputChange,
	disabled = false,
	label = "Preprocessing Rules",
	description = "Transform series titles before metadata search using regex patterns. Rules are applied in order.",
}: PreprocessingRulesEditorProps) {
	const [localTestInput, setLocalTestInput] = useState(testInput);

	// Use local state if no external control
	const effectiveTestInput = onTestInputChange ? testInput : localTestInput;
	const setTestInput = onTestInputChange ?? setLocalTestInput;

	// Validate all patterns
	const patternErrors = useMemo(() => {
		return value.map((rule) => validatePattern(rule.pattern));
	}, [value]);

	const hasErrors = patternErrors.some((e) => e !== null);

	// Preview the result of applying all rules
	const previewResult = useMemo(() => {
		if (!effectiveTestInput.trim()) return "";
		return applyAllRules(effectiveTestInput, value);
	}, [effectiveTestInput, value]);

	const addRule = useCallback(() => {
		onChange([
			...value,
			{ pattern: "", replacement: "", description: "", enabled: true },
		]);
	}, [value, onChange]);

	const addCommonRule = useCallback(
		(common: { pattern: string; replacement: string; description: string }) => {
			onChange([
				...value,
				{
					pattern: common.pattern,
					replacement: common.replacement,
					description: common.description,
					enabled: true,
				},
			]);
		},
		[value, onChange],
	);

	const updateRule = useCallback(
		(index: number, updates: Partial<PreprocessingRule>) => {
			const newRules = [...value];
			newRules[index] = { ...newRules[index], ...updates };
			onChange(newRules);
		},
		[value, onChange],
	);

	const removeRule = useCallback(
		(index: number) => {
			onChange(value.filter((_, i) => i !== index));
		},
		[value, onChange],
	);

	const moveRule = useCallback(
		(index: number, direction: "up" | "down") => {
			const newIndex = direction === "up" ? index - 1 : index + 1;
			if (newIndex < 0 || newIndex >= value.length) return;

			const newRules = [...value];
			[newRules[index], newRules[newIndex]] = [
				newRules[newIndex],
				newRules[index],
			];
			onChange(newRules);
		},
		[value, onChange],
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
				<Button
					size="xs"
					variant="light"
					leftSection={<IconPlus size={14} />}
					onClick={addRule}
					disabled={disabled}
				>
					Add Rule
				</Button>
			</Group>

			{/* Rules list */}
			{value.length === 0 ? (
				<Alert color="gray" variant="light">
					<Stack gap="sm">
						<Text size="sm">No preprocessing rules configured.</Text>
						<Text size="xs" c="dimmed">
							Add rules to clean up series titles before metadata search. Common
							patterns include removing "(Digital)" or year suffixes.
						</Text>
						<Group gap="xs" wrap="wrap">
							{COMMON_PATTERNS.slice(0, 3).map((common) => (
								<Button
									key={common.pattern}
									size="xs"
									variant="subtle"
									onClick={() => addCommonRule(common)}
									disabled={disabled}
								>
									{common.description}
								</Button>
							))}
						</Group>
					</Stack>
				</Alert>
			) : (
				<Stack gap="sm">
					{value.map((rule, index) => (
						<Card key={`${rule.pattern}-${index}`} padding="sm" withBorder>
							<Stack gap="xs">
								{/* Rule header with controls */}
								<Group justify="space-between">
									<Group gap="xs">
										<Checkbox
											checked={rule.enabled !== false}
											onChange={(e) =>
												updateRule(index, { enabled: e.currentTarget.checked })
											}
											disabled={disabled}
											size="xs"
										/>
										<Text size="xs" c="dimmed">
											Rule {index + 1}
										</Text>
									</Group>
									<Group gap={4}>
										<Tooltip label="Move up">
											<ActionIcon
												size="xs"
												variant="subtle"
												onClick={() => moveRule(index, "up")}
												disabled={disabled || index === 0}
											>
												<IconArrowUp size={14} />
											</ActionIcon>
										</Tooltip>
										<Tooltip label="Move down">
											<ActionIcon
												size="xs"
												variant="subtle"
												onClick={() => moveRule(index, "down")}
												disabled={disabled || index === value.length - 1}
											>
												<IconArrowDown size={14} />
											</ActionIcon>
										</Tooltip>
										<Tooltip label="Remove rule">
											<ActionIcon
												size="xs"
												variant="subtle"
												color="red"
												onClick={() => removeRule(index)}
												disabled={disabled}
											>
												<IconTrash size={14} />
											</ActionIcon>
										</Tooltip>
									</Group>
								</Group>

								{/* Pattern input */}
								<TextInput
									size="xs"
									label="Pattern (regex)"
									placeholder="\s*\(Digital\)$"
									value={rule.pattern}
									onChange={(e) =>
										updateRule(index, { pattern: e.currentTarget.value })
									}
									error={patternErrors[index]}
									disabled={disabled}
									styles={{
										input: { fontFamily: "monospace" },
									}}
								/>

								{/* Replacement input */}
								<TextInput
									size="xs"
									label="Replacement"
									placeholder="(empty to remove)"
									value={rule.replacement}
									onChange={(e) =>
										updateRule(index, { replacement: e.currentTarget.value })
									}
									disabled={disabled}
									styles={{
										input: { fontFamily: "monospace" },
									}}
								/>

								{/* Description (optional) */}
								<TextInput
									size="xs"
									label="Description (optional)"
									placeholder="Remove (Digital) suffix"
									value={rule.description ?? ""}
									onChange={(e) =>
										updateRule(index, { description: e.currentTarget.value })
									}
									disabled={disabled}
								/>

								{/* Per-rule preview */}
								{effectiveTestInput.trim() && !patternErrors[index] && (
									<Group gap="xs">
										<Text size="xs" c="dimmed">
											Result:
										</Text>
										<Text size="xs" ff="monospace">
											{applyRule(effectiveTestInput, rule)}
										</Text>
									</Group>
								)}
							</Stack>
						</Card>
					))}
				</Stack>
			)}

			{/* Common patterns quick add */}
			{value.length > 0 && (
				<Group gap="xs">
					<Text size="xs" c="dimmed">
						Quick add:
					</Text>
					{COMMON_PATTERNS.filter(
						(c) => !value.some((r) => r.pattern === c.pattern),
					)
						.slice(0, 3)
						.map((common) => (
							<Button
								key={common.pattern}
								size="compact-xs"
								variant="subtle"
								onClick={() => addCommonRule(common)}
								disabled={disabled}
							>
								{common.description}
							</Button>
						))}
				</Group>
			)}

			{/* Test preview section */}
			<Card padding="sm" withBorder bg="var(--mantine-color-dark-7)">
				<Stack gap="xs">
					<Text size="xs" fw={500}>
						Test Preview
					</Text>
					<TextInput
						size="xs"
						placeholder="Enter a test title to preview..."
						value={effectiveTestInput}
						onChange={(e) => setTestInput(e.currentTarget.value)}
						disabled={disabled}
						leftSection={<Text size="xs">Input:</Text>}
						leftSectionWidth={50}
					/>
					{effectiveTestInput.trim() && (
						<Group gap="xs" align="center">
							<Text size="xs" c="dimmed" w={50}>
								Output:
							</Text>
							{hasErrors ? (
								<Group gap={4}>
									<IconX size={12} color="var(--mantine-color-red-6)" />
									<Text size="xs" ff="monospace" c="red">
										Fix pattern errors first
									</Text>
								</Group>
							) : previewResult === effectiveTestInput ? (
								<Group gap={4}>
									<Text size="xs" ff="monospace">
										{previewResult}
									</Text>
									<Text size="xs" c="dimmed">
										(no change)
									</Text>
								</Group>
							) : (
								<Group gap={4}>
									<IconCheck size={12} color="var(--mantine-color-green-6)" />
									<Text size="xs" ff="monospace" c="green">
										{previewResult}
									</Text>
								</Group>
							)}
						</Group>
					)}
				</Stack>
			</Card>

			{/* Validation errors summary */}
			{hasErrors && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="light"
					title="Invalid patterns"
				>
					Some rules have invalid regex patterns. Fix them before saving.
				</Alert>
			)}
		</Stack>
	);
}
