import {
	ActionIcon,
	Alert,
	Box,
	Button,
	Group,
	Menu,
	Paper,
	SegmentedControl,
	Stack,
	Text,
	Tooltip,
	useComputedColorScheme,
} from "@mantine/core";
import {
	IconAlertCircle,
	IconChevronDown,
	IconCode,
	IconLock,
	IconLockOpen,
	IconTrash,
	IconTree,
} from "@tabler/icons-react";
import { githubDarkTheme, githubLightTheme, JsonEditor } from "json-edit-react";
import { useCallback, useEffect, useState } from "react";
import {
	DEFAULT_TEMPLATE_EXAMPLE,
	EXTERNAL_LINKS_EXAMPLE,
	MINIMAL_METADATA,
	READING_LIST_EXAMPLE,
} from "@/mocks/data/customMetadata";

export interface CustomMetadataEditorProps {
	/** Current custom metadata value */
	value: Record<string, unknown> | null;
	/** Callback when value changes */
	onChange: (value: Record<string, unknown> | null) => void;
	/** Whether the field is locked */
	locked: boolean;
	/** Callback when lock state changes */
	onLockChange: (locked: boolean) => void;
	/** Original value (for auto-lock detection) */
	originalValue?: Record<string, unknown> | null;
	/** Whether to auto-lock when value differs from original */
	autoLock?: boolean;
}

type ViewMode = "tree" | "json";

/**
 * A JSON editor for custom metadata with tree and raw JSON view modes.
 * Supports lockable field pattern for metadata protection.
 */
export function CustomMetadataEditor({
	value,
	onChange,
	locked,
	onLockChange,
	originalValue,
	autoLock = true,
}: CustomMetadataEditorProps) {
	const colorScheme = useComputedColorScheme("dark");
	const [viewMode, setViewMode] = useState<ViewMode>("tree");
	const [jsonError, setJsonError] = useState<string | null>(null);
	const [rawJson, setRawJson] = useState<string>("");

	// Sync rawJson with value when in tree mode or when value changes externally
	useEffect(() => {
		setRawJson(value ? JSON.stringify(value, null, 2) : "{}");
		setJsonError(null);
	}, [value]);

	const handleTreeChange = useCallback(
		(newData: unknown) => {
			const newValue = newData as Record<string, unknown>;
			onChange(newValue);
			setJsonError(null);

			// Auto-lock when value differs from original
			if (autoLock && originalValue !== undefined && !locked) {
				if (JSON.stringify(newValue) !== JSON.stringify(originalValue)) {
					onLockChange(true);
				}
			}
		},
		[onChange, autoLock, originalValue, locked, onLockChange],
	);

	const handleRawJsonChange = useCallback(
		(newJson: string) => {
			setRawJson(newJson);
			try {
				const parsed = JSON.parse(newJson) as Record<string, unknown>;
				if (typeof parsed !== "object" || Array.isArray(parsed)) {
					setJsonError("Custom metadata must be a JSON object");
					return;
				}
				onChange(parsed);
				setJsonError(null);

				// Auto-lock when value differs from original
				if (autoLock && originalValue !== undefined && !locked) {
					if (JSON.stringify(parsed) !== JSON.stringify(originalValue)) {
						onLockChange(true);
					}
				}
			} catch (e) {
				setJsonError(e instanceof Error ? e.message : "Invalid JSON");
			}
		},
		[onChange, autoLock, originalValue, locked, onLockChange],
	);

	const toggleLock = () => {
		onLockChange(!locked);
	};

	const handleClear = () => {
		onChange(null);
		setRawJson("{}");
		setJsonError(null);
	};

	// Use empty object if value is null for the editor
	const editorValue = value ?? {};

	// Theme configuration for json-edit-react
	const jsonTheme = colorScheme === "dark" ? githubDarkTheme : githubLightTheme;

	return (
		<Stack gap="sm">
			{/* Header with controls */}
			<Group justify="space-between">
				<Group gap="xs">
					<Tooltip
						label={
							locked
								? "Locked: Protected from automatic updates"
								: "Unlocked: Can be updated automatically"
						}
						position="left"
					>
						<ActionIcon
							variant="subtle"
							color={locked ? "orange" : "gray"}
							onClick={toggleLock}
							aria-label={locked ? "Unlock field" : "Lock field"}
						>
							{locked ? <IconLock size={18} /> : <IconLockOpen size={18} />}
						</ActionIcon>
					</Tooltip>
					<Text size="sm" fw={500}>
						Custom Metadata
					</Text>
				</Group>

				<Group gap="xs">
					<SegmentedControl
						size="xs"
						value={viewMode}
						onChange={(v) => setViewMode(v as ViewMode)}
						data={[
							{
								value: "tree",
								label: (
									<Group gap={4} wrap="nowrap" align="center">
										<IconTree size={14} style={{ display: "block" }} />
										<span>Tree</span>
									</Group>
								),
							},
							{
								value: "json",
								label: (
									<Group gap={4} wrap="nowrap" align="center">
										<IconCode size={14} style={{ display: "block" }} />
										<span>JSON</span>
									</Group>
								),
							},
						]}
					/>

					{value && Object.keys(value).length > 0 && (
						<Tooltip label="Clear all custom metadata">
							<ActionIcon
								variant="subtle"
								color="red"
								onClick={handleClear}
								aria-label="Clear custom metadata"
							>
								<IconTrash size={16} />
							</ActionIcon>
						</Tooltip>
					)}
				</Group>
			</Group>

			{/* Help text */}
			<Text size="xs" c="dimmed">
				{viewMode === "tree"
					? "Click on values to edit them. Use the + button to add new fields."
					: "Edit the raw JSON directly. Changes are validated automatically."}
			</Text>

			{/* Error display */}
			{jsonError && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="light"
					title="Invalid JSON"
				>
					{jsonError}
				</Alert>
			)}

			{/* Editor */}
			<Paper withBorder p="xs" style={{ minHeight: 200 }}>
				{viewMode === "tree" ? (
					<Stack gap="md">
						<Box style={{ overflow: "auto", maxHeight: 400 }}>
							<JsonEditor
								data={editorValue}
								setData={handleTreeChange}
								rootName="customMetadata"
								theme={[
									jsonTheme,
									{
										styles: {
											container: {
												fontSize: 12,
												fontFamily:
													'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
											},
											input:
												colorScheme === "dark"
													? { color: "#e6edf3", backgroundColor: "#21262d" }
													: {},
										},
									},
								]}
								collapse={2}
								indent={2}
								showCollectionCount="when-closed"
								enableClipboard
								minWidth="100%"
								showStringQuotes={false}
								restrictTypeSelection={[
									"string",
									"number",
									"boolean",
									"null",
									"object",
									"array",
								]}
							/>
						</Box>
						{/* Empty state suggestion - inside the editor panel */}
						{(!value || Object.keys(value).length === 0) && (
							<Stack gap="xs" align="center" py="md">
								<Text size="sm" c="dimmed" ta="center">
									No custom metadata yet. Click the + button in the editor above
									to add your first field, or load an example below.
								</Text>
								<Menu shadow="md" width={200} zIndex={1100}>
									<Menu.Target>
										<Button
											size="xs"
											variant="light"
											rightSection={<IconChevronDown size={14} />}
										>
											Load Example
										</Button>
									</Menu.Target>
									<Menu.Dropdown>
										<Menu.Label>Example Templates</Menu.Label>
										<Menu.Item
											onClick={() => onChange({ ...MINIMAL_METADATA })}
										>
											Minimal (status + rating)
										</Menu.Item>
										<Menu.Item
											onClick={() => onChange({ ...READING_LIST_EXAMPLE })}
										>
											Reading List
										</Menu.Item>
										<Menu.Item
											onClick={() => onChange({ ...EXTERNAL_LINKS_EXAMPLE })}
										>
											External Links
										</Menu.Item>
										<Menu.Item
											onClick={() => onChange({ ...DEFAULT_TEMPLATE_EXAMPLE })}
										>
											Collection Info
										</Menu.Item>
										<Menu.Divider />
										<Menu.Item onClick={() => onChange({ example: "value" })}>
											Simple Key-Value
										</Menu.Item>
									</Menu.Dropdown>
								</Menu>
							</Stack>
						)}
					</Stack>
				) : (
					<textarea
						value={rawJson}
						onChange={(e) => handleRawJsonChange(e.target.value)}
						style={{
							width: "100%",
							minHeight: 200,
							maxHeight: 400,
							fontFamily:
								'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
							fontSize: "12px",
							padding: "8px",
							border: "none",
							outline: "none",
							resize: "vertical",
							backgroundColor: "transparent",
							color: "inherit",
						}}
						placeholder="{}"
						spellCheck={false}
					/>
				)}
			</Paper>
		</Stack>
	);
}
