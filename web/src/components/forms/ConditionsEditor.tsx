import {
	ActionIcon,
	Alert,
	Badge,
	Button,
	Card,
	Group,
	NumberInput,
	SegmentedControl,
	Select,
	Stack,
	Text,
	Textarea,
	TextInput,
	Tooltip,
} from "@mantine/core";
import {
	IconAlertCircle,
	IconCheck,
	IconCode,
	IconList,
	IconPlus,
	IconTrash,
	IconX,
} from "@tabler/icons-react";
import { useCallback, useMemo, useState } from "react";
import { SAMPLE_SERIES_CONTEXT } from "@/utils/templateUtils";

/**
 * Available condition operators.
 */
export type ConditionOperator =
	| "is_null"
	| "is_not_null"
	| "is_true"
	| "is_false"
	| "equals"
	| "not_equals"
	| "gt"
	| "gte"
	| "lt"
	| "lte"
	| "contains"
	| "not_contains"
	| "starts_with"
	| "ends_with"
	| "matches"
	| "in"
	| "not_in";

/**
 * Condition mode - how to combine multiple rules.
 */
export type ConditionMode = "all" | "any";

/**
 * A single condition rule.
 */
export interface ConditionRule {
	/** Field path to evaluate (e.g., "book_count", "metadata.title") */
	field: string;
	/** Operator to use for comparison */
	operator: ConditionOperator;
	/** Value to compare against (optional for is_null/is_not_null) */
	value?: unknown;
}

/**
 * Auto-match conditions configuration.
 */
export interface AutoMatchConditions {
	/** Mode: "all" (AND) or "any" (OR) */
	mode: ConditionMode;
	/** List of condition rules */
	rules: ConditionRule[];
}

export interface ConditionsEditorProps {
	/** Current conditions configuration */
	value: AutoMatchConditions | null;
	/** Callback when conditions change */
	onChange: (conditions: AutoMatchConditions | null) => void;
	/** Whether the editor is disabled */
	disabled?: boolean;
	/** Label for the component */
	label?: string;
	/** Description for the component */
	description?: string;
	/** Available fields for selection (defaults to common fields) */
	availableFields?: { value: string; label: string; group?: string }[];
	/** Optional test data JSON for previewing condition evaluation */
	testData?: string;
	/** Callback when test data changes */
	onTestDataChange?: (data: string) => void;
}

/**
 * Operator metadata for UI display.
 */
const OPERATORS: {
	value: ConditionOperator;
	label: string;
	requiresValue: boolean;
	valueType: "none" | "string" | "number" | "array";
}[] = [
	{
		value: "is_null",
		label: "Is empty/null",
		requiresValue: false,
		valueType: "none",
	},
	{
		value: "is_not_null",
		label: "Is not empty",
		requiresValue: false,
		valueType: "none",
	},
	{
		value: "is_true",
		label: "Is true",
		requiresValue: false,
		valueType: "none",
	},
	{
		value: "is_false",
		label: "Is false",
		requiresValue: false,
		valueType: "none",
	},
	{
		value: "equals",
		label: "Equals",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "not_equals",
		label: "Not equals",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "gt",
		label: "Greater than",
		requiresValue: true,
		valueType: "number",
	},
	{
		value: "gte",
		label: "Greater or equal",
		requiresValue: true,
		valueType: "number",
	},
	{ value: "lt", label: "Less than", requiresValue: true, valueType: "number" },
	{
		value: "lte",
		label: "Less or equal",
		requiresValue: true,
		valueType: "number",
	},
	{
		value: "contains",
		label: "Contains",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "not_contains",
		label: "Does not contain",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "starts_with",
		label: "Starts with",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "ends_with",
		label: "Ends with",
		requiresValue: true,
		valueType: "string",
	},
	{
		value: "matches",
		label: "Matches regex",
		requiresValue: true,
		valueType: "string",
	},
	{ value: "in", label: "In list", requiresValue: true, valueType: "array" },
	{
		value: "not_in",
		label: "Not in list",
		requiresValue: true,
		valueType: "array",
	},
];

/**
 * Default available fields for condition evaluation.
 *
 * Field paths use camelCase to match backend SeriesContext JSON output.
 * Both camelCase and snake_case paths work for backwards compatibility.
 */
const DEFAULT_FIELDS: { value: string; label: string; group: string }[] = [
	// Series fields (top-level)
	{ value: "bookCount", label: "Book Count", group: "Series" },
	{ value: "externalIds.count", label: "External ID Count", group: "Series" },

	// Metadata fields (accessed via metadata.*)
	{ value: "metadata.title", label: "Title", group: "Metadata" },
	{ value: "metadata.titleSort", label: "Title Sort", group: "Metadata" },
	{ value: "metadata.summary", label: "Summary", group: "Metadata" },
	{ value: "metadata.publisher", label: "Publisher", group: "Metadata" },
	{ value: "metadata.imprint", label: "Imprint", group: "Metadata" },
	{ value: "metadata.status", label: "Status", group: "Metadata" },
	{ value: "metadata.ageRating", label: "Age Rating", group: "Metadata" },
	{ value: "metadata.language", label: "Language", group: "Metadata" },
	{
		value: "metadata.readingDirection",
		label: "Reading Direction",
		group: "Metadata",
	},
	{ value: "metadata.year", label: "Year", group: "Metadata" },
	{
		value: "metadata.totalBookCount",
		label: "Total Book Count",
		group: "Metadata",
	},
	// Array fields
	{ value: "metadata.genres", label: "Genres", group: "Metadata" },
	{ value: "metadata.tags", label: "Tags", group: "Metadata" },

	// Lock fields (camelCase)
	{ value: "metadata.titleLock", label: "Title Lock", group: "Locks" },
	{
		value: "metadata.titleSortLock",
		label: "Title Sort Lock",
		group: "Locks",
	},
	{ value: "metadata.summaryLock", label: "Summary Lock", group: "Locks" },
	{ value: "metadata.publisherLock", label: "Publisher Lock", group: "Locks" },
	{ value: "metadata.imprintLock", label: "Imprint Lock", group: "Locks" },
	{ value: "metadata.statusLock", label: "Status Lock", group: "Locks" },
	{
		value: "metadata.ageRatingLock",
		label: "Age Rating Lock",
		group: "Locks",
	},
	{ value: "metadata.languageLock", label: "Language Lock", group: "Locks" },
	{
		value: "metadata.readingDirectionLock",
		label: "Reading Direction Lock",
		group: "Locks",
	},
	{ value: "metadata.yearLock", label: "Year Lock", group: "Locks" },
	{
		value: "metadata.totalBookCountLock",
		label: "Total Book Count Lock",
		group: "Locks",
	},
	{ value: "metadata.genresLock", label: "Genres Lock", group: "Locks" },
	{ value: "metadata.tagsLock", label: "Tags Lock", group: "Locks" },
	{
		value: "metadata.customMetadataLock",
		label: "Custom Metadata Lock",
		group: "Locks",
	},
];

/**
 * Common condition examples for quick addition.
 *
 * Uses camelCase field paths to match backend SeriesContext.
 */
const COMMON_CONDITIONS: {
	field: string;
	operator: ConditionOperator;
	value?: unknown;
	label: string;
}[] = [
	{
		field: "bookCount",
		operator: "gte",
		value: 1,
		label: "bookCount >= 1",
	},
	{
		field: "metadata.status",
		operator: "is_null",
		label: "status is empty",
	},
	{
		field: "metadata.summaryLock",
		operator: "is_false",
		label: "summary not locked",
	},
	{
		field: "externalIds.count",
		operator: "equals",
		value: 0,
		label: "no external IDs",
	},
	{
		field: "customMetadata.source.name",
		operator: "equals",
		value: "MySource",
		label: "custom nested field",
	},
];

/**
 * Get operator metadata by value.
 */
function getOperatorMeta(op: ConditionOperator) {
	return OPERATORS.find((o) => o.value === op);
}

/**
 * Validate a condition rule.
 */
function validateRule(rule: ConditionRule): string | null {
	if (!rule.field) return "Field is required";

	const opMeta = getOperatorMeta(rule.operator);
	if (!opMeta) return "Invalid operator";

	if (opMeta.requiresValue && (rule.value === undefined || rule.value === "")) {
		return "Value is required for this operator";
	}

	if (rule.operator === "matches" && rule.value) {
		try {
			new RegExp(String(rule.value));
		} catch {
			return "Invalid regex pattern";
		}
	}

	return null;
}

/**
 * Check if a field value exists in the available fields list.
 */
function isKnownField(
	field: string,
	availableFields: { value: string }[],
): boolean {
	return availableFields.some((f) => f.value === field);
}

/**
 * Get a nested value from an object using dot notation.
 * e.g., getNestedValue({ metadata: { title: "Test" } }, "metadata.title") => "Test"
 */
function getNestedValue(obj: unknown, path: string): unknown {
	if (!obj || typeof obj !== "object") return undefined;
	const parts = path.split(".");
	let current: unknown = obj;
	for (const part of parts) {
		if (current === null || current === undefined) return undefined;
		if (typeof current !== "object") return undefined;
		current = (current as Record<string, unknown>)[part];
	}
	return current;
}

/**
 * Evaluate a single condition rule against test data.
 * Returns true if the condition passes, false otherwise.
 */
function evaluateRule(rule: ConditionRule, data: unknown): boolean {
	const fieldValue = getNestedValue(data, rule.field);

	switch (rule.operator) {
		case "is_null":
			return (
				fieldValue === null || fieldValue === undefined || fieldValue === ""
			);
		case "is_not_null":
			return (
				fieldValue !== null && fieldValue !== undefined && fieldValue !== ""
			);
		case "is_true":
			return fieldValue === true;
		case "is_false":
			return fieldValue === false;
		case "equals":
			return String(fieldValue) === String(rule.value);
		case "not_equals":
			return String(fieldValue) !== String(rule.value);
		case "gt":
			return typeof fieldValue === "number" && fieldValue > Number(rule.value);
		case "gte":
			return typeof fieldValue === "number" && fieldValue >= Number(rule.value);
		case "lt":
			return typeof fieldValue === "number" && fieldValue < Number(rule.value);
		case "lte":
			return typeof fieldValue === "number" && fieldValue <= Number(rule.value);
		case "contains":
			return String(fieldValue).includes(String(rule.value));
		case "not_contains":
			return !String(fieldValue).includes(String(rule.value));
		case "starts_with":
			return String(fieldValue).startsWith(String(rule.value));
		case "ends_with":
			return String(fieldValue).endsWith(String(rule.value));
		case "matches":
			try {
				return new RegExp(String(rule.value)).test(String(fieldValue));
			} catch {
				return false;
			}
		case "in":
			return (
				Array.isArray(rule.value) && rule.value.includes(String(fieldValue))
			);
		case "not_in":
			return (
				Array.isArray(rule.value) && !rule.value.includes(String(fieldValue))
			);
		default:
			return false;
	}
}

/**
 * Evaluate all conditions against test data.
 * Returns overall result and individual rule results.
 */
function evaluateConditions(
	conditions: AutoMatchConditions,
	data: unknown,
): { overall: boolean; rules: boolean[] } {
	if (conditions.rules.length === 0) {
		return { overall: true, rules: [] };
	}

	const ruleResults = conditions.rules.map((rule) => {
		if (!rule.field) return false; // Invalid rule
		return evaluateRule(rule, data);
	});

	const overall =
		conditions.mode === "all"
			? ruleResults.every((r) => r)
			: ruleResults.some((r) => r);

	return { overall, rules: ruleResults };
}

/**
 * Editor for auto-match conditions with field/operator/value selection.
 */
export function ConditionsEditor({
	value,
	onChange,
	disabled = false,
	label = "Auto-Match Conditions",
	description = "Define conditions that must be met for auto-matching to run.",
	availableFields = DEFAULT_FIELDS,
	testData,
	onTestDataChange,
}: ConditionsEditorProps) {
	const conditions = value ?? { mode: "all" as ConditionMode, rules: [] };

	// Internal test data state (used when no external control provided)
	const [internalTestData, setInternalTestData] = useState(
		JSON.stringify(SAMPLE_SERIES_CONTEXT, null, 2),
	);
	const effectiveTestData = testData ?? internalTestData;
	const setTestData = onTestDataChange ?? setInternalTestData;

	// Track which rules are in custom (text) mode vs assisted (dropdown) mode
	// Rules with unknown fields default to custom mode
	const [customModeRules, setCustomModeRules] = useState<Set<number>>(() => {
		const initial = new Set<number>();
		conditions.rules.forEach((rule, index) => {
			if (rule.field && !isKnownField(rule.field, availableFields)) {
				initial.add(index);
			}
		});
		return initial;
	});

	// Validate all rules
	const ruleErrors = useMemo(() => {
		return conditions.rules.map((rule) => validateRule(rule));
	}, [conditions.rules]);

	// Parse and evaluate test data
	const testResult = useMemo(() => {
		if (!effectiveTestData.trim() || conditions.rules.length === 0) {
			return null;
		}
		try {
			const parsed = JSON.parse(effectiveTestData);
			return evaluateConditions(conditions, parsed);
		} catch {
			return { error: "Invalid JSON" };
		}
	}, [effectiveTestData, conditions]);

	const hasErrors = ruleErrors.some((e) => e !== null);

	const setMode = useCallback(
		(mode: ConditionMode) => {
			onChange({ ...conditions, mode });
		},
		[conditions, onChange],
	);

	const addRule = useCallback(() => {
		onChange({
			...conditions,
			rules: [
				...conditions.rules,
				{ field: "", operator: "is_not_null" as ConditionOperator },
			],
		});
	}, [conditions, onChange]);

	const updateRule = useCallback(
		(index: number, updates: Partial<ConditionRule>) => {
			const newRules = [...conditions.rules];
			newRules[index] = { ...newRules[index], ...updates };

			// Clear value if operator doesn't require it
			const opMeta = getOperatorMeta(newRules[index].operator);
			if (opMeta && !opMeta.requiresValue) {
				delete newRules[index].value;
			}

			onChange({ ...conditions, rules: newRules });
		},
		[conditions, onChange],
	);

	const removeRule = useCallback(
		(index: number) => {
			const newRules = conditions.rules.filter((_, i) => i !== index);
			// Update custom mode indices
			setCustomModeRules((prev) => {
				const updated = new Set<number>();
				for (const i of prev) {
					if (i < index) updated.add(i);
					else if (i > index) updated.add(i - 1);
				}
				return updated;
			});
			if (newRules.length === 0) {
				onChange(null);
			} else {
				onChange({ ...conditions, rules: newRules });
			}
		},
		[conditions, onChange],
	);

	const clearAll = useCallback(() => {
		setCustomModeRules(new Set());
		onChange(null);
	}, [onChange]);

	const addCommonCondition = useCallback(
		(common: (typeof COMMON_CONDITIONS)[number]) => {
			const newRule: ConditionRule = {
				field: common.field,
				operator: common.operator,
				...(common.value !== undefined && { value: common.value }),
			};
			const newIndex = conditions.rules.length;
			onChange({
				...conditions,
				rules: [...conditions.rules, newRule],
			});
			// Auto-enable custom mode for fields not in the predefined list
			if (!isKnownField(common.field, availableFields)) {
				setCustomModeRules((prev) => new Set([...prev, newIndex]));
			}
		},
		[conditions, onChange, availableFields],
	);

	const toggleRuleMode = useCallback((index: number) => {
		setCustomModeRules((prev) => {
			const updated = new Set(prev);
			if (updated.has(index)) {
				updated.delete(index);
			} else {
				updated.add(index);
			}
			return updated;
		});
	}, []);

	// Group fields by category
	const groupedFields = useMemo(() => {
		const groups = new Map<string, { value: string; label: string }[]>();
		for (const field of availableFields) {
			const group = field.group ?? "Other";
			if (!groups.has(group)) {
				groups.set(group, []);
			}
			groups.get(group)?.push({ value: field.value, label: field.label });
		}
		return Array.from(groups.entries()).map(([group, items]) => ({
			group,
			items,
		}));
	}, [availableFields]);

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
					{conditions.rules.length > 0 && (
						<Button
							size="xs"
							variant="subtle"
							color="red"
							onClick={clearAll}
							disabled={disabled}
						>
							Clear All
						</Button>
					)}
					<Button
						size="xs"
						variant="light"
						leftSection={<IconPlus size={14} />}
						onClick={addRule}
						disabled={disabled}
					>
						Add Condition
					</Button>
				</Group>
			</Group>

			{/* Mode selector */}
			{conditions.rules.length > 1 && (
				<Group gap="xs" align="center">
					<Text size="sm">Match</Text>
					<Select
						size="xs"
						w={100}
						value={conditions.mode}
						onChange={(v) => setMode(v as ConditionMode)}
						data={[
							{ value: "all", label: "ALL" },
							{ value: "any", label: "ANY" },
						]}
						disabled={disabled}
						comboboxProps={{ zIndex: 1001 }}
					/>
					<Text size="sm">of the following conditions:</Text>
				</Group>
			)}

			{/* Rules list */}
			{conditions.rules.length === 0 ? (
				<Alert color="gray" variant="light">
					<Stack gap="sm">
						<Text size="sm">No conditions configured.</Text>
						<Text size="xs" c="dimmed">
							Without conditions, auto-matching will run for all series. Click
							an example to add it, or use "Add Condition" for custom rules.
						</Text>
						<Group gap="xs">
							{COMMON_CONDITIONS.map((common) => (
								<Badge
									key={common.label}
									size="sm"
									variant="light"
									color="blue"
									style={{ cursor: disabled ? "default" : "pointer" }}
									onClick={() => !disabled && addCommonCondition(common)}
								>
									{common.label}
								</Badge>
							))}
						</Group>
					</Stack>
				</Alert>
			) : (
				<Stack gap="sm">
					{conditions.rules.map((rule, index) => {
						const opMeta = getOperatorMeta(rule.operator);
						const error = ruleErrors[index];
						const isCustomMode = customModeRules.has(index);

						return (
							// biome-ignore lint/suspicious/noArrayIndexKey: Rules don't have unique IDs
							<Card key={index} padding="sm" withBorder>
								<Stack gap="xs">
									<Group justify="space-between">
										<Group gap="xs">
											<Text size="xs" c="dimmed">
												Condition {index + 1}
											</Text>
											<SegmentedControl
												size="xs"
												value={isCustomMode ? "custom" : "assisted"}
												onChange={() => toggleRuleMode(index)}
												disabled={disabled}
												data={[
													{
														value: "assisted",
														label: (
															<Tooltip label="Select from dropdown">
																<IconList size={12} />
															</Tooltip>
														),
													},
													{
														value: "custom",
														label: (
															<Tooltip label="Type custom field path">
																<IconCode size={12} />
															</Tooltip>
														),
													},
												]}
											/>
										</Group>
										<Tooltip label="Remove condition">
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

									<Group gap="xs" grow>
										{/* Field selector - assisted or custom mode */}
										{isCustomMode ? (
											<TextInput
												size="xs"
												placeholder="e.g., customMetadata.source.name"
												value={rule.field}
												onChange={(e) =>
													updateRule(index, { field: e.currentTarget.value })
												}
												disabled={disabled}
												error={error && !rule.field}
												styles={{ input: { fontFamily: "monospace" } }}
											/>
										) : (
											<Select
												size="xs"
												placeholder="Select field..."
												value={rule.field}
												onChange={(v) => updateRule(index, { field: v ?? "" })}
												data={groupedFields}
												searchable
												disabled={disabled}
												error={error && !rule.field}
												comboboxProps={{ zIndex: 1001 }}
											/>
										)}

										{/* Operator selector */}
										<Select
											size="xs"
											value={rule.operator}
											onChange={(v) =>
												updateRule(index, { operator: v as ConditionOperator })
											}
											data={OPERATORS.map((op) => ({
												value: op.value,
												label: op.label,
											}))}
											disabled={disabled}
											comboboxProps={{ zIndex: 1001 }}
										/>
									</Group>

									{/* Value input (conditional) */}
									{opMeta?.requiresValue &&
										(opMeta.valueType === "number" ? (
											<NumberInput
												size="xs"
												placeholder="Enter value..."
												value={
													typeof rule.value === "number"
														? rule.value
														: undefined
												}
												onChange={(v) => updateRule(index, { value: v })}
												disabled={disabled}
												error={error}
											/>
										) : opMeta.valueType === "array" ? (
											<TextInput
												size="xs"
												placeholder="value1, value2, value3"
												value={
													Array.isArray(rule.value)
														? rule.value.join(", ")
														: String(rule.value ?? "")
												}
												onChange={(e) =>
													updateRule(index, {
														value: e.currentTarget.value
															.split(",")
															.map((s) => s.trim())
															.filter(Boolean),
													})
												}
												disabled={disabled}
												description="Comma-separated list of values"
												error={error}
											/>
										) : (
											<TextInput
												size="xs"
												placeholder="Enter value..."
												value={String(rule.value ?? "")}
												onChange={(e) =>
													updateRule(index, { value: e.currentTarget.value })
												}
												disabled={disabled}
												error={error}
												styles={
													rule.operator === "matches"
														? { input: { fontFamily: "monospace" } }
														: undefined
												}
											/>
										))}

									{/* Rule preview with test result */}
									<Group gap="xs" align="center">
										<Text size="xs" c="dimmed" ff="monospace">
											{rule.field || "<field>"} {rule.operator}
											{opMeta?.requiresValue && rule.value !== undefined
												? ` "${rule.value}"`
												: ""}
										</Text>
										{testResult && !("error" in testResult) && rule.field && (
											<Tooltip
												label={testResult.rules[index] ? "Passes" : "Fails"}
											>
												{testResult.rules[index] ? (
													<IconCheck
														size={14}
														color="var(--mantine-color-green-6)"
													/>
												) : (
													<IconX size={14} color="var(--mantine-color-red-6)" />
												)}
											</Tooltip>
										)}
									</Group>
								</Stack>
							</Card>
						);
					})}
				</Stack>
			)}

			{/* Test preview section */}
			{conditions.rules.length > 0 && (
				<Card padding="sm" withBorder bg="var(--mantine-color-dark-7)">
					<Stack gap="xs">
						<Group justify="space-between" align="center">
							<Text size="xs" fw={500}>
								Test Preview
							</Text>
							{testResult && !("error" in testResult) && (
								<Badge
									size="sm"
									color={testResult.overall ? "green" : "red"}
									variant="light"
									leftSection={
										testResult.overall ? (
											<IconCheck size={12} />
										) : (
											<IconX size={12} />
										)
									}
								>
									{testResult.overall ? "Would match" : "Would not match"}
								</Badge>
							)}
							{"error" in (testResult ?? {}) && (
								<Badge size="sm" color="red" variant="light">
									Invalid JSON
								</Badge>
							)}
						</Group>
						<Textarea
							size="xs"
							placeholder='{"bookCount": 5, "metadata": {"title": "Test"}, "customMetadata": {"source": {"name": "MySource"}}}'
							value={effectiveTestData}
							onChange={(e) => setTestData(e.currentTarget.value)}
							disabled={disabled}
							rows={12}
							styles={{ input: { fontFamily: "monospace", fontSize: "11px" } }}
							description="Paste sample series data to test conditions (uses camelCase field names)"
						/>
					</Stack>
				</Card>
			)}

			{/* Validation errors summary */}
			{hasErrors && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="light"
					title="Invalid conditions"
				>
					Some conditions have errors. Fix them before saving.
				</Alert>
			)}
		</Stack>
	);
}
