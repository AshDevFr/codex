import {
	ActionIcon,
	Alert,
	Badge,
	Box,
	Button,
	Card,
	CopyButton,
	Group,
	Modal,
	PasswordInput,
	SegmentedControl,
	Select,
	Stack,
	Switch,
	Table,
	Tabs,
	Text,
	TextInput,
	Title,
	Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconCheck,
	IconCopy,
	IconKey,
	IconLink,
	IconPalette,
	IconPlus,
	IconTrash,
	IconUser,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/api/client";
import { userIntegrationsApi } from "@/api/userIntegrations";
import { userPreferencesApi } from "@/api/userPreferences";
import { useAppName } from "@/hooks/useAppName";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useAuthStore } from "@/store/authStore";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import type { components } from "@/types/api.generated";
import type { PreferenceKey, TypedPreferences } from "@/types/preferences";
import { PREFERENCE_DEFAULTS } from "@/types/preferences";

type ApiKeyDto = components["schemas"]["ApiKeyDto"];
type UserIntegrationDto = components["schemas"]["UserIntegrationDto"];
type AvailableIntegrationDto = components["schemas"]["AvailableIntegrationDto"];

export function ProfileSettings() {
	const appName = useAppName();
	const { user } = useAuthStore();
	const queryClient = useQueryClient();
	const { getPreference, setPreference } = useUserPreferencesStore();
	const [createKeyModalOpened, setCreateKeyModalOpened] = useState(false);
	const [newApiKey, setNewApiKey] = useState<string | null>(null);

	useDocumentTitle("Profile Settings");

	// Fetch user preferences
	const { data: preferences } = useQuery({
		queryKey: ["user-preferences"],
		queryFn: userPreferencesApi.getAll,
	});

	// Fetch API keys
	const { data: apiKeys, isLoading: apiKeysLoading } = useQuery({
		queryKey: ["api-keys"],
		queryFn: async () => {
			const response = await api.get<ApiKeyDto[]>("/api-keys");
			// Handle both array and object with data property
			const data = response.data;
			if (Array.isArray(data)) {
				return data;
			}
			// If it's an object with a data property (paginated response)
			if (data && typeof data === "object" && "data" in data) {
				return (data as { data: ApiKeyDto[] }).data;
			}
			return [];
		},
	});

	// Fetch user integrations
	const { data: integrationsData, isLoading: integrationsLoading } = useQuery({
		queryKey: ["user-integrations"],
		queryFn: userIntegrationsApi.getAll,
	});

	// Password change form
	const passwordForm = useForm({
		initialValues: {
			currentPassword: "",
			newPassword: "",
			confirmPassword: "",
		},
		validate: {
			newPassword: (value) =>
				value.length < 8 ? "Password must be at least 8 characters" : null,
			confirmPassword: (value, values) =>
				value !== values.newPassword ? "Passwords do not match" : null,
		},
	});

	// Create API key form
	const apiKeyForm = useForm({
		initialValues: {
			name: "",
			expiresInDays: 30,
		},
		validate: {
			name: (value) => (value.length < 1 ? "Name is required" : null),
		},
	});

	// Mutations
	const updatePreferenceMutation = useMutation({
		mutationFn: async ({
			key,
			value,
		}: {
			key: PreferenceKey;
			value: TypedPreferences[PreferenceKey];
		}) => {
			return userPreferencesApi.set(key, value as never);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["user-preferences"] });
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to update preference",
				color: "red",
			});
		},
	});

	const changePasswordMutation = useMutation({
		mutationFn: async (data: {
			currentPassword: string;
			newPassword: string;
		}) => {
			await api.post("/auth/change-password", data);
		},
		onSuccess: () => {
			notifications.show({
				title: "Success",
				message: "Password changed successfully",
				color: "green",
			});
			passwordForm.reset();
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message:
					"Failed to change password. Please check your current password.",
				color: "red",
			});
		},
	});

	const createApiKeyMutation = useMutation({
		mutationFn: async (data: { name: string; expiresInDays: number }) => {
			const response = await api.post<{ apiKey: ApiKeyDto; key: string }>(
				"/api-keys",
				{
					name: data.name,
					expiresAt: data.expiresInDays
						? new Date(
								Date.now() + data.expiresInDays * 24 * 60 * 60 * 1000,
							).toISOString()
						: null,
				},
			);
			return response.data;
		},
		onSuccess: (data) => {
			setNewApiKey(data.key);
			queryClient.invalidateQueries({ queryKey: ["api-keys"] });
			apiKeyForm.reset();
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to create API key",
				color: "red",
			});
		},
	});

	const deleteApiKeyMutation = useMutation({
		mutationFn: async (keyId: string) => {
			await api.delete(`/api-keys/${keyId}`);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["api-keys"] });
			notifications.show({
				title: "Success",
				message: "API key deleted",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to delete API key",
				color: "red",
			});
		},
	});

	const disconnectIntegrationMutation = useMutation({
		mutationFn: async (name: string) => {
			await userIntegrationsApi.disconnect(name);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["user-integrations"] });
			notifications.show({
				title: "Success",
				message: "Integration disconnected",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to disconnect integration",
				color: "red",
			});
		},
	});

	// Helper to get preference value with default
	const getPref = <K extends PreferenceKey>(key: K): TypedPreferences[K] => {
		const cached = getPreference(key);
		if (cached !== undefined) return cached;

		const serverPref = preferences?.find((p) => p.key === key);
		if (serverPref) {
			return serverPref.value as TypedPreferences[K];
		}
		return PREFERENCE_DEFAULTS[key];
	};

	// Helper to update preference
	const updatePref = <K extends PreferenceKey>(
		key: K,
		value: TypedPreferences[K],
	) => {
		setPreference(key, value);
		updatePreferenceMutation.mutate({ key, value });
	};

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Profile Settings</Title>

				<Tabs defaultValue="account">
					<Tabs.List>
						<Tabs.Tab value="account" leftSection={<IconUser size={16} />}>
							Account
						</Tabs.Tab>
						<Tabs.Tab
							value="preferences"
							leftSection={<IconPalette size={16} />}
						>
							Preferences
						</Tabs.Tab>
						<Tabs.Tab value="integrations" leftSection={<IconLink size={16} />}>
							Integrations
						</Tabs.Tab>
						<Tabs.Tab value="api-keys" leftSection={<IconKey size={16} />}>
							API Keys
						</Tabs.Tab>
					</Tabs.List>

					{/* Account Tab */}
					<Tabs.Panel value="account" pt="md">
						<Stack gap="lg">
							<Card withBorder>
								<Stack gap="md">
									<Title order={3}>Account Information</Title>
									<Group>
										<Text fw={500}>Username:</Text>
										<Text>{user?.username}</Text>
									</Group>
									<Group>
										<Text fw={500}>Email:</Text>
										<Text>{user?.email}</Text>
									</Group>
									<Group>
										<Text fw={500}>Role:</Text>
										<Badge color={user?.isAdmin ? "blue" : "gray"}>
											{user?.isAdmin ? "Admin" : "User"}
										</Badge>
									</Group>
								</Stack>
							</Card>

							<Card withBorder>
								<form
									onSubmit={passwordForm.onSubmit((values) =>
										changePasswordMutation.mutate({
											currentPassword: values.currentPassword,
											newPassword: values.newPassword,
										}),
									)}
								>
									<Stack gap="md">
										<Title order={3}>Change Password</Title>
										<PasswordInput
											label="Current Password"
											placeholder="Enter current password"
											{...passwordForm.getInputProps("currentPassword")}
										/>
										<PasswordInput
											label="New Password"
											placeholder="Enter new password"
											{...passwordForm.getInputProps("newPassword")}
										/>
										<PasswordInput
											label="Confirm New Password"
											placeholder="Confirm new password"
											{...passwordForm.getInputProps("confirmPassword")}
										/>
										<Group>
											<Button
												type="submit"
												loading={changePasswordMutation.isPending}
											>
												Change Password
											</Button>
										</Group>
									</Stack>
								</form>
							</Card>
						</Stack>
					</Tabs.Panel>

					{/* Preferences Tab */}
					<Tabs.Panel value="preferences" pt="md">
						<Stack gap="lg">
							<Card withBorder>
								<Stack gap="md">
									<Title order={3}>Appearance</Title>
									<Group justify="space-between">
										<div>
											<Text fw={500}>Theme</Text>
											<Text size="sm" c="dimmed">
												Choose your preferred color theme
											</Text>
										</div>
										<SegmentedControl
											value={getPref("ui.theme")}
											onChange={(value) =>
												updatePref(
													"ui.theme",
													value as "light" | "dark" | "system",
												)
											}
											data={[
												{ label: "Light", value: "light" },
												{ label: "Dark", value: "dark" },
												{ label: "System", value: "system" },
											]}
										/>
									</Group>
								</Stack>
							</Card>

							<Card withBorder>
								<Stack gap="md">
									<Title order={3}>Library Display</Title>
									<Group justify="space-between">
										<div>
											<Text fw={500}>Show Deleted Books</Text>
											<Text size="sm" c="dimmed">
												Display soft-deleted books in the library
											</Text>
										</div>
										<Switch
											checked={getPref("library.show_deleted_books")}
											onChange={(e) =>
												updatePref(
													"library.show_deleted_books",
													e.currentTarget.checked,
												)
											}
										/>
									</Group>
								</Stack>
							</Card>
						</Stack>
					</Tabs.Panel>

					{/* Integrations Tab */}
					<Tabs.Panel value="integrations" pt="md">
						<Stack gap="lg">
							{integrationsLoading ? (
								<Text>Loading integrations...</Text>
							) : (
								<>
									{/* Connected Integrations */}
									<Card withBorder>
										<Stack gap="md">
											<Title order={3}>Connected Integrations</Title>
											{integrationsData?.integrations &&
											integrationsData.integrations.length > 0 ? (
												<Table>
													<Table.Thead>
														<Table.Tr>
															<Table.Th>Service</Table.Th>
															<Table.Th>Status</Table.Th>
															<Table.Th>Last Sync</Table.Th>
															<Table.Th>Actions</Table.Th>
														</Table.Tr>
													</Table.Thead>
													<Table.Tbody>
														{integrationsData.integrations.map(
															(integration: UserIntegrationDto) => (
																<Table.Tr key={integration.integrationName}>
																	<Table.Td>
																		<Group gap="xs">
																			<Text fw={500}>
																				{integration.displayName ||
																					integration.integrationName}
																			</Text>
																			{integration.externalUsername && (
																				<Text size="sm" c="dimmed">
																					(@{integration.externalUsername})
																				</Text>
																			)}
																		</Group>
																	</Table.Td>
																	<Table.Td>
																		<Badge
																			color={
																				integration.enabled ? "green" : "gray"
																			}
																		>
																			{integration.enabled
																				? "Enabled"
																				: "Disabled"}
																		</Badge>
																	</Table.Td>
																	<Table.Td>
																		{integration.lastSyncAt
																			? new Date(
																					integration.lastSyncAt,
																				).toLocaleString()
																			: "Never"}
																	</Table.Td>
																	<Table.Td>
																		<Button
																			size="xs"
																			color="red"
																			variant="light"
																			onClick={() =>
																				disconnectIntegrationMutation.mutate(
																					integration.integrationName,
																				)
																			}
																			loading={
																				disconnectIntegrationMutation.isPending
																			}
																		>
																			Disconnect
																		</Button>
																	</Table.Td>
																</Table.Tr>
															),
														)}
													</Table.Tbody>
												</Table>
											) : (
												<Text c="dimmed">No integrations connected yet.</Text>
											)}
										</Stack>
									</Card>

									{/* Available Integrations */}
									<Card withBorder>
										<Stack gap="md">
											<Title order={3}>Available Integrations</Title>
											<Alert icon={<IconAlertCircle size={16} />} color="blue">
												Integration providers are coming soon. Connect your
												AniList, MyAnimeList, and other accounts to sync your
												reading progress.
											</Alert>
											{integrationsData?.available &&
												integrationsData.available.length > 0 && (
													<Stack gap="sm">
														{integrationsData.available.map(
															(available: AvailableIntegrationDto) => (
																<Card
																	key={available.name}
																	withBorder
																	padding="sm"
																>
																	<Group justify="space-between">
																		<div>
																			<Text fw={500}>
																				{available.displayName}
																			</Text>
																			<Text size="sm" c="dimmed">
																				{available.description}
																			</Text>
																			<Group gap="xs" mt="xs">
																				{available.features.map((feature) => (
																					<Badge
																						key={feature}
																						size="xs"
																						variant="light"
																					>
																						{feature.replace(/_/g, " ")}
																					</Badge>
																				))}
																			</Group>
																		</div>
																		<Button variant="light" disabled size="sm">
																			Connect
																		</Button>
																	</Group>
																</Card>
															),
														)}
													</Stack>
												)}
										</Stack>
									</Card>
								</>
							)}
						</Stack>
					</Tabs.Panel>

					{/* API Keys Tab */}
					<Tabs.Panel value="api-keys" pt="md">
						<Stack gap="lg">
							<Card withBorder>
								<Stack gap="md">
									<Group justify="space-between">
										<Title order={3}>API Keys</Title>
										<Button
											leftSection={<IconPlus size={16} />}
											onClick={() => setCreateKeyModalOpened(true)}
										>
											Create Key
										</Button>
									</Group>
									<Text size="sm" c="dimmed">
										API keys allow external applications to access your {appName}{" "}
										account. Keep them secure and never share them publicly.
									</Text>
									{apiKeysLoading ? (
										<Text>Loading API keys...</Text>
									) : Array.isArray(apiKeys) && apiKeys.length > 0 ? (
										<Table>
											<Table.Thead>
												<Table.Tr>
													<Table.Th>Name</Table.Th>
													<Table.Th>Created</Table.Th>
													<Table.Th>Expires</Table.Th>
													<Table.Th>Last Used</Table.Th>
													<Table.Th>Actions</Table.Th>
												</Table.Tr>
											</Table.Thead>
											<Table.Tbody>
												{apiKeys.map((key: ApiKeyDto) => (
													<Table.Tr key={key.id}>
														<Table.Td>
															<Text fw={500}>{key.name}</Text>
														</Table.Td>
														<Table.Td>
															{new Date(key.createdAt).toLocaleDateString()}
														</Table.Td>
														<Table.Td>
															{key.expiresAt
																? new Date(key.expiresAt).toLocaleDateString()
																: "Never"}
														</Table.Td>
														<Table.Td>
															{key.lastUsedAt
																? new Date(key.lastUsedAt).toLocaleString()
																: "Never"}
														</Table.Td>
														<Table.Td>
															<ActionIcon
																color="red"
																variant="light"
																onClick={() =>
																	deleteApiKeyMutation.mutate(key.id)
																}
																loading={deleteApiKeyMutation.isPending}
															>
																<IconTrash size={16} />
															</ActionIcon>
														</Table.Td>
													</Table.Tr>
												))}
											</Table.Tbody>
										</Table>
									) : (
										<Text c="dimmed">No API keys created yet.</Text>
									)}
								</Stack>
							</Card>
						</Stack>
					</Tabs.Panel>
				</Tabs>
			</Stack>

			{/* Create API Key Modal */}
			<Modal
				opened={createKeyModalOpened}
				onClose={() => {
					setCreateKeyModalOpened(false);
					setNewApiKey(null);
					apiKeyForm.reset();
				}}
				title="Create API Key"
			>
				{newApiKey ? (
					<Stack gap="md">
						<Alert icon={<IconCheck size={16} />} color="green">
							API key created successfully!
						</Alert>
						<Text size="sm" c="dimmed">
							Copy this key now. You won't be able to see it again.
						</Text>
						<Group>
							<TextInput
								value={newApiKey}
								readOnly
								style={{ flex: 1, fontFamily: "monospace" }}
							/>
							<CopyButton value={newApiKey}>
								{({ copied, copy }) => (
									<Tooltip label={copied ? "Copied" : "Copy"}>
										<ActionIcon
											color={copied ? "green" : "gray"}
											onClick={copy}
										>
											{copied ? (
												<IconCheck size={16} />
											) : (
												<IconCopy size={16} />
											)}
										</ActionIcon>
									</Tooltip>
								)}
							</CopyButton>
						</Group>
						<Button
							onClick={() => {
								setCreateKeyModalOpened(false);
								setNewApiKey(null);
							}}
						>
							Done
						</Button>
					</Stack>
				) : (
					<form
						onSubmit={apiKeyForm.onSubmit((values) =>
							createApiKeyMutation.mutate(values),
						)}
					>
						<Stack gap="md">
							<TextInput
								label="Key Name"
								placeholder="My API Key"
								description="A name to identify this key"
								{...apiKeyForm.getInputProps("name")}
							/>
							<Select
								label="Expiration"
								description="When this key should expire"
								data={[
									{ label: "7 days", value: "7" },
									{ label: "30 days", value: "30" },
									{ label: "90 days", value: "90" },
									{ label: "1 year", value: "365" },
									{ label: "Never", value: "0" },
								]}
								value={String(apiKeyForm.values.expiresInDays)}
								onChange={(value) =>
									apiKeyForm.setFieldValue(
										"expiresInDays",
										Number.parseInt(value || "30", 10),
									)
								}
							/>
							<Group justify="flex-end">
								<Button
									variant="subtle"
									onClick={() => setCreateKeyModalOpened(false)}
								>
									Cancel
								</Button>
								<Button type="submit" loading={createApiKeyMutation.isPending}>
									Create Key
								</Button>
							</Group>
						</Stack>
					</form>
				)}
			</Modal>
		</Box>
	);
}
