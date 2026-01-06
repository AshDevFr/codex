import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  Container,
  Paper,
  TextInput,
  PasswordInput,
  Button,
  Title,
  Text,
  Stack,
  Alert,
  Anchor,
} from '@mantine/core';
import { IconAlertCircle } from '@tabler/icons-react';
import { useMutation } from '@tanstack/react-query';
import { authApi } from '@/api/auth';
import { useAuthStore } from '@/store/authStore';
import type { ApiError, LoginRequest, LoginResponse } from '@/types/api';

export function Login() {
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const { setAuth } = useAuthStore();

  const loginMutation = useMutation<LoginResponse, ApiError, LoginRequest>({
    mutationFn: authApi.login,
    onSuccess: (data) => {
      setAuth(data.user, data.accessToken);
      navigate('/');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    loginMutation.mutate({ username, password });
  };

  return (
    <Container size={420} my={100}>
      <Title ta="center" mb="xl">
        Welcome to Codex
      </Title>

      <Paper withBorder shadow="md" p={30} radius="md">
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
                {loginMutation.error?.error || 'Login failed'}
              </Alert>
            )}

            <Button type="submit" fullWidth loading={loginMutation.isPending}>
              Sign in
            </Button>
          </Stack>
        </form>

        <Text c="dimmed" size="sm" ta="center" mt="md">
          Don't have an account?{' '}
          <Anchor component={Link} to="/register" size="sm">
            Create one
          </Anchor>
        </Text>
      </Paper>
    </Container>
  );
}
