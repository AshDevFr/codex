import {
  Alert,
  Badge,
  Card,
  Code,
  Group,
  Modal,
  Stack,
  Text,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import type { DryRunResponse } from "@/api/libraryJobs";

interface JobDryRunModalProps {
  opened: boolean;
  onClose: () => void;
  result: DryRunResponse | undefined;
}

export function JobDryRunModal({
  opened,
  onClose,
  result,
}: JobDryRunModalProps) {
  return (
    <Modal opened={opened} onClose={onClose} title="Preview changes" size="lg">
      {!result ? (
        <Text c="dimmed">No preview available.</Text>
      ) : (
        <Stack>
          <Alert variant="light" icon={<IconInfoCircle size={16} />}>
            <Text size="sm">
              <strong>{result.totalEligible}</strong> series eligible · skipped:{" "}
              {result.estSkippedNoId} no external ID,{" "}
              {result.estSkippedRecentlySynced} recently synced
            </Text>
          </Alert>

          {result.planFailure && (
            <Alert color="red" variant="light">
              Plan failure: <Code>{result.planFailure}</Code>
            </Alert>
          )}

          {result.sample.length === 0 ? (
            <Text c="dimmed" size="sm">
              No series in the sample.
            </Text>
          ) : (
            <Stack gap="xs">
              {result.sample.map((s) => (
                <Card key={s.seriesId} withBorder padding="sm">
                  <Stack gap={4}>
                    <Group gap="xs">
                      <Text fw={500}>{s.seriesName}</Text>
                      <Badge size="xs" variant="light">
                        candidate
                      </Badge>
                    </Group>
                    {Object.entries(s.changes).map(([field, change]) => (
                      <Text key={field} size="xs">
                        <code>{field}</code>:{" "}
                        <Text component="span" c="red" td="line-through">
                          {JSON.stringify(change.before)}
                        </Text>{" "}
                        →{" "}
                        <Text component="span" c="green">
                          {JSON.stringify(change.after)}
                        </Text>
                      </Text>
                    ))}
                  </Stack>
                </Card>
              ))}
            </Stack>
          )}
        </Stack>
      )}
    </Modal>
  );
}
