import {
  Badge,
  Box,
  Button,
  Card,
  Code,
  Divider,
  Group,
  Loader,
  Modal,
  ScrollArea,
  Stack,
  Text,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { type PluginFailuresResponse, pluginsApi } from "@/api/plugins";

// Individual failure card component
function FailureCard({
  failure,
  showDetails = false,
}: {
  failure: PluginFailuresResponse["failures"][0];
  showDetails?: boolean;
}) {
  return (
    <Card withBorder p="xs" radius="sm">
      <Stack gap="xs">
        <Group justify="space-between" wrap="nowrap">
          <Group gap="xs" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
            {failure.errorCode && (
              <Badge size="xs" color="red" variant="light">
                {failure.errorCode}
              </Badge>
            )}
            {failure.method && (
              <Badge size="xs" color="blue" variant="outline">
                {failure.method}
              </Badge>
            )}
            <Text
              size="xs"
              lineClamp={showDetails ? undefined : 1}
              style={{ flex: 1 }}
            >
              {failure.errorMessage}
            </Text>
          </Group>
          <Text size="xs" c="dimmed" style={{ flexShrink: 0 }}>
            {new Date(failure.occurredAt).toLocaleString()}
          </Text>
        </Group>
        {showDetails && failure.requestSummary && (
          <Box>
            <Text size="xs" c="dimmed" fw={600}>
              Request Summary:
            </Text>
            <Code block style={{ fontSize: "11px" }}>
              {failure.requestSummary}
            </Code>
          </Box>
        )}
      </Stack>
    </Card>
  );
}

// Plugin failure history component
export function PluginFailureHistory({ pluginId }: { pluginId: string }) {
  const [showAllModal, setShowAllModal] = useState(false);
  const [page, setPage] = useState(1);
  const pageSize = 5; // Show 5 recent failures inline
  const modalPageSize = 20;

  // Query for inline display (first 5)
  const { data, isLoading, error } = useQuery<PluginFailuresResponse>({
    queryKey: ["plugin-failures", pluginId, "inline"],
    queryFn: () => pluginsApi.getFailures(pluginId, pageSize, 0),
  });

  // Query for modal display (paginated)
  const { data: modalData, isLoading: modalLoading } =
    useQuery<PluginFailuresResponse>({
      queryKey: ["plugin-failures", pluginId, "modal", page],
      queryFn: () =>
        pluginsApi.getFailures(
          pluginId,
          modalPageSize,
          (page - 1) * modalPageSize,
        ),
      enabled: showAllModal,
    });

  if (isLoading) {
    return (
      <Group justify="center" py="sm">
        <Loader size="sm" />
      </Group>
    );
  }

  if (error || !data) {
    return null;
  }

  if (data.failures.length === 0) {
    return null;
  }

  const totalPages = Math.ceil(
    (modalData?.total ?? data.total) / modalPageSize,
  );

  return (
    <>
      <Divider label="Failure History" labelPosition="left" />
      <Group gap="xl">
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
            Window Failures
          </Text>
          <Group gap="xs">
            <Text
              size="sm"
              fw={500}
              c={data.windowFailures >= data.threshold ? "red" : undefined}
            >
              {data.windowFailures} / {data.threshold}
            </Text>
            <Text size="xs" c="dimmed">
              (in {Math.round(data.windowSeconds / 60)} min)
            </Text>
          </Group>
        </div>
        <div>
          <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
            Total Recorded
          </Text>
          <Text size="sm">{data.total}</Text>
        </div>
        {data.total > pageSize && (
          <Button
            variant="light"
            size="xs"
            onClick={() => {
              setPage(1);
              setShowAllModal(true);
            }}
          >
            View All ({data.total})
          </Button>
        )}
      </Group>

      <Stack gap="xs">
        {data.failures.map((failure) => (
          <FailureCard key={failure.id} failure={failure} />
        ))}
      </Stack>

      {/* View All Failures Modal */}
      <Modal
        opened={showAllModal}
        onClose={() => setShowAllModal(false)}
        title="Failure History"
        size="lg"
      >
        <Stack gap="md">
          <Group gap="xl">
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Window Failures
              </Text>
              <Text size="sm" fw={500}>
                {data.windowFailures} / {data.threshold}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Window Duration
              </Text>
              <Text size="sm">
                {Math.round(data.windowSeconds / 60)} minutes
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                Total Failures
              </Text>
              <Text size="sm">{modalData?.total ?? data.total}</Text>
            </div>
          </Group>

          <Divider />

          {modalLoading ? (
            <Group justify="center" py="xl">
              <Loader />
            </Group>
          ) : (
            <ScrollArea.Autosize mah={400}>
              <Stack gap="xs">
                {modalData?.failures.map((failure) => (
                  <FailureCard key={failure.id} failure={failure} showDetails />
                ))}
              </Stack>
            </ScrollArea.Autosize>
          )}

          {totalPages > 1 && (
            <Group justify="center" mt="md">
              <Button
                variant="subtle"
                size="xs"
                disabled={page === 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
              >
                Previous
              </Button>
              <Text size="sm">
                Page {page} of {totalPages}
              </Text>
              <Button
                variant="subtle"
                size="xs"
                disabled={page >= totalPages}
                onClick={() => setPage((p) => p + 1)}
              >
                Next
              </Button>
            </Group>
          )}
        </Stack>
      </Modal>
    </>
  );
}
