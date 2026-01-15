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
	Textarea,
	TextInput,
	Title,
	Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconCheck,
	IconChevronDown,
	IconChevronRight,
	IconFileCode,
	IconHistory,
	IconPlug,
	IconRefresh,
	IconRestore,
	IconServer,
	IconSettings,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { type SettingDto, settingsApi } from "@/api/settings";
import { systemIntegrationsApi } from "@/api/systemIntegrations";
import { TemplateEditor } from "@/components/forms/TemplateEditor";
import { TemplateSelector } from "@/components/forms/TemplateSelector";
import type { components } from "@/types/api.generated";

type SystemIntegrationDto = components["schemas"]["SystemIntegrationDto"];
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
	// State for test data - synced when selecting example templates
	const [testData, setTestData] = useState<Record<string, unknown> | undefined>(
		undefined,
	);

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

	// Handle template selection - update local state and test data
	const handleTemplateSelect = (
		template: string,
		sampleData: Record<string, unknown>,
	) => {
		setLocalTemplate(template);
		setTestData(sampleData);
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
				testData={testData}
				onTestDataChange={setTestData}
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

// Integration Card component
function IntegrationCard({
	integration,
	onEnable,
	onDisable,
	onTest,
	onDelete,
}: {
	integration: SystemIntegrationDto;
	onEnable: () => void;
	onDisable: () => void;
	onTest: () => void;
	onDelete: () => void;
}) {
	const healthColor =
		{
			healthy: "green",
			degraded: "yellow",
			unhealthy: "red",
			unknown: "gray",
			disabled: "gray",
		}[integration.healthStatus] || "gray";

	return (
		<Card withBorder padding="md">
			<Group justify="space-between" mb="xs">
				<Group gap="sm">
					<Text fw={500}>{integration.displayName}</Text>
					<Badge size="sm" variant="light">
						{integration.integrationType}
					</Badge>
				</Group>
				<Group gap="xs">
					<Badge color={healthColor} size="sm">
						{integration.healthStatus}
					</Badge>
					<Badge color={integration.enabled ? "green" : "gray"} size="sm">
						{integration.enabled ? "Enabled" : "Disabled"}
					</Badge>
				</Group>
			</Group>
			<Text size="sm" c="dimmed" mb="md">
				{integration.name}
			</Text>
			{integration.errorMessage && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					mb="md"
					title="Error"
				>
					{integration.errorMessage}
				</Alert>
			)}
			<Group gap="xs">
				{integration.enabled ? (
					<Button size="xs" variant="light" color="gray" onClick={onDisable}>
						Disable
					</Button>
				) : (
					<Button size="xs" variant="light" color="green" onClick={onEnable}>
						Enable
					</Button>
				)}
				<Button size="xs" variant="light" onClick={onTest}>
					Test Connection
				</Button>
				<Button size="xs" variant="light" color="red" onClick={onDelete}>
					Delete
				</Button>
			</Group>
		</Card>
	);
}

export function ServerSettings() {
	const queryClient = useQueryClient();
	const [historyModalOpened, setHistoryModalOpened] = useState(false);
	const [historyKey, setHistoryKey] = useState<string | null>(null);
	const [createIntegrationOpened, setCreateIntegrationOpened] = useState(false);

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

	// Fetch system integrations
	const { data: integrations, isLoading: integrationsLoading } = useQuery({
		queryKey: ["system-integrations"],
		queryFn: systemIntegrationsApi.getAll,
	});

	// Create integration form
	const integrationForm = useForm({
		initialValues: {
			name: "",
			displayName: "",
			integrationType: "metadata_provider",
			config: "{}",
		},
		validate: {
			name: (value) =>
				value.length < 1
					? "Name is required"
					: !/^[a-z0-9_]+$/.test(value)
						? "Name must be lowercase alphanumeric with underscores"
						: null,
			displayName: (value) =>
				value.length < 1 ? "Display name is required" : null,
		},
	});

	// Mutations
	const updateSettingMutation = useMutation({
		mutationFn: async ({ key, value }: { key: string; value: string }) => {
			return settingsApi.update(key, { value });
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["admin-settings"] });
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
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["admin-settings"] });
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

	const createIntegrationMutation = useMutation({
		mutationFn: async (data: {
			name: string;
			displayName: string;
			integrationType: string;
			config: string;
		}) => {
			return systemIntegrationsApi.create({
				name: data.name,
				displayName: data.displayName,
				integrationType: data.integrationType,
				config: JSON.parse(data.config || "{}"),
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["system-integrations"] });
			setCreateIntegrationOpened(false);
			integrationForm.reset();
			notifications.show({
				title: "Success",
				message: "Integration created",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to create integration",
				color: "red",
			});
		},
	});

	const enableIntegrationMutation = useMutation({
		mutationFn: systemIntegrationsApi.enable,
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["system-integrations"] });
			notifications.show({
				title: "Success",
				message: "Integration enabled",
				color: "green",
			});
		},
	});

	const disableIntegrationMutation = useMutation({
		mutationFn: systemIntegrationsApi.disable,
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["system-integrations"] });
			notifications.show({
				title: "Success",
				message: "Integration disabled",
				color: "green",
			});
		},
	});

	const testIntegrationMutation = useMutation({
		mutationFn: systemIntegrationsApi.test,
		onSuccess: (result) => {
			notifications.show({
				title: result.success ? "Success" : "Failed",
				message: result.message,
				color: result.success ? "green" : "red",
			});
		},
	});

	const deleteIntegrationMutation = useMutation({
		mutationFn: systemIntegrationsApi.delete,
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["system-integrations"] });
			notifications.show({
				title: "Success",
				message: "Integration deleted",
				color: "green",
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
						<Tabs.Tab value="integrations" leftSection={<IconPlug size={16} />}>
							System Integrations
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

					{/* System Integrations Tab */}
					<Tabs.Panel value="integrations" pt="md">
						<Stack gap="md">
							<Group justify="space-between">
								<Text c="dimmed">
									System integrations are app-wide external service connections
									managed by admins.
								</Text>
								<Button
									leftSection={<IconPlug size={16} />}
									onClick={() => setCreateIntegrationOpened(true)}
								>
									Add Integration
								</Button>
							</Group>

							{integrationsLoading ? (
								<Group justify="center" py="xl">
									<Loader />
								</Group>
							) : integrations?.integrations &&
								integrations.integrations.length > 0 ? (
								<Stack gap="md">
									{integrations.integrations.map(
										(integration: SystemIntegrationDto) => (
											<IntegrationCard
												key={integration.id}
												integration={integration}
												onEnable={() =>
													enableIntegrationMutation.mutate(integration.id)
												}
												onDisable={() =>
													disableIntegrationMutation.mutate(integration.id)
												}
												onTest={() =>
													testIntegrationMutation.mutate(integration.id)
												}
												onDelete={() =>
													deleteIntegrationMutation.mutate(integration.id)
												}
											/>
										),
									)}
								</Stack>
							) : (
								<Card withBorder>
									<Stack align="center" py="xl">
										<IconServer size={48} color="gray" />
										<Text c="dimmed">No system integrations configured.</Text>
										<Button
											variant="light"
											onClick={() => setCreateIntegrationOpened(true)}
										>
											Add Your First Integration
										</Button>
									</Stack>
								</Card>
							)}
						</Stack>
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

			{/* Create Integration Modal */}
			<Modal
				opened={createIntegrationOpened}
				onClose={() => {
					setCreateIntegrationOpened(false);
					integrationForm.reset();
				}}
				title="Add System Integration"
			>
				<form
					onSubmit={integrationForm.onSubmit((values) =>
						createIntegrationMutation.mutate(values),
					)}
				>
					<Stack gap="md">
						<TextInput
							label="Name"
							placeholder="mangaupdates"
							description="Unique identifier (lowercase, alphanumeric, underscores)"
							{...integrationForm.getInputProps("name")}
						/>
						<TextInput
							label="Display Name"
							placeholder="MangaUpdates"
							description="Human-readable name"
							{...integrationForm.getInputProps("displayName")}
						/>
						<TextInput
							label="Integration Type"
							placeholder="metadata_provider"
							description="Type of integration (metadata_provider, notification, storage)"
							{...integrationForm.getInputProps("integrationType")}
						/>
						<Textarea
							label="Configuration (JSON)"
							placeholder='{"base_url": "https://api.example.com"}'
							description="Non-sensitive configuration in JSON format"
							{...integrationForm.getInputProps("config")}
							minRows={3}
						/>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setCreateIntegrationOpened(false)}
							>
								Cancel
							</Button>
							<Button
								type="submit"
								loading={createIntegrationMutation.isPending}
							>
								Create
							</Button>
						</Group>
					</Stack>
				</form>
			</Modal>
		</Box>
	);
}
