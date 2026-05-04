import {
  Anchor,
  Breadcrumbs,
  Button,
  Center,
  Container,
  Group,
  Loader,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconArrowLeft, IconPlus } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import type { LibraryJob } from "@/api/libraryJobs";
import { JobList } from "@/components/library-jobs/LibraryJobsList";
import { JobEditor } from "@/components/library-jobs/MetadataRefreshJobEditor";
import { useDynamicDocumentTitle } from "@/hooks/useDocumentTitle";
import {
  useDeleteLibraryJob,
  useLibraryJobsList,
} from "@/hooks/useLibraryJobs";

export function LibraryJobsPage() {
  const { libraryId } = useParams<{ libraryId: string }>();
  const navigate = useNavigate();
  const [editorOpened, editor] = useDisclosure(false);
  const [editingJob, setEditingJob] = useState<LibraryJob | null>(null);

  const library = useQuery({
    queryKey: ["library", libraryId],
    queryFn: () => librariesApi.getById(libraryId as string),
    enabled: Boolean(libraryId),
  });

  const jobsQuery = useLibraryJobsList(libraryId ?? "");
  const deleteMutation = useDeleteLibraryJob(libraryId ?? "");

  useDynamicDocumentTitle(
    library.data ? `Scheduled Jobs · ${library.data.name}` : "Scheduled Jobs",
  );

  if (!libraryId) return null;

  if (library.isLoading) {
    return (
      <Container size="xl" py="xl">
        <Center>
          <Loader />
        </Center>
      </Container>
    );
  }

  const handleAdd = () => {
    setEditingJob(null);
    editor.open();
  };

  const handleEdit = (job: LibraryJob) => {
    setEditingJob(job);
    editor.open();
  };

  const handleDelete = (job: LibraryJob) => {
    if (!window.confirm(`Delete job "${job.name}"? This cannot be undone.`)) {
      return;
    }
    deleteMutation.mutate(job.id);
  };

  return (
    <Container size="lg" py="xl">
      <Stack gap="lg">
        <Breadcrumbs>
          <Anchor onClick={() => navigate("/libraries")} component="button">
            Libraries
          </Anchor>
          {library.data && (
            <Anchor
              onClick={() => navigate(`/libraries/${library.data.id}`)}
              component="button"
            >
              {library.data.name}
            </Anchor>
          )}
          <Text>Scheduled Jobs</Text>
        </Breadcrumbs>

        <Group justify="space-between" align="flex-end">
          <Stack gap={4}>
            <Title order={2}>Scheduled Jobs</Title>
            <Text c="dimmed" size="sm">
              Each job runs on its own cron, against one provider, and writes
              the field groups you select. Multiple jobs per library are
              supported.
            </Text>
          </Stack>
          <Group>
            <Button
              variant="default"
              leftSection={<IconArrowLeft size={16} />}
              onClick={() =>
                navigate(
                  library.data ? `/libraries/${library.data.id}` : "/libraries",
                )
              }
            >
              Back to library
            </Button>
            <Button leftSection={<IconPlus size={16} />} onClick={handleAdd}>
              Add job
            </Button>
          </Group>
        </Group>

        <JobList
          libraryId={libraryId}
          jobs={jobsQuery.data ?? []}
          isLoading={jobsQuery.isLoading}
          onEdit={handleEdit}
          onDelete={handleDelete}
        />
      </Stack>

      <JobEditor
        libraryId={libraryId}
        opened={editorOpened}
        onClose={editor.close}
        job={editingJob}
      />
    </Container>
  );
}
