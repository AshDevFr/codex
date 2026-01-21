import {
	ActionIcon,
	Alert,
	Anchor,
	Badge,
	Box,
	Button,
	Card,
	Group,
	Loader,
	Modal,
	Stack,
	Table,
	Text,
	Textarea,
	TextInput,
	Title,
	Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconEdit,
	IconPlus,
	IconShare,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import { type SharingTagDto, sharingTagsApi } from "@/api/sharingTags";

export function SharingTagsSettings() {
	const queryClient = useQueryClient();
	const [createModalOpened, setCreateModalOpened] = useState(false);
	const [editModalOpened, setEditModalOpened] = useState(false);
	const [deleteModalOpened, setDeleteModalOpened] = useState(false);
	const [selectedTag, setSelectedTag] = useState<SharingTagDto | null>(null);

	// Fetch sharing tags
	const {
		data: sharingTags,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["sharing-tags"],
		queryFn: sharingTagsApi.list,
	});

	// Create form
	const createForm = useForm({
		initialValues: {
			name: "",
			description: "",
		},
		validate: {
			name: (value) =>
				value.trim().length < 1 ? "Name is required" : null,
		},
	});

	// Edit form
	const editForm = useForm({
		initialValues: {
			name: "",
			description: "",
		},
		validate: {
			name: (value) =>
				value.trim().length < 1 ? "Name is required" : null,
		},
	});

	// Mutations
	const createMutation = useMutation({
		mutationFn: async (data: { name: string; description: string }) => {
			return sharingTagsApi.create({
				name: data.name.trim(),
				description: data.description.trim() || null,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["sharing-tags"] });
			setCreateModalOpened(false);
			createForm.reset();
			notifications.show({
				title: "Success",
				message: "Sharing tag created successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to create sharing tag",
				color: "red",
			});
		},
	});

	const updateMutation = useMutation({
		mutationFn: async ({
			tagId,
			data,
		}: {
			tagId: string;
			data: { name: string; description: string };
		}) => {
			return sharingTagsApi.update(tagId, {
				name: data.name.trim(),
				description: data.description.trim() || null,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["sharing-tags"] });
			setEditModalOpened(false);
			setSelectedTag(null);
			notifications.show({
				title: "Success",
				message: "Sharing tag updated successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update sharing tag",
				color: "red",
			});
		},
	});

	const deleteMutation = useMutation({
		mutationFn: async (tagId: string) => {
			return sharingTagsApi.delete(tagId);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["sharing-tags"] });
			setDeleteModalOpened(false);
			setSelectedTag(null);
			notifications.show({
				title: "Success",
				message: "Sharing tag deleted successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to delete sharing tag",
				color: "red",
			});
		},
	});

	const handleEditTag = (tag: SharingTagDto) => {
		setSelectedTag(tag);
		editForm.setValues({
			name: tag.name,
			description: tag.description || "",
		});
		setEditModalOpened(true);
	};

	const handleDeleteTag = (tag: SharingTagDto) => {
		setSelectedTag(tag);
		setDeleteModalOpened(true);
	};

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<div>
						<Title order={1}>Sharing Tags</Title>
						<Text c="dimmed" size="sm" mt="xs">
							Manage content sharing tags for controlling series visibility to users
						</Text>
					</div>
					<Button
						leftSection={<IconPlus size={16} />}
						onClick={() => setCreateModalOpened(true)}
					>
						Create Tag
					</Button>
				</Group>

				{isLoading ? (
					<Group justify="center" py="xl">
						<Loader />
					</Group>
				) : error ? (
					<Alert icon={<IconAlertCircle size={16} />} color="red">
						Failed to load sharing tags. Please try again.
					</Alert>
				) : sharingTags && sharingTags.length > 0 ? (
					<Card withBorder>
						<Table>
							<Table.Thead>
								<Table.Tr>
									<Table.Th>Tag</Table.Th>
									<Table.Th>Description</Table.Th>
									<Table.Th>Series</Table.Th>
									<Table.Th>Users</Table.Th>
									<Table.Th>Created</Table.Th>
									<Table.Th>Actions</Table.Th>
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{sharingTags.map((tag) => (
									<Table.Tr key={tag.id}>
										<Table.Td>
											<Group gap="sm">
												<IconShare size={20} />
												<Text fw={500}>{tag.name}</Text>
											</Group>
										</Table.Td>
										<Table.Td>
											<Text size="sm" c={tag.description ? undefined : "dimmed"}>
												{tag.description || "No description"}
											</Text>
										</Table.Td>
										<Table.Td>
											<Anchor
												component={Link}
												to={`/libraries/all/series?stf=any:${encodeURIComponent(tag.name)}`}
												underline="never"
											>
												<Badge
													variant="light"
													color="blue"
													style={{ cursor: "pointer" }}
												>
													{tag.seriesCount} series
												</Badge>
											</Anchor>
										</Table.Td>
										<Table.Td>
											<Anchor
												component={Link}
												to={`/settings/users?sharingTag=${encodeURIComponent(tag.name)}`}
												underline="never"
											>
												<Badge
													variant="light"
													color="green"
													style={{ cursor: "pointer" }}
												>
													{tag.userCount} users
												</Badge>
											</Anchor>
										</Table.Td>
										<Table.Td>
											{new Date(tag.createdAt).toLocaleDateString()}
										</Table.Td>
										<Table.Td>
											<Group gap="xs">
												<Tooltip label="Edit Tag">
													<ActionIcon
														variant="subtle"
														onClick={() => handleEditTag(tag)}
													>
														<IconEdit size={16} />
													</ActionIcon>
												</Tooltip>
												<Tooltip label="Delete Tag">
													<ActionIcon
														variant="subtle"
														color="red"
														onClick={() => handleDeleteTag(tag)}
													>
														<IconTrash size={16} />
													</ActionIcon>
												</Tooltip>
											</Group>
										</Table.Td>
									</Table.Tr>
								))}
							</Table.Tbody>
						</Table>
					</Card>
				) : (
					<Alert icon={<IconShare size={16} />} color="gray" variant="light">
						<Text fw={500}>No sharing tags yet</Text>
						<Text size="sm" mt="xs">
							Create sharing tags to control which series are visible to specific users.
							Tags can be assigned to series and then granted to users with "allow" or "deny" access modes.
						</Text>
					</Alert>
				)}
			</Stack>

			{/* Create Tag Modal */}
			<Modal
				opened={createModalOpened}
				onClose={() => {
					setCreateModalOpened(false);
					createForm.reset();
				}}
				title="Create Sharing Tag"
			>
				<form
					onSubmit={createForm.onSubmit((values) =>
						createMutation.mutate(values),
					)}
				>
					<Stack gap="md">
						<TextInput
							label="Name"
							placeholder="e.g., Kids Content, Adult Only"
							description="A unique name for this sharing tag"
							required
							{...createForm.getInputProps("name")}
						/>
						<Textarea
							label="Description"
							placeholder="Optional description for this tag"
							description="Help admins understand when to use this tag"
							rows={3}
							{...createForm.getInputProps("description")}
						/>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => {
									setCreateModalOpened(false);
									createForm.reset();
								}}
							>
								Cancel
							</Button>
							<Button type="submit" loading={createMutation.isPending}>
								Create Tag
							</Button>
						</Group>
					</Stack>
				</form>
			</Modal>

			{/* Edit Tag Modal */}
			<Modal
				opened={editModalOpened}
				onClose={() => {
					setEditModalOpened(false);
					setSelectedTag(null);
				}}
				title={`Edit Tag: ${selectedTag?.name}`}
			>
				<form
					onSubmit={editForm.onSubmit((values) => {
						if (selectedTag) {
							updateMutation.mutate({
								tagId: selectedTag.id,
								data: values,
							});
						}
					})}
				>
					<Stack gap="md">
						<TextInput
							label="Name"
							placeholder="e.g., Kids Content, Adult Only"
							required
							{...editForm.getInputProps("name")}
						/>
						<Textarea
							label="Description"
							placeholder="Optional description for this tag"
							rows={3}
							{...editForm.getInputProps("description")}
						/>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => {
									setEditModalOpened(false);
									setSelectedTag(null);
								}}
							>
								Cancel
							</Button>
							<Button type="submit" loading={updateMutation.isPending}>
								Save Changes
							</Button>
						</Group>
					</Stack>
				</form>
			</Modal>

			{/* Delete Tag Modal */}
			<Modal
				opened={deleteModalOpened}
				onClose={() => {
					setDeleteModalOpened(false);
					setSelectedTag(null);
				}}
				title="Delete Sharing Tag"
			>
				<Stack gap="md">
					<Text>
						Are you sure you want to delete the sharing tag{" "}
						<strong>{selectedTag?.name}</strong>?
					</Text>
					{selectedTag && (selectedTag.seriesCount > 0 || selectedTag.userCount > 0) && (
						<Alert icon={<IconAlertCircle size={16} />} color="yellow">
							This tag is currently assigned to {selectedTag.seriesCount} series
							and {selectedTag.userCount} users. Deleting it will remove all these
							associations.
						</Alert>
					)}
					<Text size="sm" c="dimmed">
						This action cannot be undone.
					</Text>
					<Group justify="flex-end">
						<Button
							variant="subtle"
							onClick={() => {
								setDeleteModalOpened(false);
								setSelectedTag(null);
							}}
						>
							Cancel
						</Button>
						<Button
							color="red"
							loading={deleteMutation.isPending}
							onClick={() =>
								selectedTag && deleteMutation.mutate(selectedTag.id)
							}
						>
							Delete Tag
						</Button>
					</Group>
				</Stack>
			</Modal>
		</Box>
	);
}
