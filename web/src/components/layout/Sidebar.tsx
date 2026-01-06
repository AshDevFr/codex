import { AppShell, Stack, NavLink } from '@mantine/core';
import {
  IconHome,
  IconBooks,
  IconSettings,
  IconUsers,
  IconLogout,
} from '@tabler/icons-react';
import { useNavigate } from 'react-router-dom';
import { useAuthStore } from '@/store/authStore';

interface SidebarProps {
  currentPath?: string;
}

export function Sidebar({ currentPath = '/' }: SidebarProps) {
  const navigate = useNavigate();
  const { user, clearAuth } = useAuthStore();
  const isAdmin = user?.role === 'ADMIN';

  const handleLogout = () => {
    clearAuth();
    navigate('/login');
  };

  return (
    <AppShell.Navbar p="md">
      <AppShell.Section grow>
        <Stack gap="xs">
          <NavLink
            onClick={() => navigate('/')}
            label="Home"
            leftSection={<IconHome size={20} />}
            active={currentPath === '/'}
          />
          <NavLink
            onClick={() => navigate('/libraries')}
            label="Libraries"
            leftSection={<IconBooks size={20} />}
            active={currentPath === '/libraries'}
          />

          {isAdmin && (
            <>
              <NavLink
                onClick={() => navigate('/users')}
                label="Users"
                leftSection={<IconUsers size={20} />}
                active={currentPath === '/users'}
              />
            </>
          )}

          <NavLink
            onClick={() => navigate('/settings')}
            label="Settings"
            leftSection={<IconSettings size={20} />}
            active={currentPath === '/settings'}
          />
        </Stack>
      </AppShell.Section>

      <AppShell.Section>
        <NavLink
          label="Logout"
          leftSection={<IconLogout size={20} />}
          onClick={handleLogout}
          color="red"
        />
      </AppShell.Section>
    </AppShell.Navbar>
  );
}
