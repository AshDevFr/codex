import {
	ActionIcon,
	Alert,
	Badge,
	Box,
	Button,
	Card,
	Collapse,
	Group,
	Loader,
	Modal,
	NumberInput,
	Stack,
	Switch,
	Table,
	Tabs,
	Text,
	TextInput,
	Title,
	Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconCheck,
	IconChevronDown,
	IconChevronRight,
	IconFileCode,
	IconHistory,
	IconRefresh,
	IconRestore,
	IconSettings,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { type SettingDto, settingsApi } from "@/api/settings";
import { TemplateEditor } from "@/components/forms/TemplateEditor";
import { TemplateSelector } from "@/components/forms/TemplateSelector";
import { brandingQueryKey } from "@/hooks/useAppName";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import type { components } from "@/types/api.generated";

type SettingHistoryDto = components["schemas"]["SettingHistoryDto"];

// Group settings by category
function groupSettingsByCategory(settings: SettingDto[]) {
	const groups: Record<string, SettingDto[]> = {};
	for (const setting of settings) {
		if (!groups[setting.category]) {
			groups[setting.category] = [];
		}
		groups[setting.category].push(setting);
	}
	return groups;
}

// Format category name for display
function formatCategoryName(category: string): string {
	return category
		.split("_")
		.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
		.join(" ");
}

// Setting row component
function SettingRow({
	setting,
	onUpdate,
	onReset,
	onViewHistory,
}: {
	setting: SettingDto;
	onUpdate: (key: string, value: string) => void;
	onReset: (key: string) => void;
	onViewHistory: (key: string) => void;
}) {
	const [localValue, setLocalValue] = useState(setting.value);
	const [isEditing, setIsEditing] = useState(false);

	const handleSave = () => {
		if (localValue !== setting.value) {
			onUpdate(setting.key, localValue);
		}
		setIsEditing(false);
	};

	const handleCancel = () => {
		setLocalValue(setting.value);
		setIsEditing(false);
	};

	const renderInput = () => {
		switch (setting.value_type) {
			case "boolean":
				return (
					<Switch
						checked={localValue === "true"}
						onChange={(e) => {
							const newValue = String(e.currentTarget.checked);
							setLocalValue(newValue);
							onUpdate(setting.key, newValue);
						}}
					/>
				);
			case "integer":
				return (
					<NumberInput
						value={Number.parseInt(localValue, 10) || 0}
						onChange={(value) => setLocalValue(String(value))}
						min={setting.min_value ?? undefined}
						max={setting.max_value ?? undefined}
						onBlur={handleSave}
						w={120}
					/>
				);
			default:
				return isEditing ? (
					<Group gap="xs">
						<TextInput
							value={localValue}
							onChange={(e) => setLocalValue(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter") handleSave();
								if (e.key === "Escape") handleCancel();
							}}
							autoFocus
							w={200}
						/>
						<Button size="xs" onClick={handleSave}>
							Save
						</Button>
						<Button size="xs" variant="subtle" onClick={handleCancel}>
							Cancel
						</Button>
					</Group>
				) : (
					<Group gap="xs">
						<Text
							style={{ cursor: "pointer" }}
							onClick={() => setIsEditing(true)}
						>
							{setting.is_sensitive ? "••••••••" : localValue || "(empty)"}
						</Text>
						<Button
							size="xs"
							variant="subtle"
							onClick={() => setIsEditing(true)}
						>
							Edit
						</Button>
					</Group>
				);
		}
	};

	return (
		<Table.Tr>
			<Table.Td>
				<Stack gap={2}>
					<Text fw={500}>{setting.key}</Text>
					<Text size="xs" c="dimmed">
						{setting.description}
					</Text>
				</Stack>
			</Table.Td>
			<Table.Td>{renderInput()}</Table.Td>
			<Table.Td>
				<Badge variant="light" size="sm">
					{setting.value_type}
				</Badge>
			</Table.Td>
			<Table.Td>
				<Group gap="xs">
					<Tooltip label="View History">
						<ActionIcon
							variant="subtle"
							onClick={() => onViewHistory(setting.key)}
						>
							<IconHistory size={16} />
						</ActionIcon>
					</Tooltip>
					<Tooltip label="Reset to Default">
						<ActionIcon
							variant="subtle"
							color="orange"
							onClick={() => onReset(setting.key)}
							disabled={setting.value === setting.default_value}
						>
							<IconRefresh size={16} />
						</ActionIcon>
					</Tooltip>
				</Group>
			</Table.Td>
		</Table.Tr>
	);
}

// Template setting key constant
const CUSTOM_METADATA_TEMPLATE_KEY = "display.custom_metadata_template";

// Custom Metadata Template section with manual save
function CustomMetadataTemplateSection({
	setting,
	onSave,
	onViewHistory,
	isSaving,
}: {
	setting: SettingDto | undefined;
	onSave: (key: string, value: string) => void;
	onViewHistory: (key: string) => void;
	isSaving: boolean;
}) {
	// Local state for editing (not auto-saved)
	const [localTemplate, setLocalTemplate] = useState(setting?.value ?? "");

	// Sync local state when setting value changes externally (e.g., from history restore)
	useEffect(() => {
		setLocalTemplate(setting?.value ?? "");
	}, [setting?.value]);

	// Track if there are unsaved changes
	const hasChanges = localTemplate !== (setting?.value ?? "");

	// Revert to saved value from DB
	const handleRevertChanges = () => {
		setLocalTemplate(setting?.value ?? "");
	};

	// Handle template selection - update local state
	// Note: sampleData is ignored as TemplateEditor now manages its own context
	const handleTemplateSelect = (
		template: string,
		_sampleData: Record<string, unknown>,
	) => {
		setLocalTemplate(template);
	};

	// Handle save
	const handleSave = () => {
		onSave(CUSTOM_METADATA_TEMPLATE_KEY, localTemplate);
	};

	if (!setting) {
		return (
			<Alert icon={<IconAlertCircle size={16} />} color="yellow">
				Custom metadata template setting not found.
			</Alert>
		);
	}

	return (
		<Stack gap="md">
			<TemplateEditor
				value={localTemplate}
				onChange={setLocalTemplate}
				label="Custom Metadata Display Template"
				description="Handlebars template for displaying custom metadata on series detail pages. The template is rendered as Markdown."
			/>
			<Group justify="space-between">
				<Group gap="xs">
					<TemplateSelector
						onSelect={handleTemplateSelect}
						currentTemplate={localTemplate}
					/>
					<Tooltip label="View change history">
						<ActionIcon
							variant="subtle"
							onClick={() => onViewHistory(CUSTOM_METADATA_TEMPLATE_KEY)}
						>
							<IconHistory size={16} />
						</ActionIcon>
					</Tooltip>
				</Group>
				<Group gap="xs">
					<Button
						variant="subtle"
						size="sm"
						onClick={handleRevertChanges}
						disabled={!hasChanges}
					>
						Revert Changes
					</Button>
					<Button
						size="sm"
						onClick={handleSave}
						disabled={!hasChanges}
						loading={isSaving}
						leftSection={<IconCheck size={16} />}
					>
						Save Template
					</Button>
				</Group>
			</Group>
		</Stack>
	);
}

// Settings category section
function SettingsCategorySection({
	category,
	settings,
	onUpdate,
	onReset,
	onViewHistory,
}: {
	category: string;
	settings: SettingDto[];
	onUpdate: (key: string, value: string) => void;
	onReset: (key: string) => void;
	onViewHistory: (key: string) => void;
}) {
	const [opened, { toggle }] = useDisclosure(true);

	return (
		<Card withBorder>
			<Group
				onClick={toggle}
				style={{ cursor: "pointer" }}
				justify="space-between"
			>
				<Title order={4}>{formatCategoryName(category)}</Title>
				{opened ? (
					<IconChevronDown size={20} />
				) : (
					<IconChevronRight size={20} />
				)}
			</Group>
			<Collapse in={opened}>
				<Table mt="md">
					<Table.Thead>
						<Table.Tr>
							<Table.Th>Setting</Table.Th>
							<Table.Th>Value</Table.Th>
							<Table.Th>Type</Table.Th>
							<Table.Th>Actions</Table.Th>
						</Table.Tr>
					</Table.Thead>
					<Table.Tbody>
						{settings.map((setting) => (
							<SettingRow
								key={setting.key}
								setting={setting}
								onUpdate={onUpdate}
								onReset={onReset}
								onViewHistory={onViewHistory}
							/>
						))}
					</Table.Tbody>
				</Table>
			</Collapse>
		</Card>
	);
}

export function ServerSettings() {
	const queryClient = useQueryClient();
	const [historyModalOpened, setHistoryModalOpened] = useState(false);
	const [historyKey, setHistoryKey] = useState<string | null>(null);

	useDocumentTitle("Server Settings");

	// Fetch settings
	const {
		data: settings,
		isLoading: settingsLoading,
		error: settingsError,
	} = useQuery({
		queryKey: ["admin-settings"],
		queryFn: settingsApi.list,
	});

	// Fetch setting history
	const { data: history, isLoading: historyLoading } = useQuery({
		queryKey: ["admin-settings-history", historyKey],
		queryFn: () => (historyKey ? settingsApi.getHistory(historyKey) : []),
		enabled: !!historyKey && historyModalOpened,
	});

	// Mutations
	const updateSettingMutation = useMutation({
		mutationFn: async ({ key, value }: { key: string; value: string }) => {
			return settingsApi.update(key, { value });
		},
		onSuccess: (_data, variables) => {
			queryClient.invalidateQueries({ queryKey: ["admin-settings"] });
			queryClient.invalidateQueries({ queryKey: ["public-settings"] });
			// Invalidate branding cache when application name is updated
			if (variables.key === "application.name") {
				queryClient.invalidateQueries({ queryKey: brandingQueryKey });
			}
			notifications.show({
				title: "Success",
				message: "Setting updated",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to update setting",
				color: "red",
			});
		},
	});

	const resetSettingMutation = useMutation({
		mutationFn: async (key: string) => {
			return settingsApi.reset(key);
		},
		onSuccess: (_data, key) => {
			queryClient.invalidateQueries({ queryKey: ["admin-settings"] });
			queryClient.invalidateQueries({ queryKey: ["public-settings"] });
			// Invalidate branding cache when application name is reset
			if (key === "application.name") {
				queryClient.invalidateQueries({ queryKey: brandingQueryKey });
			}
			notifications.show({
				title: "Success",
				message: "Setting reset to default",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to reset setting",
				color: "red",
			});
		},
	});

	const handleViewHistory = (key: string) => {
		setHistoryKey(key);
		setHistoryModalOpened(true);
	};

	const groupedSettings = settings ? groupSettingsByCategory(settings) : {};

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Server Settings</Title>

				<Tabs defaultValue="settings">
					<Tabs.List>
						<Tabs.Tab value="settings" leftSection={<IconSettings size={16} />}>
							Settings
						</Tabs.Tab>
						<Tabs.Tab
							value="custom-metadata"
							leftSection={<IconFileCode size={16} />}
						>
							Custom Metadata
						</Tabs.Tab>
					</Tabs.List>

					{/* Settings Tab */}
					<Tabs.Panel value="settings" pt="md">
						{settingsLoading ? (
							<Group justify="center" py="xl">
								<Loader />
							</Group>
						) : settingsError ? (
							<Alert icon={<IconAlertCircle size={16} />} color="red">
								Failed to load settings. Please try again.
							</Alert>
						) : (
							<Stack gap="md">
								{Object.entries(groupedSettings)
									.filter(([category]) => category.toLowerCase() !== "display")
									.sort(([a], [b]) => a.localeCompare(b))
									.map(([category, categorySettings]) => (
										<SettingsCategorySection
											key={category}
											category={category}
											settings={categorySettings}
											onUpdate={(key, value) =>
												updateSettingMutation.mutate({ key, value })
											}
											onReset={(key) => resetSettingMutation.mutate(key)}
											onViewHistory={handleViewHistory}
										/>
									))}
							</Stack>
						)}
					</Tabs.Panel>

					{/* Custom Metadata Tab */}
					<Tabs.Panel value="custom-metadata" pt="md">
						{settingsLoading ? (
							<Group justify="center" py="xl">
								<Loader />
							</Group>
						) : settingsError ? (
							<Alert icon={<IconAlertCircle size={16} />} color="red">
								Failed to load settings. Please try again.
							</Alert>
						) : (
							<CustomMetadataTemplateSection
								setting={(groupedSettings.Display || []).find(
									(s) => s.key === CUSTOM_METADATA_TEMPLATE_KEY,
								)}
								onSave={(key, value) =>
									updateSettingMutation.mutate({ key, value })
								}
								onViewHistory={handleViewHistory}
								isSaving={updateSettingMutation.isPending}
							/>
						)}
					</Tabs.Panel>
				</Tabs>
			</Stack>

			{/* History Modal */}
			<Modal
				opened={historyModalOpened}
				onClose={() => {
					setHistoryModalOpened(false);
					setHistoryKey(null);
				}}
				title={`History: ${historyKey}`}
				size="xl"
			>
				{historyLoading ? (
					<Group justify="center" py="xl">
						<Loader />
					</Group>
				) : history && history.length > 0 ? (
					<Table>
						<Table.Thead>
							<Table.Tr>
								<Table.Th>Previous Value</Table.Th>
								<Table.Th>New Value</Table.Th>
								<Table.Th>Changed At</Table.Th>
								<Table.Th>Reason</Table.Th>
								<Table.Th>Actions</Table.Th>
							</Table.Tr>
						</Table.Thead>
						<Table.Tbody>
							{history.map((entry: SettingHistoryDto, index: number) => {
								// Get the current setting value to check if restore is needed
								const currentValue = settings?.find(
									(s) => s.key === historyKey,
								)?.value;
								const canRestore =
									entry.old_value !== null && entry.old_value !== currentValue;

								return (
									// biome-ignore lint/suspicious/noArrayIndexKey: History entries have no unique ID
									<Table.Tr key={index}>
										<Table.Td>
											<Text
												size="sm"
												style={{
													fontFamily: "monospace",
													whiteSpace: "pre-wrap",
													wordBreak: "break-word",
													maxWidth: 200,
												}}
												lineClamp={3}
											>
												{entry.old_value ?? "(empty)"}
											</Text>
										</Table.Td>
										<Table.Td>
											<Text
												size="sm"
												style={{
													fontFamily: "monospace",
													whiteSpace: "pre-wrap",
													wordBreak: "break-word",
													maxWidth: 200,
												}}
												lineClamp={3}
											>
												{entry.new_value}
											</Text>
										</Table.Td>
										<Table.Td>
											{new Date(entry.changed_at).toLocaleString()}
										</Table.Td>
										<Table.Td>{entry.change_reason || "-"}</Table.Td>
										<Table.Td>
											{canRestore ? (
												<Tooltip label="Restore to this value">
													<ActionIcon
														variant="subtle"
														color="blue"
														onClick={() => {
															if (historyKey) {
																updateSettingMutation.mutate({
																	key: historyKey,
																	value: entry.old_value as string,
																});
															}
														}}
														loading={updateSettingMutation.isPending}
													>
														<IconRestore size={16} />
													</ActionIcon>
												</Tooltip>
											) : (
												<Text size="xs" c="dimmed">
													-
												</Text>
											)}
										</Table.Td>
									</Table.Tr>
								);
							})}
						</Table.Tbody>
					</Table>
				) : (
					<Text c="dimmed" ta="center" py="xl">
						No history available for this setting.
					</Text>
				)}
			</Modal>
		</Box>
	);
}
