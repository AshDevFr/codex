import { AppShell, Group, Text, Burger, TextInput, ActionIcon } from '@mantine/core';
import { IconSearch, IconMoon, IconSun } from '@tabler/icons-react';
import { useMantineColorScheme } from '@mantine/core';

interface HeaderProps {
  opened: boolean;
  toggle: () => void;
}

export function Header({ opened, toggle }: HeaderProps) {
  const { colorScheme, toggleColorScheme } = useMantineColorScheme();

  return (
    <AppShell.Header>
      <Group h="100%" px="md" justify="space-between">
        <Group>
          <Burger opened={opened} onClick={toggle} hiddenFrom="sm" size="sm" />
          <Text size="xl" fw={700}>
            Codex
          </Text>
        </Group>

        <Group>
          <TextInput
            placeholder="Search..."
            leftSection={<IconSearch size={16} />}
            visibleFrom="sm"
            w={300}
          />

          <ActionIcon
            variant="subtle"
            onClick={toggleColorScheme}
            title="Toggle color scheme"
          >
            {colorScheme === 'dark' ? <IconSun size={18} /> : <IconMoon size={18} />}
          </ActionIcon>
        </Group>
      </Group>
    </AppShell.Header>
  );
}
