import {
	Alert,
	Button,
	Collapse,
	Container,
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
import { CronInput } from "@/components/forms/CronInput";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useAuthStore } from "@/store/authStore";
import type {
	ApiError,
	ConfigureSettingsRequest,
	InitializeSetupRequest,
	SetupStatusResponse,
} from "@/types";

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

	useDocumentTitle("Setup");
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
	const [showAdvanced, setShowAdvanced] = useState(false);

	// Scanner settings
	const [scanTimeoutMinutes, setScanTimeoutMinutes] = useState(120);
	const [retryFailedFiles, setRetryFailedFiles] = useState(false);
	const [scannerBatchSize, setScannerBatchSize] = useState(100);
	const [scannerParallelHashing, setScannerParallelHashing] = useState(8);
	const [scannerParallelSeries, setScannerParallelSeries] = useState(4);

	// Application settings
	const [appName, setAppName] = useState("Codex");

	// Authentication settings
	const [registrationEnabled, setRegistrationEnabled] = useState(false);

	// Task worker settings
	const [pollIntervalSeconds, setPollIntervalSeconds] = useState(5);
	const [cleanupIntervalSeconds, setCleanupIntervalSeconds] = useState(30);
	const [prioritizeScansOverAnalysis, setPrioritizeScansOverAnalysis] =
		useState(true);

	// Thumbnail settings
	const [thumbnailMaxDimension, setThumbnailMaxDimension] = useState(400);
	const [thumbnailJpegQuality, setThumbnailJpegQuality] = useState(85);

	// Deduplication settings
	const [deduplicationEnabled, setDeduplicationEnabled] = useState(true);
	const [deduplicationCronSchedule, setDeduplicationCronSchedule] =
		useState("");

	// Purge settings
	const [purgeEmptySeries, setPurgeEmptySeries] = useState(true);

	// Initialize setup mutation
	const initializeMutation = useMutation<any, ApiError, InitializeSetupRequest>(
		{
			mutationFn: setupApi.initialize,
			onSuccess: (data) => {
				// Set auth from the returned token
				setAuth(data.user, data.accessToken);
				// Move to next step
				setActive(1);
			},
		},
	);

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
				registrationEnabled: false,
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
			const settings: Record<string, string> = {
				"scanner.scan_timeout_minutes": scanTimeoutMinutes.toString(),
				"scanner.retry_failed_files": retryFailedFiles.toString(),
				"scanner.batch_size": scannerBatchSize.toString(),
				"scanner.parallel_hashing": scannerParallelHashing.toString(),
				"scanner.parallel_series": scannerParallelSeries.toString(),
				"application.name": appName,
				"auth.registration_enabled": registrationEnabled.toString(),
				"task.poll_interval_seconds": pollIntervalSeconds.toString(),
				"task.cleanup_interval_seconds": cleanupIntervalSeconds.toString(),
				"task.prioritize_scans_over_analysis":
					prioritizeScansOverAnalysis.toString(),
				"thumbnail.max_dimension": thumbnailMaxDimension.toString(),
				"thumbnail.jpeg_quality": thumbnailJpegQuality.toString(),
				"deduplication.enabled": deduplicationEnabled.toString(),
				"purge.purge_empty_series": purgeEmptySeries.toString(),
			};

			// Only include cron schedule if deduplication is enabled
			if (deduplicationEnabled && deduplicationCronSchedule.trim()) {
				settings["deduplication.cron_schedule"] =
					deduplicationCronSchedule.trim();
			}

			configureSettingsMutation.mutate({
				settings,
				skipConfiguration: false,
			});
		}
	};

	const passwordsMatch = password === confirmPassword;
	const passwordErrors = validatePassword(password);
	const isPasswordValid = passwordErrors.length === 0;
	const isEmailValid = email.trim() !== "" && validateEmail(email);
	const canSubmitAdmin =
		username.trim() !== "" && isEmailValid && isPasswordValid && passwordsMatch;

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
				<Stepper
					active={active}
					onStepClick={setActive}
					allowNextStepsSelect={false}
				>
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
										{/* Essential Settings - Always Visible */}
										<Paper withBorder p="md">
											<Text fw={500} mb="md">
												Basic Settings
											</Text>
											<Stack gap="sm">
												<TextInput
													label="Application Name"
													description="Display name for branding and UI"
													value={appName}
													onChange={(e) => setAppName(e.currentTarget.value)}
												/>

												<Switch
													label="Enable User Registration"
													description="Allow new users to register accounts (disabled by default for security)"
													checked={registrationEnabled}
													onChange={(e) =>
														setRegistrationEnabled(e.currentTarget.checked)
													}
												/>
											</Stack>
										</Paper>

										{/* Advanced Settings Toggle */}
										<Button
											variant="subtle"
											onClick={() => setShowAdvanced(!showAdvanced)}
											fullWidth
										>
											{showAdvanced
												? "Hide Advanced Settings"
												: "Show Advanced Settings"}
										</Button>

										{/* Advanced Settings - Hidden by Default */}
										<Collapse in={showAdvanced}>
											<Stack gap="md">
												{/* Scanner Settings */}
												<Paper withBorder p="md">
													<Text fw={500} mb="xs">
														Scanner Settings
													</Text>
													<Text size="xs" c="dimmed" mb="md">
														Performance tuning for library scanning
													</Text>
													<Stack gap="sm">
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
															label="Batch Size"
															description="Files per batch during scanning (higher = faster but more memory)"
															value={scannerBatchSize}
															onChange={(val) =>
																setScannerBatchSize(Number(val) || 100)
															}
															min={10}
															max={500}
														/>

														<NumberInput
															label="Parallel Hashing"
															description="Concurrent file hashing (higher = faster on SSD, lower for HDD/network)"
															value={scannerParallelHashing}
															onChange={(val) =>
																setScannerParallelHashing(Number(val) || 8)
															}
															min={1}
															max={32}
														/>

														<NumberInput
															label="Parallel Series"
															description="Concurrent series processing (higher = faster for many small series)"
															value={scannerParallelSeries}
															onChange={(val) =>
																setScannerParallelSeries(Number(val) || 4)
															}
															min={1}
															max={16}
														/>
													</Stack>
												</Paper>

												{/* Task Worker Settings */}
												<Paper withBorder p="md">
													<Text fw={500} mb="xs">
														Task Worker Settings
													</Text>
													<Text size="xs" c="dimmed" mb="md">
														Background task processing configuration
													</Text>
													<Stack gap="sm">
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

														<Switch
															label="Prioritize Scans Over Analysis"
															description="When enabled, scan tasks will be processed before analysis tasks in the queue"
															checked={prioritizeScansOverAnalysis}
															onChange={(e) =>
																setPrioritizeScansOverAnalysis(
																	e.currentTarget.checked,
																)
															}
														/>
													</Stack>
												</Paper>

												{/* Thumbnail Settings */}
												<Paper withBorder p="md">
													<Text fw={500} mb="xs">
														Thumbnail Settings
													</Text>
													<Text size="xs" c="dimmed" mb="md">
														Configure thumbnail generation
													</Text>
													<Stack gap="sm">
														<NumberInput
															label="Max Dimension (pixels)"
															description="Maximum width or height for generated thumbnails"
															value={thumbnailMaxDimension}
															onChange={(val) =>
																setThumbnailMaxDimension(Number(val) || 400)
															}
															min={100}
															max={2000}
														/>

														<NumberInput
															label="JPEG Quality"
															description="Quality for thumbnail images (higher = better quality but larger files)"
															value={thumbnailJpegQuality}
															onChange={(val) =>
																setThumbnailJpegQuality(Number(val) || 85)
															}
															min={50}
															max={100}
														/>
													</Stack>
												</Paper>

												{/* Deduplication Settings */}
												<Paper withBorder p="md">
													<Text fw={500} mb="xs">
														Deduplication Settings
													</Text>
													<Text size="xs" c="dimmed" mb="md">
														Automatic duplicate detection
													</Text>
													<Stack gap="sm">
														<Switch
															label="Enable Deduplication"
															description="Enable automatic duplicate detection scanning"
															checked={deduplicationEnabled}
															onChange={(e) =>
																setDeduplicationEnabled(e.currentTarget.checked)
															}
														/>

														{deduplicationEnabled && (
															<CronInput
																label="Cron Schedule"
																description="Cron expression for automatic duplicate detection (e.g., '0 2 * * *' for daily at 2am). Leave empty to disable automatic scanning."
																placeholder="0 2 * * *"
																value={deduplicationCronSchedule}
																onChange={setDeduplicationCronSchedule}
															/>
														)}
													</Stack>
												</Paper>

												{/* Purge Settings */}
												<Paper withBorder p="md">
													<Text fw={500} mb="xs">
														Purge Settings
													</Text>
													<Text size="xs" c="dimmed" mb="md">
														Cleanup behavior for deleted items
													</Text>
													<Stack gap="sm">
														<Switch
															label="Purge Empty Series"
															description="When purging deleted books, also delete series that have no remaining books"
															checked={purgeEmptySeries}
															onChange={(e) =>
																setPurgeEmptySeries(e.currentTarget.checked)
															}
														/>
													</Stack>
												</Paper>
											</Stack>
										</Collapse>
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
									{skipSettings
										? "Skip and Finish"
										: "Save Settings and Finish"}
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
