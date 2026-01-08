import {
	Alert,
	Button,
	Collapse,
	Container,
	Group,
	NumberInput,
	Paper,
	PasswordInput,
	Stack,
	Stepper,
	Switch,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { IconAlertCircle, IconCheck } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { setupApi } from "@/api/setup";
import { useAuthStore } from "@/store/authStore";
import type {
	ApiError,
	ConfigureSettingsRequest,
	InitializeSetupRequest,
	SetupStatusResponse,
} from "@/types/api";

// Password validation utilities
const validatePassword = (password: string) => {
	const errors: string[] = [];

	if (password.length < 8) {
		errors.push("at least 8 characters");
	}
	if (!/[A-Z]/.test(password)) {
		errors.push("one uppercase letter");
	}
	if (!/[a-z]/.test(password)) {
		errors.push("one lowercase letter");
	}
	if (!/[0-9]/.test(password)) {
		errors.push("one number");
	}
	if (!/[!@#$%^&*(),.?":{}|<>]/.test(password)) {
		errors.push("one special character");
	}

	return errors;
};

const validateEmail = (email: string) => {
	const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
	return emailRegex.test(email);
};

export function Setup() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { setAuth } = useAuthStore();
	const [active, setActive] = useState(0);

	// Check setup status - redirect if already complete
	const { data: setupStatus, isLoading: isStatusLoading } = useQuery({
		queryKey: ["setup-status"],
		queryFn: setupApi.checkStatus,
		retry: 1,
	});

	useEffect(() => {
		// Redirect away if setup is already complete
		if (!isStatusLoading && setupStatus && !setupStatus.setupRequired) {
			navigate("/", { replace: true });
		}
	}, [setupStatus, isStatusLoading, navigate]);

	// Step 1: Create admin user
	const [username, setUsername] = useState("");
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");
	const [confirmPassword, setConfirmPassword] = useState("");

	// Step 2: Configure settings (optional)
	const [skipSettings, setSkipSettings] = useState(false);
	const [scannerExpanded, setScannerExpanded] = useState(true);
	const [appExpanded, setAppExpanded] = useState(false);
	const [taskExpanded, setTaskExpanded] = useState(false);

	// Scanner settings
	const [maxConcurrentScans, setMaxConcurrentScans] = useState(2);
	const [scanTimeoutMinutes, setScanTimeoutMinutes] = useState(120);
	const [retryFailedFiles, setRetryFailedFiles] = useState(false);
	const [autoAnalyzeConcurrency, setAutoAnalyzeConcurrency] = useState(4);

	// Application settings
	const [appName, setAppName] = useState("Codex");

	// Task worker settings
	const [pollIntervalSeconds, setPollIntervalSeconds] = useState(5);
	const [cleanupIntervalSeconds, setCleanupIntervalSeconds] = useState(30);

	// Initialize setup mutation
	const initializeMutation = useMutation<
		any,
		ApiError,
		InitializeSetupRequest
	>({
		mutationFn: setupApi.initialize,
		onSuccess: (data) => {
			// Set auth from the returned token
			setAuth(data.user, data.accessToken);
			// Move to next step
			setActive(1);
		},
	});

	// Configure settings mutation
	const configureSettingsMutation = useMutation<
		any,
		ApiError,
		ConfigureSettingsRequest
	>({
		mutationFn: setupApi.configureSettings,
		onSuccess: () => {
			// Immediately update the query cache to mark setup as complete
			// This prevents SetupRedirect from redirecting back to /setup
			queryClient.setQueryData<SetupStatusResponse>(["setup-status"], {
				setupRequired: false,
				hasUsers: true,
			});
			// Also invalidate to trigger a refetch in the background
			queryClient.invalidateQueries({ queryKey: ["setup-status"] });
			// Setup complete, redirect to home
			navigate("/", { replace: true });
		},
	});

	const handleCreateAdmin = (e: React.FormEvent) => {
		e.preventDefault();

		if (password !== confirmPassword) {
			return;
		}

		initializeMutation.mutate({ username, email, password });
	};

	const handleConfigureSettings = (e: React.FormEvent) => {
		e.preventDefault();

		if (skipSettings) {
			configureSettingsMutation.mutate({
				settings: {},
				skipConfiguration: true,
			});
		} else {
			configureSettingsMutation.mutate({
				settings: {
					"scanner.max_concurrent_scans": maxConcurrentScans.toString(),
					"scanner.scan_timeout_minutes": scanTimeoutMinutes.toString(),
					"scanner.retry_failed_files": retryFailedFiles.toString(),
					"scanner.auto_analyze_concurrency":
						autoAnalyzeConcurrency.toString(),
					"application.name": appName,
					"task.poll_interval_seconds": pollIntervalSeconds.toString(),
					"task.cleanup_interval_seconds": cleanupIntervalSeconds.toString(),
				},
				skipConfiguration: false,
			});
		}
	};

	const passwordsMatch = password === confirmPassword;
	const passwordErrors = validatePassword(password);
	const isPasswordValid = passwordErrors.length === 0;
	const isEmailValid = email.trim() !== "" && validateEmail(email);
	const canSubmitAdmin =
		username.trim() !== "" &&
		isEmailValid &&
		isPasswordValid &&
		passwordsMatch;

	// Don't render if setup is already complete or still loading status
	if (isStatusLoading || (setupStatus && !setupStatus.setupRequired)) {
		return null;
	}

	return (
		<Container size={700} my={40}>
			<Title ta="center" mb="xl">
				Welcome to Codex
			</Title>
			<Text c="dimmed" size="sm" ta="center" mb="xl">
				Let's set up your Codex instance
			</Text>

			<Paper withBorder shadow="md" p={30} radius="md">
				<Stepper active={active} onStepClick={setActive} allowNextStepsSelect={false}>
					<Stepper.Step
						label="Admin Account"
						description="Create your first admin user"
					>
						<form onSubmit={handleCreateAdmin}>
							<Stack>
								<TextInput
									label="Username"
									placeholder="admin"
									required
									value={username}
									onChange={(e) => setUsername(e.currentTarget.value)}
									disabled={initializeMutation.isPending}
								/>

								<TextInput
									label="Email"
									placeholder="admin@example.com"
									type="email"
									required
									value={email}
									onChange={(e) => setEmail(e.currentTarget.value)}
									disabled={initializeMutation.isPending}
									error={
										email && !isEmailValid ? "Invalid email address" : undefined
									}
								/>

								<PasswordInput
									label="Password"
									placeholder="Your password"
									required
									value={password}
									onChange={(e) => setPassword(e.currentTarget.value)}
									disabled={initializeMutation.isPending}
									description="Must contain: uppercase, lowercase, number, special character"
									error={
										password && !isPasswordValid
											? `Missing: ${passwordErrors.join(", ")}`
											: undefined
									}
								/>

								<PasswordInput
									label="Confirm Password"
									placeholder="Confirm your password"
									required
									value={confirmPassword}
									onChange={(e) => setConfirmPassword(e.currentTarget.value)}
									disabled={initializeMutation.isPending}
									error={
										confirmPassword && !passwordsMatch
											? "Passwords do not match"
											: undefined
									}
								/>

								{initializeMutation.isError && (
									<Alert icon={<IconAlertCircle size={16} />} color="red">
										{initializeMutation.error?.error ||
											"Failed to create admin user"}
									</Alert>
								)}

								<Button
									type="submit"
									fullWidth
									loading={initializeMutation.isPending}
									disabled={!canSubmitAdmin}
								>
									Create Admin User
								</Button>
							</Stack>
						</form>
					</Stepper.Step>

					<Stepper.Step
						label="Configure Settings"
						description="Optional: Customize your instance"
					>
						<form onSubmit={handleConfigureSettings}>
							<Stack>
								<Switch
									label="Skip configuration and use defaults"
									description="You can change these settings later from the admin panel"
									checked={skipSettings}
									onChange={(e) => setSkipSettings(e.currentTarget.checked)}
								/>

								{!skipSettings && (
									<Stack>
										{/* Scanner Settings */}
										<Paper withBorder p="md">
											<Group justify="space-between" mb="xs">
												<div>
													<Text fw={500}>Scanner Settings</Text>
													<Text size="xs" c="dimmed">
														Configure library scanning behavior
													</Text>
												</div>
												<Button
													variant="subtle"
													size="xs"
													onClick={() => setScannerExpanded(!scannerExpanded)}
												>
													{scannerExpanded ? "Collapse" : "Expand"}
												</Button>
											</Group>

											<Collapse in={scannerExpanded}>
												<Stack gap="sm" mt="sm">
													<NumberInput
														label="Max Concurrent Scans"
														description="Number of libraries that can be scanned simultaneously"
														value={maxConcurrentScans}
														onChange={(val) =>
															setMaxConcurrentScans(Number(val) || 2)
														}
														min={1}
														max={10}
													/>

													<NumberInput
														label="Scan Timeout (minutes)"
														description="Maximum time for a single library scan"
														value={scanTimeoutMinutes}
														onChange={(val) =>
															setScanTimeoutMinutes(Number(val) || 120)
														}
														min={10}
														max={1440}
													/>

													<Switch
														label="Retry Failed Files"
														description="Automatically retry files that failed to scan"
														checked={retryFailedFiles}
														onChange={(e) =>
															setRetryFailedFiles(e.currentTarget.checked)
														}
													/>

													<NumberInput
														label="Auto-Analyze Concurrency"
														description="Number of concurrent threads for file analysis"
														value={autoAnalyzeConcurrency}
														onChange={(val) =>
															setAutoAnalyzeConcurrency(Number(val) || 4)
														}
														min={1}
														max={16}
													/>
												</Stack>
											</Collapse>
										</Paper>

										{/* Application Settings */}
										<Paper withBorder p="md">
											<Group justify="space-between" mb="xs">
												<div>
													<Text fw={500}>Application Settings</Text>
													<Text size="xs" c="dimmed">
														General application configuration
													</Text>
												</div>
												<Button
													variant="subtle"
													size="xs"
													onClick={() => setAppExpanded(!appExpanded)}
												>
													{appExpanded ? "Collapse" : "Expand"}
												</Button>
											</Group>

											<Collapse in={appExpanded}>
												<Stack gap="sm" mt="sm">
													<TextInput
														label="Application Name"
														description="Display name for branding and UI"
														value={appName}
														onChange={(e) => setAppName(e.currentTarget.value)}
													/>
												</Stack>
											</Collapse>
										</Paper>

										{/* Task Worker Settings */}
										<Paper withBorder p="md">
											<Group justify="space-between" mb="xs">
												<div>
													<Text fw={500}>Task Worker Settings</Text>
													<Text size="xs" c="dimmed">
														Background task processing configuration
													</Text>
												</div>
												<Button
													variant="subtle"
													size="xs"
													onClick={() => setTaskExpanded(!taskExpanded)}
												>
													{taskExpanded ? "Collapse" : "Expand"}
												</Button>
											</Group>

											<Collapse in={taskExpanded}>
												<Stack gap="sm" mt="sm">
													<NumberInput
														label="Poll Interval (seconds)"
														description="How often to check for new tasks"
														value={pollIntervalSeconds}
														onChange={(val) =>
															setPollIntervalSeconds(Number(val) || 5)
														}
														min={1}
														max={60}
													/>

													<NumberInput
														label="Cleanup Interval (seconds)"
														description="How often to clean up completed tasks"
														value={cleanupIntervalSeconds}
														onChange={(val) =>
															setCleanupIntervalSeconds(Number(val) || 30)
														}
														min={10}
														max={300}
													/>
												</Stack>
											</Collapse>
										</Paper>
									</Stack>
								)}

								{configureSettingsMutation.isError && (
									<Alert icon={<IconAlertCircle size={16} />} color="red">
										{configureSettingsMutation.error?.error ||
											"Failed to configure settings"}
									</Alert>
								)}

								<Button
									type="submit"
									fullWidth
									loading={configureSettingsMutation.isPending}
								>
									{skipSettings ? "Skip and Finish" : "Save Settings and Finish"}
								</Button>
							</Stack>
						</form>
					</Stepper.Step>

					<Stepper.Completed>
						<Stack align="center">
							<IconCheck size={64} color="green" />
							<Text size="lg" fw={500}>
								Setup Complete!
							</Text>
							<Text c="dimmed">Redirecting to your library...</Text>
						</Stack>
					</Stepper.Completed>
				</Stepper>
			</Paper>
		</Container>
	);
}
