import {
  Alert,
  Anchor,
  Box,
  Button,
  Container,
  Divider,
  Paper,
  PasswordInput,
  Stack,
  Text,
  TextInput,
  Title,
} from "@mantine/core";
import { IconAlertCircle, IconLogin } from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { authApi } from "@/api/auth";
import { setupApi } from "@/api/setup";
import { useAppName } from "@/hooks/useAppName";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useAuthStore } from "@/store/authStore";
import type {
  ApiError,
  LoginRequest,
  LoginResponse,
  OidcLoginResponse,
} from "@/types";

export function Login() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const appName = useAppName();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const { setAuth } = useAuthStore();

  // OIDC error from callback redirect (e.g., user denied access at IdP)
  const oidcError =
    searchParams.get("error_description") || searchParams.get("error");

  useDocumentTitle("Login");

  const { data: setupStatus } = useQuery({
    queryKey: ["setup-status"],
    queryFn: setupApi.checkStatus,
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
  });

  const { data: oidcProviders } = useQuery({
    queryKey: ["oidc-providers"],
    queryFn: authApi.getOidcProviders,
    staleTime: 5 * 60 * 1000,
  });

  const loginMutation = useMutation<LoginResponse, ApiError, LoginRequest>({
    mutationFn: authApi.login,
    onSuccess: (data) => {
      setAuth(data.user, data.accessToken);
      navigate("/");
    },
  });

  const oidcLoginMutation = useMutation<OidcLoginResponse, ApiError, string>({
    mutationFn: authApi.initiateOidcLogin,
    onSuccess: (data) => {
      // Redirect to the identity provider
      window.location.href = data.redirectUrl;
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    loginMutation.mutate({ username, password });
  };

  const hasOidcProviders =
    oidcProviders?.enabled && oidcProviders.providers.length > 0;

  const errorMessage =
    oidcError ||
    (loginMutation.isError
      ? loginMutation.error?.error || "Login failed"
      : null) ||
    (oidcLoginMutation.isError
      ? oidcLoginMutation.error?.error || "Failed to start SSO login"
      : null);

  return (
    <Box bg="dark.7" mih="100vh">
      <Container size={420} py={100}>
        <Title ta="center" mb="xl">
          Welcome to {appName}
        </Title>

        <Paper shadow="md" p={30} radius="md" bg="dark.6">
          {hasOidcProviders && (
            <>
              <Stack>
                {oidcProviders.providers.map((provider) => (
                  <Button
                    key={provider.name}
                    fullWidth
                    variant="default"
                    leftSection={<IconLogin size={18} />}
                    loading={
                      oidcLoginMutation.isPending &&
                      oidcLoginMutation.variables === provider.name
                    }
                    disabled={oidcLoginMutation.isPending}
                    onClick={() => oidcLoginMutation.mutate(provider.name)}
                  >
                    Sign in with {provider.displayName}
                  </Button>
                ))}
              </Stack>

              <Divider
                label="Or continue with"
                labelPosition="center"
                my="lg"
              />
            </>
          )}

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

              {errorMessage && (
                <Alert icon={<IconAlertCircle size={16} />} color="red">
                  {errorMessage}
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
