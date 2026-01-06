import { Container, Title, SimpleGrid, Card, Text, Group, Badge, Button, Stack, Loader, Center } from '@mantine/core';
import { IconBooks, IconFolder, IconRefresh } from '@tabler/icons-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { librariesApi } from '@/api/libraries';
import { notifications } from '@mantine/notifications';
import type { Library } from '@/types/api';

export function Home() {
  const queryClient = useQueryClient();

  const { data: libraries, isLoading } = useQuery({
    queryKey: ['libraries'],
    queryFn: librariesApi.getAll,
  });

  const scanMutation = useMutation({
    mutationFn: (libraryId: string) => librariesApi.scan(libraryId),
    onSuccess: () => {
      notifications.show({
        title: 'Scan started',
        message: 'Library scan has been initiated',
        color: 'blue',
      });
      queryClient.invalidateQueries({ queryKey: ['libraries'] });
    },
    onError: (error: any) => {
      notifications.show({
        title: 'Scan failed',
        message: error.message || 'Failed to start scan',
        color: 'red',
      });
    },
  });

  if (isLoading) {
    return (
      <Center h="100vh">
        <Loader size="xl" />
      </Center>
    );
  }

  return (
    <Container size="xl" py="xl">
      <Stack gap="xl">
        <Group justify="space-between">
          <Title order={1}>Libraries</Title>
          <Button leftSection={<IconFolder size={18} />}>Add Library</Button>
        </Group>

        {libraries && libraries.length > 0 ? (
          <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="lg">
            {libraries.map((library: Library) => (
              <Card key={library.id} shadow="sm" padding="lg" radius="md" withBorder>
                <Stack gap="md">
                  <Group justify="space-between">
                    <Text fw={500} size="lg">
                      {library.name}
                    </Text>
                    <Badge color={library.scan_mode === 'AUTO' ? 'green' : 'gray'}>
                      {library.scan_mode}
                    </Badge>
                  </Group>

                  <Text size="sm" c="dimmed" lineClamp={2}>
                    {library.path}
                  </Text>

                  <Group gap="xs">
                    <Group gap={4}>
                      <IconBooks size={16} />
                      <Text size="sm">{library.book_count || 0} books</Text>
                    </Group>
                    <Text size="sm" c="dimmed">
                      {library.series_count || 0} series
                    </Text>
                  </Group>

                  {library.last_scan_at && (
                    <Text size="xs" c="dimmed">
                      Last scan: {new Date(library.last_scan_at).toLocaleString()}
                    </Text>
                  )}

                  <Button
                    leftSection={<IconRefresh size={16} />}
                    variant="light"
                    fullWidth
                    onClick={() => scanMutation.mutate(library.id)}
                    loading={scanMutation.isPending}
                  >
                    Scan Library
                  </Button>
                </Stack>
              </Card>
            ))}
          </SimpleGrid>
        ) : (
          <Card padding="xl" radius="md" withBorder>
            <Center>
              <Stack align="center" gap="md">
                <IconFolder size={48} stroke={1.5} />
                <Title order={3}>No libraries found</Title>
                <Text c="dimmed" ta="center">
                  Get started by adding your first library
                </Text>
                <Button leftSection={<IconFolder size={18} />}>Add Library</Button>
              </Stack>
            </Center>
          </Card>
        )}
      </Stack>
    </Container>
  );
}
