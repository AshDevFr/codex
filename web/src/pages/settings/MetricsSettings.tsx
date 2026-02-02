import {
  Badge,
  Box,
  Button,
  Card,
  Center,
  Grid,
  Group,
  Loader,
  Paper,
  Progress,
  RingProgress,
  SimpleGrid,
  Stack,
  Table,
  Tabs,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconBook,
  IconBooks,
  IconChartBar,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconClock,
  IconDatabase,
  IconFolder,
  IconPlugConnected,
  IconRefresh,
  IconTrash,
  IconUsers,
  IconX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  LibraryMetricsDto,
  MetricsDto,
  PluginMetricsDto,
  PluginMetricsResponse,
  TaskMetricsResponse,
  TaskTypeMetricsDto,
} from "@/api/metrics";
import { metricsApi } from "@/api/metrics";

// Helper to format bytes
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Number.parseFloat((bytes / k ** i).toFixed(2))} ${sizes[i]}`;
}

// Helper to format duration
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms.toFixed(0)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
}

// Stat card component
function StatCard({
  title,
  value,
  icon: Icon,
  color = "blue",
}: {
  title: string;
  value: string | number;
  icon: React.ComponentType<{ size?: number }>;
  color?: string;
}) {
  return (
    <Paper p="md" withBorder>
      <Group>
        <RingProgress
          size={80}
          roundCaps
          thickness={8}
          sections={[{ value: 100, color }]}
          label={
            <Center>
              <Icon size={22} />
            </Center>
          }
        />
        <div>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            {title}
          </Text>
          <Text fw={700} size="xl">
            {value}
          </Text>
        </div>
      </Group>
    </Paper>
  );
}

// Library metrics card
function LibraryCard({ library }: { library: LibraryMetricsDto }) {
  return (
    <Card withBorder shadow="sm">
      <Group justify="space-between" mb="xs">
        <Group gap="xs">
          <IconFolder size={18} />
          <Text fw={500}>{library.name}</Text>
        </Group>
      </Group>
      <SimpleGrid cols={3} spacing="xs">
        <div>
          <Text size="xs" c="dimmed">
            Series
          </Text>
          <Text fw={500}>{library.series_count.toLocaleString()}</Text>
        </div>
        <div>
          <Text size="xs" c="dimmed">
            Books
          </Text>
          <Text fw={500}>{library.book_count.toLocaleString()}</Text>
        </div>
        <div>
          <Text size="xs" c="dimmed">
            Size
          </Text>
          <Text fw={500}>{formatBytes(library.total_size)}</Text>
        </div>
      </SimpleGrid>
    </Card>
  );
}

// Task type metrics row with expandable details
function TaskTypeRow({ metrics }: { metrics: TaskTypeMetricsDto }) {
  const [opened, { toggle }] = useDisclosure(false);
  const successRate =
    metrics.executed > 0
      ? ((metrics.succeeded / metrics.executed) * 100).toFixed(1)
      : "0";

  return (
    <>
      <Table.Tr onClick={toggle} style={{ cursor: "pointer" }}>
        <Table.Td>
          <Group gap="xs">
            {opened ? (
              <IconChevronDown size={14} />
            ) : (
              <IconChevronRight size={14} />
            )}
            <Text fw={500} size="sm">
              {metrics.task_type.replace(/_/g, " ")}
            </Text>
          </Group>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{metrics.executed.toLocaleString()}</Text>
        </Table.Td>
        <Table.Td>
          <Group gap="xs">
            <Progress
              value={Number.parseFloat(successRate)}
              color={
                Number.parseFloat(successRate) >= 95
                  ? "green"
                  : Number.parseFloat(successRate) >= 80
                    ? "yellow"
                    : "red"
              }
              size="sm"
              w={60}
            />
            <Text size="sm">{successRate}%</Text>
          </Group>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{formatDuration(metrics.avg_duration_ms)}</Text>
        </Table.Td>
        <Table.Td>
          <Tooltip label="Median / 95th percentile">
            <Text size="sm" c="dimmed">
              {formatDuration(metrics.p50_duration_ms)} /{" "}
              {formatDuration(metrics.p95_duration_ms)}
            </Text>
          </Tooltip>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{metrics.items_processed.toLocaleString()}</Text>
        </Table.Td>
        <Table.Td>
          {metrics.last_error ? (
            <Tooltip label={metrics.last_error}>
              <Badge
                color="red"
                size="sm"
                variant="light"
                leftSection={<IconAlertCircle size={12} />}
              >
                {metrics.failed} errors
              </Badge>
            </Tooltip>
          ) : (
            <Badge color="green" size="sm" variant="light">
              Healthy
            </Badge>
          )}
        </Table.Td>
      </Table.Tr>
      {opened && (
        <Table.Tr>
          <Table.Td colSpan={7} p={0}>
            <Box bg="var(--mantine-color-gray-light)" p="md">
              <SimpleGrid cols={{ base: 2, sm: 4, md: 6 }} spacing="md">
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Succeeded
                  </Text>
                  <Text size="sm" fw={500} c="green">
                    {metrics.succeeded.toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Failed
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={metrics.failed > 0 ? "red" : undefined}
                  >
                    {metrics.failed.toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Retried
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={metrics.retried > 0 ? "yellow" : undefined}
                  >
                    {metrics.retried.toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Error Rate
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={
                      metrics.error_rate_pct > 5
                        ? "red"
                        : metrics.error_rate_pct > 1
                          ? "yellow"
                          : undefined
                    }
                  >
                    {metrics.error_rate_pct.toFixed(2)}%
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Min Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.min_duration_ms)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Max Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.max_duration_ms)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    P50 Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.p50_duration_ms)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    P95 Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.p95_duration_ms)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Avg Queue Wait
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.avg_queue_wait_ms)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Bytes Processed
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatBytes(metrics.bytes_processed)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Throughput
                  </Text>
                  <Text size="sm" fw={500}>
                    {metrics.throughput_per_sec.toFixed(1)}/sec
                  </Text>
                </div>
                {metrics.last_error_at && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Error At
                    </Text>
                    <Text size="sm" fw={500} c="red">
                      {new Date(metrics.last_error_at).toLocaleString()}
                    </Text>
                  </div>
                )}
              </SimpleGrid>
              {metrics.last_error && (
                <Box
                  mt="md"
                  p="sm"
                  bg="var(--mantine-color-red-light)"
                  style={{ borderRadius: 4 }}
                >
                  <Group gap="xs" mb={4}>
                    <IconAlertCircle
                      size={14}
                      color="var(--mantine-color-red-filled)"
                    />
                    <Text size="xs" fw={600} c="red">
                      Last Error
                    </Text>
                  </Group>
                  <Text size="sm" style={{ fontFamily: "monospace" }}>
                    {metrics.last_error}
                  </Text>
                </Box>
              )}
            </Box>
          </Table.Td>
        </Table.Tr>
      )}
    </>
  );
}

// Inventory tab content
function InventoryTab({ metrics }: { metrics: MetricsDto }) {
  return (
    <Stack gap="lg">
      <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
        <StatCard
          title="Libraries"
          value={(metrics.library_count ?? 0).toLocaleString()}
          icon={IconFolder}
          color="blue"
        />
        <StatCard
          title="Series"
          value={(metrics.series_count ?? 0).toLocaleString()}
          icon={IconBooks}
          color="teal"
        />
        <StatCard
          title="Books"
          value={(metrics.book_count ?? 0).toLocaleString()}
          icon={IconBook}
          color="violet"
        />
        <StatCard
          title="Users"
          value={(metrics.user_count ?? 0).toLocaleString()}
          icon={IconUsers}
          color="orange"
        />
      </SimpleGrid>

      <SimpleGrid cols={{ base: 1, sm: 2 }}>
        <StatCard
          title="Total Book Size"
          value={formatBytes(metrics.total_book_size ?? 0)}
          icon={IconDatabase}
          color="cyan"
        />
        <StatCard
          title="Database Size"
          value={formatBytes(metrics.database_size ?? 0)}
          icon={IconDatabase}
          color="pink"
        />
      </SimpleGrid>

      {metrics.libraries && metrics.libraries.length > 0 && (
        <div>
          <Title order={4} mb="md">
            Libraries Breakdown
          </Title>
          <Grid>
            {metrics.libraries.map((library) => (
              <Grid.Col key={library.id} span={{ base: 12, sm: 6, md: 4 }}>
                <LibraryCard library={library} />
              </Grid.Col>
            ))}
          </Grid>
        </div>
      )}
    </Stack>
  );
}

// Task metrics tab content
function TaskMetricsTab({ metrics }: { metrics: TaskMetricsResponse }) {
  const summary = metrics.summary ?? {
    total_executed: 0,
    total_succeeded: 0,
    total_failed: 0,
    avg_duration_ms: 0,
    avg_queue_wait_ms: 0,
    tasks_per_minute: 0,
  };
  const queue = metrics.queue ?? {
    pending_count: 0,
    processing_count: 0,
    stale_count: 0,
    oldest_pending_age_ms: null,
  };
  const byType = metrics.by_type ?? [];

  // Calculate aggregates from by_type data
  const totalRetried = byType.reduce((sum, t) => sum + (t.retried ?? 0), 0);
  const successRate =
    summary.total_executed > 0
      ? ((summary.total_succeeded ?? 0) / summary.total_executed) * 100
      : 0;

  return (
    <Stack gap="lg">
      {/* Summary cards */}
      <SimpleGrid cols={{ base: 2, sm: 3, lg: 6 }}>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Total Executed
          </Text>
          <Text fw={700} size="xl">
            {(summary.total_executed ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Succeeded
          </Text>
          <Text fw={700} size="xl" c="green">
            {(summary.total_succeeded ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Failed
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.total_failed ?? 0) > 0 ? "red" : undefined}
          >
            {(summary.total_failed ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Retried
          </Text>
          <Text fw={700} size="xl" c={totalRetried > 0 ? "yellow" : undefined}>
            {totalRetried.toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Success Rate
          </Text>
          <Group gap="xs" align="baseline">
            <Text
              fw={700}
              size="xl"
              c={
                successRate >= 95
                  ? "green"
                  : successRate >= 80
                    ? "yellow"
                    : "red"
              }
            >
              {successRate.toFixed(1)}%
            </Text>
          </Group>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Throughput
          </Text>
          <Text fw={700} size="xl">
            {(summary.tasks_per_minute ?? 0).toFixed(1)}/min
          </Text>
        </Paper>
      </SimpleGrid>

      {/* Duration and queue stats */}
      <SimpleGrid cols={{ base: 1, sm: 2 }}>
        <Paper p="md" withBorder>
          <Title order={5} mb="md">
            Duration Stats
          </Title>
          <SimpleGrid cols={2}>
            <div>
              <Text size="xs" c="dimmed">
                Avg Duration
              </Text>
              <Text fw={500} size="lg">
                {formatDuration(summary.avg_duration_ms ?? 0)}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Avg Queue Wait
              </Text>
              <Text fw={500} size="lg">
                {formatDuration(summary.avg_queue_wait_ms ?? 0)}
              </Text>
            </div>
          </SimpleGrid>
        </Paper>

        {/* Queue health */}
        <Paper p="md" withBorder>
          <Title order={5} mb="md">
            Queue Health
          </Title>
          <SimpleGrid cols={{ base: 2, sm: 4 }}>
            <div>
              <Text size="xs" c="dimmed">
                Pending
              </Text>
              <Text fw={500} size="lg">
                {queue.pending_count ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Processing
              </Text>
              <Text fw={500} size="lg">
                {queue.processing_count ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Stale
              </Text>
              <Text
                fw={500}
                size="lg"
                c={(queue.stale_count ?? 0) > 0 ? "red" : "green"}
              >
                {queue.stale_count ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Oldest Pending
              </Text>
              <Text fw={500} size="lg">
                {queue.oldest_pending_age_ms
                  ? formatDuration(queue.oldest_pending_age_ms)
                  : "-"}
              </Text>
            </div>
          </SimpleGrid>
        </Paper>
      </SimpleGrid>

      {/* Task type breakdown */}
      {byType.length > 0 && (
        <div>
          <Title order={5} mb="md">
            Task Performance by Type
          </Title>
          <Table striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Task Type</Table.Th>
                <Table.Th>Executed</Table.Th>
                <Table.Th>Success Rate</Table.Th>
                <Table.Th>Avg Duration</Table.Th>
                <Table.Th>P50 / P95</Table.Th>
                <Table.Th>Items Processed</Table.Th>
                <Table.Th>Status</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {[...byType]
                .sort((a, b) => a.task_type.localeCompare(b.task_type))
                .map((taskMetrics) => (
                  <TaskTypeRow
                    key={taskMetrics.task_type}
                    metrics={taskMetrics}
                  />
                ))}
            </Table.Tbody>
          </Table>
        </div>
      )}
    </Stack>
  );
}

// Plugin metrics row with expandable details
function PluginMetricsRow({ metrics }: { metrics: PluginMetricsDto }) {
  const [opened, { toggle }] = useDisclosure(false);
  const successRate =
    metrics.requests_total > 0
      ? (
          ((metrics.requests_success ?? 0) / metrics.requests_total) *
          100
        ).toFixed(1)
      : "0";

  const healthColor =
    metrics.health_status === "healthy"
      ? "green"
      : metrics.health_status === "degraded"
        ? "yellow"
        : metrics.health_status === "unhealthy"
          ? "red"
          : "gray";

  return (
    <>
      <Table.Tr onClick={toggle} style={{ cursor: "pointer" }}>
        <Table.Td>
          <Group gap="xs">
            {opened ? (
              <IconChevronDown size={14} />
            ) : (
              <IconChevronRight size={14} />
            )}
            <Text fw={500} size="sm">
              {metrics.plugin_name}
            </Text>
          </Group>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{metrics.requests_total.toLocaleString()}</Text>
        </Table.Td>
        <Table.Td>
          <Group gap="xs">
            <Progress
              value={Number.parseFloat(successRate)}
              color={
                Number.parseFloat(successRate) >= 95
                  ? "green"
                  : Number.parseFloat(successRate) >= 80
                    ? "yellow"
                    : "red"
              }
              size="sm"
              w={60}
            />
            <Text size="sm">{successRate}%</Text>
          </Group>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{formatDuration(metrics.avg_duration_ms ?? 0)}</Text>
        </Table.Td>
        <Table.Td>
          <Text size="sm">
            {(metrics.rate_limit_rejections ?? 0).toLocaleString()}
          </Text>
        </Table.Td>
        <Table.Td>
          <Badge color={healthColor} size="sm" variant="light">
            {metrics.health_status}
          </Badge>
        </Table.Td>
      </Table.Tr>
      {opened && (
        <Table.Tr>
          <Table.Td colSpan={6} p={0}>
            <Box bg="var(--mantine-color-gray-light)" p="md">
              <SimpleGrid cols={{ base: 2, sm: 4, md: 6 }} spacing="md">
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Succeeded
                  </Text>
                  <Text size="sm" fw={500} c="green">
                    {(metrics.requests_success ?? 0).toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Failed
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={(metrics.requests_failed ?? 0) > 0 ? "red" : undefined}
                  >
                    {(metrics.requests_failed ?? 0).toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Error Rate
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={
                      (metrics.error_rate_pct ?? 0) > 10
                        ? "red"
                        : (metrics.error_rate_pct ?? 0) > 5
                          ? "yellow"
                          : undefined
                    }
                  >
                    {(metrics.error_rate_pct ?? 0).toFixed(2)}%
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Rate Limit Hits
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={
                      (metrics.rate_limit_rejections ?? 0) > 0
                        ? "yellow"
                        : undefined
                    }
                  >
                    {(metrics.rate_limit_rejections ?? 0).toLocaleString()}
                  </Text>
                </div>
                {metrics.last_success && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Success
                    </Text>
                    <Text size="sm" fw={500}>
                      {new Date(metrics.last_success).toLocaleString()}
                    </Text>
                  </div>
                )}
                {metrics.last_failure && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Failure
                    </Text>
                    <Text size="sm" fw={500} c="red">
                      {new Date(metrics.last_failure).toLocaleString()}
                    </Text>
                  </div>
                )}
              </SimpleGrid>

              {/* Method breakdown */}
              {metrics.by_method &&
                Object.keys(metrics.by_method).length > 0 && (
                  <Box mt="md">
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600} mb="xs">
                      By Method
                    </Text>
                    <SimpleGrid cols={{ base: 1, sm: 2, md: 3 }} spacing="xs">
                      {Object.entries(metrics.by_method).map(
                        ([method, methodMetrics]) => (
                          <Paper key={method} p="xs" withBorder>
                            <Group justify="space-between" mb={4}>
                              <Text size="xs" fw={500}>
                                {method}
                              </Text>
                              <Badge size="xs" variant="light">
                                {methodMetrics.requests_total} calls
                              </Badge>
                            </Group>
                            <Group gap="xs">
                              <Text size="xs" c="dimmed">
                                {methodMetrics.requests_success} ok
                              </Text>
                              {(methodMetrics.requests_failed ?? 0) > 0 && (
                                <Text size="xs" c="red">
                                  {methodMetrics.requests_failed} failed
                                </Text>
                              )}
                              <Text size="xs" c="dimmed">
                                avg{" "}
                                {formatDuration(methodMetrics.avg_duration_ms)}
                              </Text>
                            </Group>
                          </Paper>
                        ),
                      )}
                    </SimpleGrid>
                  </Box>
                )}

              {/* Failure breakdown */}
              {metrics.failure_counts &&
                Object.keys(metrics.failure_counts).length > 0 && (
                  <Box mt="md">
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600} mb="xs">
                      Failures by Type
                    </Text>
                    <Group gap="xs">
                      {Object.entries(metrics.failure_counts).map(
                        ([code, count]) => (
                          <Badge
                            key={code}
                            color="red"
                            variant="light"
                            size="sm"
                          >
                            {code}: {count}
                          </Badge>
                        ),
                      )}
                    </Group>
                  </Box>
                )}
            </Box>
          </Table.Td>
        </Table.Tr>
      )}
    </>
  );
}

// Plugin metrics tab content
function PluginMetricsTab({ metrics }: { metrics: PluginMetricsResponse }) {
  const summary = metrics.summary;
  const plugins = metrics.plugins ?? [];

  const successRate =
    summary.total_requests > 0
      ? ((summary.total_success ?? 0) / summary.total_requests) * 100
      : 0;

  return (
    <Stack gap="lg">
      {/* Summary cards */}
      <SimpleGrid cols={{ base: 2, sm: 3, lg: 6 }}>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Total Plugins
          </Text>
          <Text fw={700} size="xl">
            {summary.total_plugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Healthy
          </Text>
          <Text fw={700} size="xl" c="green">
            {summary.healthy_plugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Degraded
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.degraded_plugins ?? 0) > 0 ? "yellow" : undefined}
          >
            {summary.degraded_plugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Unhealthy
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.unhealthy_plugins ?? 0) > 0 ? "red" : undefined}
          >
            {summary.unhealthy_plugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Total Requests
          </Text>
          <Text fw={700} size="xl">
            {(summary.total_requests ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Success Rate
          </Text>
          <Text
            fw={700}
            size="xl"
            c={
              successRate >= 95 ? "green" : successRate >= 80 ? "yellow" : "red"
            }
          >
            {successRate.toFixed(1)}%
          </Text>
        </Paper>
      </SimpleGrid>

      {/* Additional stats */}
      <SimpleGrid cols={{ base: 1, sm: 3 }}>
        <Paper p="md" withBorder>
          <Group gap="xs">
            <IconCheck size={18} color="var(--mantine-color-green-6)" />
            <div>
              <Text size="xs" c="dimmed">
                Successful Requests
              </Text>
              <Text fw={500} size="lg">
                {(summary.total_success ?? 0).toLocaleString()}
              </Text>
            </div>
          </Group>
        </Paper>
        <Paper p="md" withBorder>
          <Group gap="xs">
            <IconX size={18} color="var(--mantine-color-red-6)" />
            <div>
              <Text size="xs" c="dimmed">
                Failed Requests
              </Text>
              <Text
                fw={500}
                size="lg"
                c={(summary.total_failed ?? 0) > 0 ? "red" : undefined}
              >
                {(summary.total_failed ?? 0).toLocaleString()}
              </Text>
            </div>
          </Group>
        </Paper>
        <Paper p="md" withBorder>
          <Group gap="xs">
            <IconClock size={18} color="var(--mantine-color-yellow-6)" />
            <div>
              <Text size="xs" c="dimmed">
                Rate Limit Rejections
              </Text>
              <Text
                fw={500}
                size="lg"
                c={
                  (summary.total_rate_limit_rejections ?? 0) > 0
                    ? "yellow"
                    : undefined
                }
              >
                {(summary.total_rate_limit_rejections ?? 0).toLocaleString()}
              </Text>
            </div>
          </Group>
        </Paper>
      </SimpleGrid>

      {/* Plugin breakdown table */}
      {plugins.length > 0 ? (
        <div>
          <Title order={5} mb="md">
            Plugin Performance
          </Title>
          <Table striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Plugin</Table.Th>
                <Table.Th>Requests</Table.Th>
                <Table.Th>Success Rate</Table.Th>
                <Table.Th>Avg Duration</Table.Th>
                <Table.Th>Rate Limited</Table.Th>
                <Table.Th>Health</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {[...plugins]
                .sort((a, b) => a.plugin_name.localeCompare(b.plugin_name))
                .map((plugin) => (
                  <PluginMetricsRow key={plugin.plugin_id} metrics={plugin} />
                ))}
            </Table.Tbody>
          </Table>
        </div>
      ) : (
        <Paper p="xl" withBorder>
          <Center>
            <Stack align="center" gap="xs">
              <IconPlugConnected
                size={48}
                color="var(--mantine-color-dimmed)"
              />
              <Text c="dimmed" size="sm">
                No plugin metrics available yet
              </Text>
              <Text c="dimmed" size="xs">
                Plugin metrics will appear here after plugins are used
              </Text>
            </Stack>
          </Center>
        </Paper>
      )}
    </Stack>
  );
}

export function MetricsSettings() {
  const queryClient = useQueryClient();

  // Fetch inventory metrics
  const {
    data: inventoryMetrics,
    isLoading: inventoryLoading,
    error: inventoryError,
  } = useQuery({
    queryKey: ["metrics", "inventory"],
    queryFn: metricsApi.getInventory,
  });

  // Fetch task metrics
  const {
    data: taskMetrics,
    isLoading: taskLoading,
    error: taskError,
  } = useQuery({
    queryKey: ["metrics", "tasks"],
    queryFn: metricsApi.getTaskMetrics,
  });

  // Fetch plugin metrics
  const {
    data: pluginMetrics,
    isLoading: pluginLoading,
    error: pluginError,
  } = useQuery({
    queryKey: ["metrics", "plugins"],
    queryFn: metricsApi.getPluginMetrics,
  });

  // Cleanup mutation
  const cleanupMutation = useMutation({
    mutationFn: metricsApi.cleanupTaskMetrics,
    onSuccess: (result) => {
      notifications.show({
        title: "Cleanup Complete",
        message: `Deleted ${result.deleted_count} old metric records`,
        color: "green",
      });
      queryClient.invalidateQueries({ queryKey: ["metrics", "tasks"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Cleanup Failed",
        message: error.message || "Failed to cleanup metrics",
        color: "red",
      });
    },
  });

  const handleRefresh = () => {
    queryClient.invalidateQueries({ queryKey: ["metrics"] });
  };

  if (inventoryLoading || taskLoading || pluginLoading) {
    return (
      <Center h={400}>
        <Loader size="lg" />
      </Center>
    );
  }

  if (inventoryError || taskError || pluginError) {
    return (
      <Center h={400}>
        <Stack align="center" gap="md">
          <Text c="red">Failed to load metrics</Text>
          <Button
            onClick={handleRefresh}
            leftSection={<IconRefresh size={16} />}
          >
            Retry
          </Button>
        </Stack>
      </Center>
    );
  }

  return (
    <Stack gap="lg">
      <Group justify="space-between">
        <div>
          <Title order={2}>Metrics</Title>
          <Text c="dimmed" size="sm">
            System statistics and performance metrics
          </Text>
        </div>
        <Group>
          <Button
            variant="light"
            leftSection={<IconTrash size={16} />}
            onClick={() => cleanupMutation.mutate()}
            loading={cleanupMutation.isPending}
          >
            Cleanup Old Data
          </Button>
          <Button
            variant="light"
            leftSection={<IconRefresh size={16} />}
            onClick={handleRefresh}
          >
            Refresh
          </Button>
        </Group>
      </Group>

      <Tabs defaultValue="inventory">
        <Tabs.List>
          <Tabs.Tab value="inventory" leftSection={<IconDatabase size={16} />}>
            Inventory
          </Tabs.Tab>
          <Tabs.Tab value="tasks" leftSection={<IconChartBar size={16} />}>
            Task Performance
          </Tabs.Tab>
          <Tabs.Tab
            value="plugins"
            leftSection={<IconPlugConnected size={16} />}
          >
            Plugins
          </Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="inventory" pt="md">
          {inventoryMetrics && <InventoryTab metrics={inventoryMetrics} />}
        </Tabs.Panel>

        <Tabs.Panel value="tasks" pt="md">
          {taskMetrics && <TaskMetricsTab metrics={taskMetrics} />}
        </Tabs.Panel>

        <Tabs.Panel value="plugins" pt="md">
          {pluginMetrics && <PluginMetricsTab metrics={pluginMetrics} />}
        </Tabs.Panel>
      </Tabs>
    </Stack>
  );
}
