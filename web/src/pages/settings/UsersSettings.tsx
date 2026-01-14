import {
	ActionIcon,
	Alert,
	Badge,
	Box,
	Button,
	Card,
	Group,
	Loader,
	Modal,
	PasswordInput,
	Stack,
	Switch,
	Table,
	Text,
	TextInput,
	Title,
	Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconEdit,
	IconTrash,
	IconUser,
	IconUserPlus,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type UserDto, usersApi } from "@/api/users";
import { useAuthStore } from "@/store/authStore";

export function UsersSettings() {
	const queryClient = useQueryClient();
	const { user: currentUser } = useAuthStore();
	const [createModalOpened, setCreateModalOpened] = useState(false);
	const [editModalOpened, setEditModalOpened] = useState(false);
	const [deleteModalOpened, setDeleteModalOpened] = useState(false);
	const [selectedUser, setSelectedUser] = useState<UserDto | null>(null);

	// Fetch users
	const {
		data: users,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["users"],
		queryFn: usersApi.list,
	});

	// Create user form
	const createForm = useForm({
		initialValues: {
			username: "",
			email: "",
			password: "",
			isAdmin: false,
		},
		validate: {
			username: (value) =>
				value.length < 3 ? "Username must be at least 3 characters" : null,
			email: (value) =>
				!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
					? "Invalid email address"
					: null,
			password: (value) =>
				value.length < 8 ? "Password must be at least 8 characters" : null,
		},
	});

	// Edit user form
	const editForm = useForm({
		initialValues: {
			username: "",
			email: "",
			password: "",
			isAdmin: false,
			isActive: true,
		},
	});

	// Mutations
	const createUserMutation = useMutation({
		mutationFn: async (data: {
			username: string;
			email: string;
			password: string;
			isAdmin: boolean;
		}) => {
			return usersApi.create({
				username: data.username,
				email: data.email,
				password: data.password,
				isAdmin: data.isAdmin,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["users"] });
			setCreateModalOpened(false);
			createForm.reset();
			notifications.show({
				title: "Success",
				message: "User created successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to create user",
				color: "red",
			});
		},
	});

	const updateUserMutation = useMutation({
		mutationFn: async ({
			userId,
			data,
		}: {
			userId: string;
			data: {
				username?: string;
				email?: string;
				password?: string;
				isAdmin?: boolean;
				isActive?: boolean;
			};
		}) => {
			return usersApi.update(userId, {
				username: data.username,
				email: data.email,
				password: data.password || undefined,
				isAdmin: data.isAdmin,
				isActive: data.isActive,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["users"] });
			setEditModalOpened(false);
			setSelectedUser(null);
			notifications.show({
				title: "Success",
				message: "User updated successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update user",
				color: "red",
			});
		},
	});

	const deleteUserMutation = useMutation({
		mutationFn: async (userId: string) => {
			return usersApi.delete(userId);
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["users"] });
			setDeleteModalOpened(false);
			setSelectedUser(null);
			notifications.show({
				title: "Success",
				message: "User deleted successfully",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to delete user",
				color: "red",
			});
		},
	});

	const handleEditUser = (user: UserDto) => {
		setSelectedUser(user);
		editForm.setValues({
			username: user.username,
			email: user.email,
			password: "",
			isAdmin: user.isAdmin,
			isActive: user.isActive,
		});
		setEditModalOpened(true);
	};

	const handleDeleteUser = (user: UserDto) => {
		setSelectedUser(user);
		setDeleteModalOpened(true);
	};

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<Title order={1}>User Management</Title>
					<Button
						leftSection={<IconUserPlus size={16} />}
						onClick={() => setCreateModalOpened(true)}
					>
						Create User
					</Button>
				</Group>

				{isLoading ? (
					<Group justify="center" py="xl">
						<Loader />
					</Group>
				) : error ? (
					<Alert icon={<IconAlertCircle size={16} />} color="red">
						Failed to load users. Please try again.
					</Alert>
				) : (
					<Card withBorder>
						<Table>
							<Table.Thead>
								<Table.Tr>
									<Table.Th>User</Table.Th>
									<Table.Th>Email</Table.Th>
									<Table.Th>Role</Table.Th>
									<Table.Th>Status</Table.Th>
									<Table.Th>Created</Table.Th>
									<Table.Th>Last Login</Table.Th>
									<Table.Th>Actions</Table.Th>
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{users?.map((user: UserDto) => (
									<Table.Tr key={user.id}>
										<Table.Td>
											<Group gap="sm">
												<IconUser size={20} />
												<div>
													<Text fw={500}>{user.username}</Text>
													{user.id === currentUser?.id && (
														<Text size="xs" c="dimmed">
															(You)
														</Text>
													)}
												</div>
											</Group>
										</Table.Td>
										<Table.Td>{user.email}</Table.Td>
										<Table.Td>
											<Badge color={user.isAdmin ? "blue" : "gray"}>
												{user.isAdmin ? "Admin" : "User"}
											</Badge>
										</Table.Td>
										<Table.Td>
											<Badge color={user.isActive ? "green" : "red"}>
												{user.isActive ? "Active" : "Inactive"}
											</Badge>
										</Table.Td>
										<Table.Td>
											{new Date(user.createdAt).toLocaleDateString()}
										</Table.Td>
										<Table.Td>
											{user.lastLoginAt
												? new Date(user.lastLoginAt).toLocaleString()
												: "Never"}
										</Table.Td>
										<Table.Td>
											<Group gap="xs">
												<Tooltip label="Edit User">
													<ActionIcon
														variant="subtle"
														onClick={() => handleEditUser(user)}
													>
														<IconEdit size={16} />
													</ActionIcon>
												</Tooltip>
												<Tooltip label="Delete User">
													<ActionIcon
														variant="subtle"
														color="red"
														onClick={() => handleDeleteUser(user)}
														disabled={user.id === currentUser?.id}
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
				)}
			</Stack>

			{/* Create User Modal */}
			<Modal
				opened={createModalOpened}
				onClose={() => {
					setCreateModalOpened(false);
					createForm.reset();
				}}
				title="Create User"
			>
				<form
					onSubmit={createForm.onSubmit((values) =>
						createUserMutation.mutate(values),
					)}
				>
					<Stack gap="md">
						<TextInput
							label="Username"
							placeholder="johndoe"
							{...createForm.getInputProps("username")}
						/>
						<TextInput
							label="Email"
							placeholder="john@example.com"
							{...createForm.getInputProps("email")}
						/>
						<PasswordInput
							label="Password"
							placeholder="Enter password"
							{...createForm.getInputProps("password")}
						/>
						<Switch
							label="Admin privileges"
							description="Admin users can manage server settings and other users"
							{...createForm.getInputProps("isAdmin", { type: "checkbox" })}
						/>
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setCreateModalOpened(false)}
							>
								Cancel
							</Button>
							<Button type="submit" loading={createUserMutation.isPending}>
								Create User
							</Button>
						</Group>
					</Stack>
				</form>
			</Modal>

			{/* Edit User Modal */}
			<Modal
				opened={editModalOpened}
				onClose={() => {
					setEditModalOpened(false);
					setSelectedUser(null);
				}}
				title={`Edit User: ${selectedUser?.username}`}
			>
				<form
					onSubmit={editForm.onSubmit((values) => {
						if (selectedUser) {
							updateUserMutation.mutate({
								userId: selectedUser.id,
								data: values,
							});
						}
					})}
				>
					<Stack gap="md">
						<TextInput
							label="Username"
							placeholder="johndoe"
							{...editForm.getInputProps("username")}
						/>
						<TextInput
							label="Email"
							placeholder="john@example.com"
							{...editForm.getInputProps("email")}
						/>
						<PasswordInput
							label="New Password"
							placeholder="Leave blank to keep current password"
							{...editForm.getInputProps("password")}
						/>
						<Switch
							label="Admin privileges"
							description="Admin users can manage server settings and other users"
							{...editForm.getInputProps("isAdmin", { type: "checkbox" })}
							disabled={selectedUser?.id === currentUser?.id}
						/>
						<Switch
							label="Active"
							description="Inactive users cannot log in"
							{...editForm.getInputProps("isActive", { type: "checkbox" })}
							disabled={selectedUser?.id === currentUser?.id}
						/>
						{selectedUser?.id === currentUser?.id && (
							<Alert icon={<IconAlertCircle size={16} />} color="yellow">
								You cannot change your own admin status or deactivate your own
								account.
							</Alert>
						)}
						<Group justify="flex-end">
							<Button
								variant="subtle"
								onClick={() => setEditModalOpened(false)}
							>
								Cancel
							</Button>
							<Button type="submit" loading={updateUserMutation.isPending}>
								Save Changes
							</Button>
						</Group>
					</Stack>
				</form>
			</Modal>

			{/* Delete User Modal */}
			<Modal
				opened={deleteModalOpened}
				onClose={() => {
					setDeleteModalOpened(false);
					setSelectedUser(null);
				}}
				title="Delete User"
			>
				<Stack gap="md">
					<Text>
						Are you sure you want to delete the user{" "}
						<strong>{selectedUser?.username}</strong>?
					</Text>
					<Text size="sm" c="dimmed">
						This action cannot be undone. All data associated with this user
						(reading progress, ratings, preferences) will be permanently
						deleted.
					</Text>
					<Group justify="flex-end">
						<Button
							variant="subtle"
							onClick={() => setDeleteModalOpened(false)}
						>
							Cancel
						</Button>
						<Button
							color="red"
							loading={deleteUserMutation.isPending}
							onClick={() =>
								selectedUser && deleteUserMutation.mutate(selectedUser.id)
							}
						>
							Delete User
						</Button>
					</Group>
				</Stack>
			</Modal>
		</Box>
	);
}
