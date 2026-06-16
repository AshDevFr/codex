import {
  Box,
  Button,
  Card,
  Center,
  Container,
  Group,
  Image,
  SimpleGrid,
  Skeleton,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { IconList, IconPlus } from "@tabler/icons-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import type { ReadList } from "@/api/readlists";
import { ReadListFormModal } from "@/components/readlists/ReadListFormModal";
import { usePermissions } from "@/hooks/usePermissions";
import { useReadLists } from "@/hooks/useReadLists";
import { PERMISSIONS } from "@/types/permissions";

const NO_COVER =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='300'%3E%3Crect fill='%23ddd' width='200' height='300'/%3E%3Ctext fill='%23999' font-family='sans-serif' font-size='14' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E";

function ReadListCard({ readList }: { readList: ReadList }) {
  return (
    <Card
      component={Link}
      to={`/readlists/${readList.id}`}
      shadow="sm"
      padding={0}
      radius="md"
      withBorder
      data-pressable="true"
      style={{ height: "100%", display: "flex", flexDirection: "column" }}
    >
      <Box style={{ aspectRatio: "150/212.125", overflow: "hidden" }}>
        <Image
          src={`/api/v1/readlists/${readList.id}/thumbnail`}
          alt={readList.name}
          fit="cover"
          h="100%"
          fallbackSrc={NO_COVER}
        />
      </Box>
      <Stack gap={2} p="sm">
        <Text fw={600} size="sm" lineClamp={1}>
          {readList.name}
        </Text>
        <Text size="xs" c="dimmed">
          {readList.bookCount} books
        </Text>
      </Stack>
    </Card>
  );
}

export function ReadLists() {
  const { data: readLists, isLoading } = useReadLists();
  const { hasPermission } = usePermissions();
  const canWrite = hasPermission(PERMISSIONS.READLISTS_WRITE);
  const [createOpen, setCreateOpen] = useState(false);

  return (
    <Container size="xl" py="md">
      <Group justify="space-between" align="center" mb="lg">
        <Group gap="xs">
          <IconList size={28} />
          <Title order={2}>Read Lists</Title>
        </Group>
        {canWrite && (
          <Button
            leftSection={<IconPlus size={16} />}
            onClick={() => setCreateOpen(true)}
          >
            New Read List
          </Button>
        )}
      </Group>

      {isLoading ? (
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {Array.from({ length: 6 }).map((_, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: static skeletons
            <Skeleton key={i} height={300} radius="md" />
          ))}
        </SimpleGrid>
      ) : !readLists || readLists.length === 0 ? (
        <Center mih={240}>
          <Stack align="center" gap="xs">
            <IconList size={48} opacity={0.4} />
            <Text c="dimmed">No read lists yet.</Text>
            {canWrite && (
              <Text c="dimmed" size="sm">
                Create one, then add books to it from a book page.
              </Text>
            )}
          </Stack>
        </Center>
      ) : (
        <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 6 }} spacing="md">
          {readLists.map((readList) => (
            <ReadListCard key={readList.id} readList={readList} />
          ))}
        </SimpleGrid>
      )}

      <ReadListFormModal
        opened={createOpen}
        onClose={() => setCreateOpen(false)}
      />
    </Container>
  );
}
