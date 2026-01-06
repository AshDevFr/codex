import { describe, it, expect, beforeEach, vi } from 'vitest';
import { screen } from '@testing-library/react';
import { renderWithProviders, userEvent } from '@/test/utils';
import { AppLayout } from './AppLayout';
import { useAuthStore } from '@/store/authStore';
import type { User } from '@/types/api';

describe('Sidebar Component (via AppLayout)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();

    // Mock window.location
    delete (window as any).location;
    window.location = { href: '' } as any;
  });

  it('should render navigation links', () => {
    const mockUser: User = {
      id: '1',
      username: 'testuser',
      email: 'test@example.com',
      role: 'USER',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    };

    useAuthStore.setState({ user: mockUser, token: 'token', isAuthenticated: true });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>
    );

    expect(screen.getByText('Home')).toBeInTheDocument();
    expect(screen.getByText('Libraries')).toBeInTheDocument();
    expect(screen.getByText('Series')).toBeInTheDocument();
    expect(screen.getByText('Settings')).toBeInTheDocument();
    expect(screen.getByText('Logout')).toBeInTheDocument();
  });

  it('should show Users link for admin users', () => {
    const mockAdmin: User = {
      id: '1',
      username: 'admin',
      email: 'admin@example.com',
      role: 'ADMIN',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    };

    useAuthStore.setState({ user: mockAdmin, token: 'token', isAuthenticated: true });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>
    );

    expect(screen.getByText('Users')).toBeInTheDocument();
  });

  it('should not show Users link for regular users', () => {
    const mockUser: User = {
      id: '1',
      username: 'testuser',
      email: 'test@example.com',
      role: 'USER',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    };

    useAuthStore.setState({ user: mockUser, token: 'token', isAuthenticated: true });

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>
    );

    expect(screen.queryByText('Users')).not.toBeInTheDocument();
  });

  it('should handle logout', async () => {
    const user = userEvent.setup();
    const mockUser: User = {
      id: '1',
      username: 'testuser',
      email: 'test@example.com',
      role: 'USER',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    };

    useAuthStore.setState({ user: mockUser, token: 'token', isAuthenticated: true });
    localStorage.setItem('jwt_token', 'token');

    renderWithProviders(
      <AppLayout>
        <div>Content</div>
      </AppLayout>
    );

    const logoutButton = screen.getByText('Logout');
    await user.click(logoutButton);

    // Should clear auth and redirect
    expect(localStorage.getItem('jwt_token')).toBeNull();
    expect(window.location.href).toBe('/login');
  });
});
