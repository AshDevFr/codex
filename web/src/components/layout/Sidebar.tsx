import { AppShell, Stack, NavLink } from '@mantine/core';
import {
  IconHome,
  IconBooks,
  IconBookmark,
  IconSettings,
  IconUsers,
  IconLogout,
} from '@tabler/icons-react';
import { useAuthStore } from '@/store/authStore';

interface SidebarProps {
  currentPath?: string;
}

export function Sidebar({ currentPath = '/' }: SidebarProps) {
  const { user, clearAuth } = useAuthStore();
  const isAdmin = user?.role === 'ADMIN';

  const handleLogout = () => {
    clearAuth();
    window.location.href = '/login';
  };

  return (
    <AppShell.Navbar p="md">
      <AppShell.Section grow>
        <Stack gap="xs">
          <NavLink
            href="/"
            label="Home"
            leftSection={<IconHome size={20} />}
            active={currentPath === '/'}
          />
          <NavLink
            href="/libraries"
            label="Libraries"
            leftSection={<IconBooks size={20} />}
            active={currentPath === '/libraries'}
          />
          <NavLink
            href="/series"
            label="Series"
            leftSection={<IconBookmark size={20} />}
            active={currentPath === '/series'}
          />

          {isAdmin && (
            <>
              <NavLink
                href="/users"
                label="Users"
                leftSection={<IconUsers size={20} />}
                active={currentPath === '/users'}
              />
            </>
          )}

          <NavLink
            href="/settings"
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
