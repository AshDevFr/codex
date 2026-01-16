import {
	Alert,
	Anchor,
	Box,
	Button,
	Container,
	Paper,
	PasswordInput,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { IconAlertCircle } from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { authApi } from "@/api/auth";
import { setupApi } from "@/api/setup";
import { useAppName } from "@/hooks/useAppName";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useAuthStore } from "@/store/authStore";
import type { ApiError, LoginRequest, LoginResponse } from "@/types";

export function Login() {
	const navigate = useNavigate();
	const appName = useAppName();
	const [username, setUsername] = useState("");
	const [password, setPassword] = useState("");
	const { setAuth } = useAuthStore();

	useDocumentTitle("Login");

	const { data: setupStatus } = useQuery({
		queryKey: ["setup-status"],
		queryFn: setupApi.checkStatus,
		staleTime: 5 * 60 * 1000, // Cache for 5 minutes
	});

	const loginMutation = useMutation<LoginResponse, ApiError, LoginRequest>({
		mutationFn: authApi.login,
		onSuccess: (data) => {
			setAuth(data.user, data.accessToken);
			navigate("/");
		},
	});

	const handleSubmit = (e: React.FormEvent) => {
		e.preventDefault();
		loginMutation.mutate({ username, password });
	};

	return (
		<Box bg="dark.7" mih="100vh">
			<Container size={420} py={100}>
				<Title ta="center" mb="xl">
					Welcome to {appName}
				</Title>

				<Paper shadow="md" p={30} radius="md" bg="dark.6">
					<form onSubmit={handleSubmit}>
						<Stack>
							<TextInput
								label="Username"
								placeholder="Your username"
								required
								value={username}
								onChange={(e) => setUsername(e.currentTarget.value)}
							/>

							<PasswordInput
								label="Password"
								placeholder="Your password"
								required
								value={password}
								onChange={(e) => setPassword(e.currentTarget.value)}
							/>

							{loginMutation.isError && (
								<Alert icon={<IconAlertCircle size={16} />} color="red">
									{loginMutation.error?.error || "Login failed"}
								</Alert>
							)}

							<Button type="submit" fullWidth loading={loginMutation.isPending}>
								Sign in
							</Button>
						</Stack>
					</form>

					{setupStatus?.registrationEnabled && (
						<Text c="dimmed" size="sm" ta="center" mt="md">
							Don't have an account?{" "}
							<Anchor component={Link} to="/register" size="sm">
								Create one
							</Anchor>
						</Text>
					)}
				</Paper>
			</Container>
		</Box>
	);
}
