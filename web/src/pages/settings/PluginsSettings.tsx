import {
	ActionIcon,
	Alert,
	Badge,
	Box,
	Button,
	Card,
	Code,
	Collapse,
	Divider,
	Group,
	Loader,
	Modal,
	MultiSelect,
	NumberInput,
	ScrollArea,
	Select,
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
	IconChevronDown,
	IconChevronRight,
	IconEdit,
	IconPlayerPlay,
	IconPlugConnected,
	IconPlus,
	IconRefresh,
	IconSettings,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { librariesApi } from "@/api/libraries";
import {
	AVAILABLE_PERMISSIONS,
	AVAILABLE_SCOPES,
	CREDENTIAL_DELIVERY_OPTIONS,
	type CreatePluginRequest,
	type PluginDto,
	type PluginFailuresResponse,
	type PluginHealthStatus,
	pluginsApi,
} from "@/api/plugins";
import { SearchConfigModal } from "@/components/forms/SearchConfigModal";

// Health status badge color mapping
const healthStatusColors: Record<PluginHealthStatus, string> = {
	unknown: "gray",
	healthy: "green",
	degraded: "yellow",
	unhealthy: "orange",
	disabled: "red",
};

// Plugin form values type
interface PluginFormValues {
	name: string;
	displayName: string;
	description: string;
	command: string;
	args: string;
	envVars: { key: string; value: string }[];
	workingDirectory: string;
	permissions: string[];
	scopes: string[];
	allLibraries: boolean;
	libraryIds: string[];
	credentialDelivery: string;
	credentials: string;
	config: string;
	enabled: boolean;
	rateLimitEnabled: boolean;
	rateLimitRequestsPerMinute: number;
}

const defaultFormValues: PluginFormValues = {
	name: "",
	displayName: "",
	description: "",
	command: "",
	args: "",
	envVars: [],
	workingDirectory: "",
	permissions: [],
	scopes: [],
	allLibraries: true,
	libraryIds: [],
	credentialDelivery: "env",
	credentials: "",
	config: "",
	enabled: false,
	rateLimitEnabled: true,
	rateLimitRequestsPerMinute: 60,
};

// Normalize plugin name to slug format (lowercase alphanumeric with hyphens)
// Matches backend validation: lowercase alphanumeric and hyphens only
// Cannot start or end with a hyphen
function normalizePluginName(value: string): string {
	return value
		.toLowerCase()
		.trim()
		.replace(/[\s_]+/g, "-") // spaces and underscores -> hyphens
		.replace(/-+/g, "-") // collapse multiple hyphens to single
		.replace(/[^a-z0-9-]/g, "") // remove invalid chars
		.replace(/^-+|-+$/g, ""); // trim leading/trailing hyphens
}

/**
 * Safely parse JSON with try-catch to handle malformed input.
 * Returns undefined if parsing fails, showing an error notification to the user.
 */
function safeJsonParse(
	jsonString: string,
	fieldName: string,
): Record<string, unknown> | undefined {
	try {
		return JSON.parse(jsonString);
	} catch {
		notifications.show({
			title: "Invalid JSON",
			message: `The ${fieldName} field contains invalid JSON. Please check the format.`,
			color: "red",
		});
		return undefined;
	}
}

export function PluginsSettings() {
	const queryClient = useQueryClient();
	const [
		createModalOpened,
		{ open: openCreateModal, close: closeCreateModal },
	] = useDisclosure(false);
	const [editModalOpened, { open: openEditModal, close: closeEditModal }] =
		useDisclosure(false);
	const [
		deleteModalOpened,
		{ open: openDeleteModal, close: closeDeleteModal },
	] = useDisclosure(false);
	const [selectedPlugin, setSelectedPlugin] = useState<PluginDto | null>(null);
	const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());
	const [searchConfigPlugin, setSearchConfigPlugin] =
		useState<PluginDto | null>(null);

	// Fetch plugins
	const {
		data: pluginsResponse,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["plugins"],
		queryFn: pluginsApi.getAll,
	});

	const plugins = pluginsResponse?.plugins ?? [];

	// Fetch libraries for the library filter dropdown
	const { data: libraries = [] } = useQuery({
		queryKey: ["libraries"],
		queryFn: librariesApi.getAll,
	});

	// Create form
	const createForm = useForm<PluginFormValues>({
		initialValues: defaultFormValues,
		validate: {
			name: (value) => {
				if (!value.trim()) return "Name is required";
				if (!/^[a-z0-9-]+$/.test(value)) {
					return "Name must be lowercase alphanumeric with hyphens only";
				}
				return null;
			},
			displayName: (value) =>
				!value.trim() ? "Display name is required" : null,
			command: (value) => (!value.trim() ? "Command is required" : null),
		},
	});

	// Edit form
	const editForm = useForm<PluginFormValues>({
		initialValues: defaultFormValues,
		validate: {
			displayName: (value) =>
				!value.trim() ? "Display name is required" : null,
			command: (value) => (!value.trim() ? "Command is required" : null),
		},
	});

	// Mutations
	const createMutation = useMutation({
		mutationFn: async (values: PluginFormValues) => {
			const request: CreatePluginRequest = {
				name: values.name.trim(),
				displayName: values.displayName.trim(),
				description: values.description.trim() || undefined,
				command: values.command.trim(),
				args: values.args
					.split("\n")
					.map((a) => a.trim())
					.filter(Boolean),
				env: values.envVars.filter((e) => e.key.trim()),
				workingDirectory: values.workingDirectory.trim() || undefined,
				permissions: values.permissions,
				scopes: values.scopes,
				libraryIds: values.allLibraries ? [] : values.libraryIds,
				credentialDelivery: values.credentialDelivery,
				credentials: values.credentials.trim()
					? safeJsonParse(values.credentials, "credentials")
					: undefined,
				config: values.config.trim()
					? safeJsonParse(values.config, "config")
					: undefined,
				enabled: values.enabled,
				rateLimitRequestsPerMinute: values.rateLimitEnabled
					? values.rateLimitRequestsPerMinute
					: null,
			};
			return pluginsApi.create(request);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			closeCreateModal();
			createForm.reset();
			notifications.show({
				title: "Success",
				message: "Plugin created successfully",
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to create plugin",
				color: "red",
			});
		},
	});

	const updateMutation = useMutation({
		mutationFn: async ({
			id,
			values,
		}: {
			id: string;
			values: PluginFormValues;
		}) => {
			return pluginsApi.update(id, {
				displayName: values.displayName.trim(),
				description: values.description.trim() || null,
				command: values.command.trim(),
				args: values.args
					.split("\n")
					.map((a) => a.trim())
					.filter(Boolean),
				env: values.envVars.filter((e) => e.key.trim()),
				workingDirectory: values.workingDirectory.trim() || null,
				permissions: values.permissions,
				scopes: values.scopes,
				libraryIds: values.allLibraries ? [] : values.libraryIds,
				credentialDelivery: values.credentialDelivery,
				credentials: values.credentials.trim()
					? safeJsonParse(values.credentials, "credentials")
					: undefined,
				config: values.config.trim()
					? safeJsonParse(values.config, "config")
					: undefined,
				rateLimitRequestsPerMinute: values.rateLimitEnabled
					? values.rateLimitRequestsPerMinute
					: null,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			closeEditModal();
			setSelectedPlugin(null);
			notifications.show({
				title: "Success",
				message: "Plugin updated successfully",
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update plugin",
				color: "red",
			});
		},
	});

	const deleteMutation = useMutation({
		mutationFn: pluginsApi.delete,
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			closeDeleteModal();
			setSelectedPlugin(null);
			notifications.show({
				title: "Success",
				message: "Plugin deleted successfully",
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to delete plugin",
				color: "red",
			});
		},
	});

	const enableMutation = useMutation({
		mutationFn: pluginsApi.enable,
		onSuccess: (data) => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			notifications.show({
				title: "Success",
				message: data.message,
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to enable plugin",
				color: "red",
			});
		},
	});

	const disableMutation = useMutation({
		mutationFn: pluginsApi.disable,
		onSuccess: (data) => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			notifications.show({
				title: "Success",
				message: data.message,
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to disable plugin",
				color: "red",
			});
		},
	});

	const testMutation = useMutation({
		mutationFn: pluginsApi.test,
		onSuccess: (data) => {
			if (data.success) {
				notifications.show({
					title: "Connection Successful",
					message: `${data.message}${data.latencyMs ? ` (${data.latencyMs}ms)` : ""}`,
					color: "green",
				});
			} else {
				notifications.show({
					title: "Connection Failed",
					message: data.message,
					color: "red",
				});
			}
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Test Failed",
				message: error.message || "Failed to test plugin connection",
				color: "red",
			});
		},
	});

	const resetFailuresMutation = useMutation({
		mutationFn: pluginsApi.resetFailures,
		onSuccess: (data) => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			queryClient.invalidateQueries({ queryKey: ["plugin-actions"] });
			notifications.show({
				title: "Success",
				message: data.message,
				color: "green",
			});
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to reset failures",
				color: "red",
			});
		},
	});

	const handleEditPlugin = (plugin: PluginDto) => {
		setSelectedPlugin(plugin);
		editForm.setValues({
			name: plugin.name,
			displayName: plugin.displayName,
			description: plugin.description || "",
			command: plugin.command,
			args: plugin.args.join("\n"),
			envVars:
				typeof plugin.env === "object" && plugin.env !== null
					? Object.entries(plugin.env as Record<string, string>).map(
							([key, value]) => ({ key, value }),
						)
					: [],
			workingDirectory: plugin.workingDirectory || "",
			permissions: plugin.permissions,
			scopes: plugin.scopes,
			allLibraries: plugin.libraryIds.length === 0,
			libraryIds: plugin.libraryIds,
			credentialDelivery: plugin.credentialDelivery,
			credentials: "",
			config:
				plugin.config && Object.keys(plugin.config as object).length > 0
					? JSON.stringify(plugin.config, null, 2)
					: "",
			enabled: plugin.enabled,
			rateLimitEnabled: plugin.rateLimitRequestsPerMinute != null,
			rateLimitRequestsPerMinute: plugin.rateLimitRequestsPerMinute ?? 60,
		});
		openEditModal();
	};

	const handleDeletePlugin = (plugin: PluginDto) => {
		setSelectedPlugin(plugin);
		openDeleteModal();
	};

	const toggleRowExpansion = (id: string) => {
		setExpandedRows((prev) => {
			const next = new Set(prev);
			if (next.has(id)) {
				next.delete(id);
			} else {
				next.add(id);
			}
			return next;
		});
	};

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<div>
						<Title order={1}>Plugins</Title>
						<Text c="dimmed" size="sm" mt="xs">
							Manage external plugin processes for metadata fetching and other
							integrations
						</Text>
					</div>
					<Button
						leftSection={<IconPlus size={16} />}
						onClick={openCreateModal}
					>
						Add Plugin
					</Button>
				</Group>

				{isLoading ? (
					<Group justify="center" py="xl">
						<Loader />
					</Group>
				) : error ? (
					<Alert icon={<IconAlertCircle size={16} />} color="red">
						Failed to load plugins. Please try again.
					</Alert>
				) : plugins.length > 0 ? (
					<Card withBorder p={0}>
						<ScrollArea>
							<Table>
								<Table.Thead>
									<Table.Tr>
										<Table.Th w={40} />
										<Table.Th>Plugin</Table.Th>
										<Table.Th>Command</Table.Th>
										<Table.Th>Status</Table.Th>
										<Table.Th>Health</Table.Th>
										<Table.Th>Actions</Table.Th>
									</Table.Tr>
								</Table.Thead>
								<Table.Tbody>
									{plugins.map((plugin) => (
										<>
											<Table.Tr key={plugin.id}>
												<Table.Td>
													<ActionIcon
														variant="subtle"
														size="sm"
														onClick={() => toggleRowExpansion(plugin.id)}
													>
														{expandedRows.has(plugin.id) ? (
															<IconChevronDown size={16} />
														) : (
															<IconChevronRight size={16} />
														)}
													</ActionIcon>
												</Table.Td>
												<Table.Td>
													<Group gap="sm">
														<IconPlugConnected size={20} />
														<div>
															<Text fw={500}>{plugin.displayName}</Text>
															<Text size="xs" c="dimmed">
																{plugin.name}
															</Text>
														</div>
													</Group>
												</Table.Td>
												<Table.Td>
													<Code>{plugin.command}</Code>
												</Table.Td>
												<Table.Td>
													<Switch
														checked={plugin.enabled}
														onChange={() =>
															plugin.enabled
																? disableMutation.mutate(plugin.id)
																: enableMutation.mutate(plugin.id)
														}
														disabled={
															enableMutation.isPending ||
															disableMutation.isPending
														}
													/>
												</Table.Td>
												<Table.Td>
													<Group gap="xs">
														<Badge
															color={
																healthStatusColors[
																	plugin.healthStatus as PluginHealthStatus
																] || "gray"
															}
															variant="light"
														>
															{plugin.healthStatus}
														</Badge>
														{plugin.failureCount > 0 && (
															<Tooltip
																label={`${plugin.failureCount} failures${plugin.lastFailureAt ? ` (last: ${new Date(plugin.lastFailureAt).toLocaleString()})` : ""}`}
															>
																<Badge color="red" variant="outline" size="sm">
																	{plugin.failureCount}
																</Badge>
															</Tooltip>
														)}
													</Group>
												</Table.Td>
												<Table.Td>
													<Group gap="xs">
														<Tooltip label="Test Connection">
															<ActionIcon
																variant="subtle"
																onClick={() => testMutation.mutate(plugin.id)}
																loading={
																	testMutation.isPending &&
																	testMutation.variables === plugin.id
																}
															>
																<IconPlayerPlay size={16} />
															</ActionIcon>
														</Tooltip>
														{plugin.failureCount > 0 && (
															<Tooltip label="Reset Failures">
																<ActionIcon
																	variant="subtle"
																	color="yellow"
																	onClick={() =>
																		resetFailuresMutation.mutate(plugin.id)
																	}
																	loading={
																		resetFailuresMutation.isPending &&
																		resetFailuresMutation.variables ===
																			plugin.id
																	}
																>
																	<IconRefresh size={16} />
																</ActionIcon>
															</Tooltip>
														)}
														{plugin.manifest?.capabilities?.metadataProvider &&
															plugin.manifest.capabilities.metadataProvider
																.length > 0 && (
																<Tooltip label="Configure Search">
																	<ActionIcon
																		variant="subtle"
																		color="blue"
																		onClick={() =>
																			setSearchConfigPlugin(plugin)
																		}
																	>
																		<IconSettings size={16} />
																	</ActionIcon>
																</Tooltip>
															)}
														<Tooltip label="Edit Plugin">
															<ActionIcon
																variant="subtle"
																onClick={() => handleEditPlugin(plugin)}
															>
																<IconEdit size={16} />
															</ActionIcon>
														</Tooltip>
														<Tooltip label="Delete Plugin">
															<ActionIcon
																variant="subtle"
																color="red"
																onClick={() => handleDeletePlugin(plugin)}
															>
																<IconTrash size={16} />
															</ActionIcon>
														</Tooltip>
													</Group>
												</Table.Td>
											</Table.Tr>
											<Table.Tr key={`${plugin.id}-details`}>
												<Table.Td colSpan={6} p={0}>
													<Collapse in={expandedRows.has(plugin.id)}>
														<Box
															p="md"
															bg="var(--mantine-color-dark-6)"
															style={{
																borderTop:
																	"1px solid var(--mantine-color-dark-4)",
															}}
														>
															<PluginDetails
																plugin={plugin}
																libraries={libraries}
															/>
														</Box>
													</Collapse>
												</Table.Td>
											</Table.Tr>
										</>
									))}
								</Table.Tbody>
							</Table>
						</ScrollArea>
					</Card>
				) : (
					<Alert
						icon={<IconPlugConnected size={16} />}
						color="gray"
						variant="light"
					>
						<Text fw={500}>No plugins configured</Text>
						<Text size="sm" mt="xs">
							Add plugins to enable metadata fetching from external sources like
							MangaBaka, AniList, or other providers.
						</Text>
					</Alert>
				)}
			</Stack>

			{/* Create Plugin Modal */}
			<Modal
				opened={createModalOpened}
				onClose={() => {
					closeCreateModal();
					createForm.reset();
				}}
				title="Add Plugin"
				size="lg"
			>
				<PluginForm
					form={createForm}
					onSubmit={(values) => createMutation.mutate(values)}
					isLoading={createMutation.isPending}
					onCancel={() => {
						closeCreateModal();
						createForm.reset();
					}}
					isCreate
					libraries={libraries}
				/>
			</Modal>

			{/* Edit Plugin Modal */}
			<Modal
				opened={editModalOpened}
				onClose={() => {
					closeEditModal();
					setSelectedPlugin(null);
				}}
				title={`Edit Plugin: ${selectedPlugin?.displayName}`}
				size="lg"
			>
				<PluginForm
					form={editForm}
					onSubmit={(values) =>
						selectedPlugin &&
						updateMutation.mutate({ id: selectedPlugin.id, values })
					}
					isLoading={updateMutation.isPending}
					onCancel={() => {
						closeEditModal();
						setSelectedPlugin(null);
					}}
					libraries={libraries}
				/>
			</Modal>

			{/* Delete Plugin Modal */}
			<Modal
				opened={deleteModalOpened}
				onClose={() => {
					closeDeleteModal();
					setSelectedPlugin(null);
				}}
				title="Delete Plugin"
			>
				<Stack gap="md">
					<Text>
						Are you sure you want to delete the plugin{" "}
						<strong>{selectedPlugin?.displayName}</strong>?
					</Text>
					<Text size="sm" c="dimmed">
						This action cannot be undone.
					</Text>
					<Group justify="flex-end">
						<Button
							variant="subtle"
							onClick={() => {
								closeDeleteModal();
								setSelectedPlugin(null);
							}}
						>
							Cancel
						</Button>
						<Button
							color="red"
							loading={deleteMutation.isPending}
							onClick={() =>
								selectedPlugin && deleteMutation.mutate(selectedPlugin.id)
							}
						>
							Delete Plugin
						</Button>
					</Group>
				</Stack>
			</Modal>

			{/* Search Config Modal */}
			{searchConfigPlugin && (
				<SearchConfigModal
					plugin={searchConfigPlugin}
					opened={!!searchConfigPlugin}
					onClose={() => setSearchConfigPlugin(null)}
				/>
			)}
		</Box>
	);
}

// Plugin details component for expanded row
function PluginDetails({
	plugin,
	libraries,
}: {
	plugin: PluginDto;
	libraries: { id: string; name: string }[];
}) {
	return (
		<Stack gap="sm">
			<Group gap="xl">
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Description
					</Text>
					<Text size="sm">{plugin.description || "No description"}</Text>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Credentials
					</Text>
					<Text size="sm">
						{plugin.hasCredentials ? "Configured" : "Not configured"}
					</Text>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Delivery Method
					</Text>
					<Text size="sm">{plugin.credentialDelivery}</Text>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Rate Limit
					</Text>
					<Text size="sm">
						{plugin.rateLimitRequestsPerMinute != null
							? `${plugin.rateLimitRequestsPerMinute} req/min`
							: "No limit"}
					</Text>
				</div>
			</Group>

			{plugin.args.length > 0 && (
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Arguments
					</Text>
					<Code block>{plugin.args.join("\n")}</Code>
				</div>
			)}

			<Group gap="xl">
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Permissions
					</Text>
					<Group gap="xs" mt={4}>
						{plugin.permissions.length > 0 ? (
							plugin.permissions.map((perm) => (
								<Badge key={perm} variant="outline" size="sm">
									{perm}
								</Badge>
							))
						) : (
							<Text size="sm" c="dimmed">
								None
							</Text>
						)}
					</Group>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Scopes
					</Text>
					<Group gap="xs" mt={4}>
						{plugin.scopes.length > 0 ? (
							plugin.scopes.map((scope) => (
								<Badge key={scope} variant="outline" size="sm" color="blue">
									{scope}
								</Badge>
							))
						) : (
							<Text size="sm" c="dimmed">
								None
							</Text>
						)}
					</Group>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Libraries
					</Text>
					<Group gap="xs" mt={4}>
						{plugin.libraryIds.length === 0 ? (
							<Badge variant="light" size="sm" color="gray">
								All Libraries
							</Badge>
						) : (
							plugin.libraryIds.map((libId) => {
								const lib = libraries.find((l) => l.id === libId);
								return (
									<Badge key={libId} variant="outline" size="sm" color="cyan">
										{lib?.name || libId}
									</Badge>
								);
							})
						)}
					</Group>
				</div>
			</Group>

			{plugin.manifest && (
				<>
					<Divider label="Manifest" labelPosition="left" />
					<Group gap="xl">
						<div>
							<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
								Version
							</Text>
							<Text size="sm">{plugin.manifest.version}</Text>
						</div>
						<div>
							<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
								Protocol
							</Text>
							<Text size="sm">v{plugin.manifest.protocolVersion}</Text>
						</div>
						{plugin.manifest.author && (
							<div>
								<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
									Author
								</Text>
								<Text size="sm">{plugin.manifest.author}</Text>
							</div>
						)}
					</Group>
					<Group gap="xs">
						{plugin.manifest.capabilities.metadataProvider && (
							<Badge color="teal" variant="light">
								Metadata Provider
							</Badge>
						)}
						{plugin.manifest.capabilities.userSyncProvider && (
							<Badge color="violet" variant="light">
								User Sync Provider
							</Badge>
						)}
					</Group>
				</>
			)}

			{plugin.disabledReason && (
				<Alert
					icon={<IconAlertCircle size={16} />}
					color="red"
					variant="outline"
				>
					<Text fw={500} c="red.4">
						Disabled Reason
					</Text>
					<Text size="sm" c="dimmed">
						{plugin.disabledReason}
					</Text>
				</Alert>
			)}

			<PluginFailureHistory pluginId={plugin.id} />
		</Stack>
	);
}

// Plugin failure history component
function PluginFailureHistory({ pluginId }: { pluginId: string }) {
	const [showAllModal, setShowAllModal] = useState(false);
	const [page, setPage] = useState(1);
	const pageSize = 5; // Show 5 recent failures inline
	const modalPageSize = 20;

	// Query for inline display (first 5)
	const { data, isLoading, error } = useQuery<PluginFailuresResponse>({
		queryKey: ["plugin-failures", pluginId, "inline"],
		queryFn: () => pluginsApi.getFailures(pluginId, pageSize, 0),
	});

	// Query for modal display (paginated)
	const { data: modalData, isLoading: modalLoading } =
		useQuery<PluginFailuresResponse>({
			queryKey: ["plugin-failures", pluginId, "modal", page],
			queryFn: () =>
				pluginsApi.getFailures(
					pluginId,
					modalPageSize,
					(page - 1) * modalPageSize,
				),
			enabled: showAllModal,
		});

	if (isLoading) {
		return (
			<Group justify="center" py="sm">
				<Loader size="sm" />
			</Group>
		);
	}

	if (error || !data) {
		return null;
	}

	if (data.failures.length === 0) {
		return null;
	}

	const totalPages = Math.ceil(
		(modalData?.total ?? data.total) / modalPageSize,
	);

	return (
		<>
			<Divider label="Failure History" labelPosition="left" />
			<Group gap="xl">
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Window Failures
					</Text>
					<Group gap="xs">
						<Text
							size="sm"
							fw={500}
							c={data.windowFailures >= data.threshold ? "red" : undefined}
						>
							{data.windowFailures} / {data.threshold}
						</Text>
						<Text size="xs" c="dimmed">
							(in {Math.round(data.windowSeconds / 60)} min)
						</Text>
					</Group>
				</div>
				<div>
					<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
						Total Recorded
					</Text>
					<Text size="sm">{data.total}</Text>
				</div>
				{data.total > pageSize && (
					<Button
						variant="light"
						size="xs"
						onClick={() => {
							setPage(1);
							setShowAllModal(true);
						}}
					>
						View All ({data.total})
					</Button>
				)}
			</Group>

			<Stack gap="xs">
				{data.failures.map((failure) => (
					<FailureCard key={failure.id} failure={failure} />
				))}
			</Stack>

			{/* View All Failures Modal */}
			<Modal
				opened={showAllModal}
				onClose={() => setShowAllModal(false)}
				title="Failure History"
				size="lg"
			>
				<Stack gap="md">
					<Group gap="xl">
						<div>
							<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
								Window Failures
							</Text>
							<Text size="sm" fw={500}>
								{data.windowFailures} / {data.threshold}
							</Text>
						</div>
						<div>
							<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
								Window Duration
							</Text>
							<Text size="sm">
								{Math.round(data.windowSeconds / 60)} minutes
							</Text>
						</div>
						<div>
							<Text size="xs" c="dimmed" tt="uppercase" fw={600}>
								Total Failures
							</Text>
							<Text size="sm">{modalData?.total ?? data.total}</Text>
						</div>
					</Group>

					<Divider />

					{modalLoading ? (
						<Group justify="center" py="xl">
							<Loader />
						</Group>
					) : (
						<ScrollArea.Autosize mah={400}>
							<Stack gap="xs">
								{modalData?.failures.map((failure) => (
									<FailureCard key={failure.id} failure={failure} showDetails />
								))}
							</Stack>
						</ScrollArea.Autosize>
					)}

					{totalPages > 1 && (
						<Group justify="center" mt="md">
							<Button
								variant="subtle"
								size="xs"
								disabled={page === 1}
								onClick={() => setPage((p) => Math.max(1, p - 1))}
							>
								Previous
							</Button>
							<Text size="sm">
								Page {page} of {totalPages}
							</Text>
							<Button
								variant="subtle"
								size="xs"
								disabled={page >= totalPages}
								onClick={() => setPage((p) => p + 1)}
							>
								Next
							</Button>
						</Group>
					)}
				</Stack>
			</Modal>
		</>
	);
}

// Individual failure card component
function FailureCard({
	failure,
	showDetails = false,
}: {
	failure: PluginFailuresResponse["failures"][0];
	showDetails?: boolean;
}) {
	return (
		<Card withBorder p="xs" radius="sm">
			<Stack gap="xs">
				<Group justify="space-between" wrap="nowrap">
					<Group gap="xs" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
						{failure.errorCode && (
							<Badge size="xs" color="red" variant="light">
								{failure.errorCode}
							</Badge>
						)}
						{failure.method && (
							<Badge size="xs" color="blue" variant="outline">
								{failure.method}
							</Badge>
						)}
						<Text
							size="xs"
							lineClamp={showDetails ? undefined : 1}
							style={{ flex: 1 }}
						>
							{failure.errorMessage}
						</Text>
					</Group>
					<Text size="xs" c="dimmed" style={{ flexShrink: 0 }}>
						{new Date(failure.occurredAt).toLocaleString()}
					</Text>
				</Group>
				{showDetails && failure.requestSummary && (
					<Box>
						<Text size="xs" c="dimmed" fw={600}>
							Request Summary:
						</Text>
						<Code block style={{ fontSize: "11px" }}>
							{failure.requestSummary}
						</Code>
					</Box>
				)}
			</Stack>
		</Card>
	);
}

// Plugin form component
interface PluginFormProps {
	form: ReturnType<typeof useForm<PluginFormValues>>;
	onSubmit: (values: PluginFormValues) => void;
	isLoading: boolean;
	onCancel: () => void;
	isCreate?: boolean;
	libraries: { id: string; name: string }[];
}

function PluginForm({
	form,
	onSubmit,
	isLoading,
	onCancel,
	isCreate,
	libraries,
}: PluginFormProps) {
	const [activeTab, setActiveTab] = useState<string | null>("general");
	const [nameManuallyEdited, setNameManuallyEdited] = useState(false);

	// Check which tabs have errors
	const generalTabErrors = isCreate
		? !!(form.errors.name || form.errors.displayName)
		: !!form.errors.displayName;
	const executionTabErrors = !!form.errors.command;

	// Handle form submission with tab navigation on error
	const handleSubmit = form.onSubmit(onSubmit, (errors) => {
		// Navigate to the first tab with errors
		if (isCreate && errors.name) {
			setActiveTab("general");
		} else if (errors.displayName) {
			setActiveTab("general");
		} else if (errors.command) {
			setActiveTab("execution");
		}
	});

	return (
		<form onSubmit={handleSubmit}>
			<Tabs value={activeTab} onChange={setActiveTab}>
				<Tabs.List>
					<Tabs.Tab value="general" c={generalTabErrors ? "red" : undefined}>
						General{generalTabErrors ? " *" : ""}
					</Tabs.Tab>
					<Tabs.Tab
						value="execution"
						c={executionTabErrors ? "red" : undefined}
					>
						Execution{executionTabErrors ? " *" : ""}
					</Tabs.Tab>
					<Tabs.Tab value="permissions">Permissions</Tabs.Tab>
					<Tabs.Tab value="credentials">Credentials</Tabs.Tab>
				</Tabs.List>

				<Box mt="md">
					<Tabs.Panel value="general">
						<Stack gap="md">
							{isCreate && (
								<TextInput
									label="Name"
									placeholder="mangabaka"
									description="Unique identifier (lowercase alphanumeric with hyphens)"
									withAsterisk
									{...form.getInputProps("name")}
									onChange={(e) => {
										const value = e.currentTarget.value;
										form.setFieldValue("name", value);
										setNameManuallyEdited(value.length > 0);
									}}
									onBlur={(e) => {
										form.setFieldValue(
											"name",
											normalizePluginName(e.currentTarget.value),
										);
									}}
								/>
							)}
							<TextInput
								label="Display Name"
								placeholder="MangaBaka"
								description="Human-readable name shown in the UI"
								withAsterisk
								{...form.getInputProps("displayName")}
								onChange={(e) => {
									const displayName = e.currentTarget.value;
									form.setFieldValue("displayName", displayName);
									// Auto-generate name from display name until user manually edits it (create mode only)
									if (isCreate && !nameManuallyEdited) {
										form.setFieldValue(
											"name",
											normalizePluginName(displayName),
										);
									}
								}}
							/>
							<Textarea
								label="Description"
								placeholder="Fetch manga metadata from MangaBaka (MangaUpdates)"
								description="Optional description of what this plugin does"
								rows={2}
								{...form.getInputProps("description")}
							/>
							{isCreate && (
								<Switch
									label="Enable immediately"
									description="Start the plugin after creation"
									{...form.getInputProps("enabled", { type: "checkbox" })}
								/>
							)}
						</Stack>
					</Tabs.Panel>

					<Tabs.Panel value="execution">
						<Stack gap="md">
							<TextInput
								label="Command"
								placeholder="node"
								description="The command to execute (e.g., node, python, npx)"
								withAsterisk
								{...form.getInputProps("command")}
							/>
							<Textarea
								label="Arguments"
								placeholder="/opt/codex/plugins/mangabaka/dist/index.js"
								description="Command arguments, one per line"
								rows={3}
								{...form.getInputProps("args")}
							/>
							<TextInput
								label="Working Directory"
								placeholder="/opt/codex/plugins/mangabaka"
								description="Optional working directory for the plugin process"
								{...form.getInputProps("workingDirectory")}
							/>
							<Textarea
								label="Configuration"
								placeholder='{"rate_limit": 60}'
								description="Optional JSON configuration passed to the plugin"
								rows={3}
								{...form.getInputProps("config")}
							/>
							<Divider label="Rate Limiting" labelPosition="center" />
							<Switch
								label="Enable Rate Limiting"
								description="Limit the number of requests per minute to protect external APIs"
								{...form.getInputProps("rateLimitEnabled", {
									type: "checkbox",
								})}
							/>
							{form.values.rateLimitEnabled && (
								<NumberInput
									label="Requests per Minute"
									description="Maximum number of requests allowed per minute"
									placeholder="60"
									min={1}
									max={1000}
									{...form.getInputProps("rateLimitRequestsPerMinute")}
								/>
							)}
						</Stack>
					</Tabs.Panel>

					<Tabs.Panel value="permissions">
						<Stack gap="md">
							<MultiSelect
								label="Permissions"
								placeholder="Select permissions"
								description="RBAC permissions controlling what the plugin can write"
								data={AVAILABLE_PERMISSIONS.map((p) => ({
									value: p.value,
									label: p.label,
								}))}
								searchable
								{...form.getInputProps("permissions")}
							/>
							<MultiSelect
								label="Scopes"
								placeholder="Select scopes"
								description="Where the plugin actions will be available in the UI"
								data={AVAILABLE_SCOPES.map((s) => ({
									value: s.value,
									label: s.label,
								}))}
								searchable
								{...form.getInputProps("scopes")}
							/>
							<Divider label="Library Filter" labelPosition="center" />
							<Switch
								label="All Libraries"
								description="When enabled, plugin applies to all libraries. Disable to select specific libraries."
								{...form.getInputProps("allLibraries", { type: "checkbox" })}
							/>
							{!form.values.allLibraries && (
								<MultiSelect
									label="Libraries"
									placeholder="Select libraries"
									description="Plugin will only be available for series/books in these libraries"
									data={libraries.map((lib) => ({
										value: lib.id,
										label: lib.name,
									}))}
									searchable
									{...form.getInputProps("libraryIds")}
								/>
							)}
						</Stack>
					</Tabs.Panel>

					<Tabs.Panel value="credentials">
						<Stack gap="md">
							<Select
								label="Credential Delivery"
								description="How credentials are passed to the plugin"
								data={CREDENTIAL_DELIVERY_OPTIONS.map((o) => ({
									value: o.value,
									label: o.label,
								}))}
								{...form.getInputProps("credentialDelivery")}
							/>
							<Textarea
								label="Credentials"
								placeholder='{"api_key": "your-api-key"}'
								description="JSON object with credentials (will be encrypted). Leave empty to keep existing credentials."
								rows={3}
								{...form.getInputProps("credentials")}
							/>
							<Alert
								icon={<IconAlertCircle size={16} />}
								color="yellow"
								variant="light"
							>
								Credentials are encrypted before storage and never displayed.
								When editing, leave the credentials field empty to keep existing
								values.
							</Alert>
						</Stack>
					</Tabs.Panel>
				</Box>
			</Tabs>

			<Group justify="flex-end" mt="xl">
				<Button variant="subtle" onClick={onCancel}>
					Cancel
				</Button>
				<Button type="submit" loading={isLoading}>
					{isCreate ? "Create Plugin" : "Save Changes"}
				</Button>
			</Group>
		</form>
	);
}
