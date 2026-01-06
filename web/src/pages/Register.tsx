import { useState } from 'react';
import { Link } from 'react-router-dom';
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
import { IconAlertCircle, IconCircleCheck } from '@tabler/icons-react';
import { useMutation } from '@tanstack/react-query';
import { authApi } from '@/api/auth';
import { useAuthStore } from '@/store/authStore';
import type { ApiError, RegisterRequest } from '@/types/api';

export function Register() {
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [passwordError, setPasswordError] = useState('');
  const { setAuth } = useAuthStore();

  const registerMutation = useMutation<any, ApiError, RegisterRequest>({
    mutationFn: authApi.register,
    onSuccess: (data) => {
      // If access token is provided, user is logged in automatically
      if (data.accessToken) {
        setAuth(data.user, data.accessToken);
        window.location.href = '/';
      }
      // Otherwise, email confirmation is required (message will show in UI)
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    // Validate password match
    if (password !== confirmPassword) {
      setPasswordError('Passwords do not match');
      return;
    }

    // Validate password length
    if (password.length < 8) {
      setPasswordError('Password must be at least 8 characters');
      return;
    }

    setPasswordError('');
    registerMutation.mutate({ username, email, password });
  };

  return (
    <Container size={420} my={100}>
      <Title ta="center" mb="xl">
        Create Account
      </Title>

      <Paper withBorder shadow="md" p={30} radius="md">
        <form onSubmit={handleSubmit}>
          <Stack>
            <TextInput
              label="Username"
              placeholder="Choose a username"
              required
              value={username}
              onChange={(e) => setUsername(e.currentTarget.value)}
            />

            <TextInput
              label="Email"
              placeholder="your@email.com"
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.currentTarget.value)}
            />

            <PasswordInput
              label="Password"
              placeholder="At least 8 characters"
              required
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              error={passwordError}
            />

            <PasswordInput
              label="Confirm Password"
              placeholder="Repeat your password"
              required
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.currentTarget.value)}
              error={passwordError}
            />

            {registerMutation.isSuccess && registerMutation.data.message && (
              <Alert icon={<IconCircleCheck size={16} />} color="green">
                {registerMutation.data.message}
              </Alert>
            )}

            {registerMutation.isError && (
              <Alert icon={<IconAlertCircle size={16} />} color="red">
                {registerMutation.error?.error || 'Registration failed'}
              </Alert>
            )}

            <Button type="submit" fullWidth loading={registerMutation.isPending}>
              Create Account
            </Button>
          </Stack>
        </form>

        <Text c="dimmed" size="sm" ta="center" mt="md">
          Already have an account?{' '}
          <Anchor component={Link} to="/login" size="sm">
            Sign in
          </Anchor>
        </Text>
      </Paper>
    </Container>
  );
}
