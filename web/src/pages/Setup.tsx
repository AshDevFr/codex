import {
  Alert,
  Button,
  Container,
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

  // Application settings
  const [appName, setAppName] = useState("Codex");

  // Authentication settings
  const [registrationEnabled, setRegistrationEnabled] = useState(false);

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

    const settings: Record<string, string> = {
      "application.name": appName,
      "auth.registration_enabled": registrationEnabled.toString(),
    };

    configureSettingsMutation.mutate({
      settings,
      skipConfiguration: false,
    });
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
                <Text size="sm" c="dimmed">
                  You can change these settings later from the admin panel.
                </Text>

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
                  Finish Setup
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
