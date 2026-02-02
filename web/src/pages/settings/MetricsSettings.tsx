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
          <Text fw={500}>{library.seriesCount.toLocaleString()}</Text>
        </div>
        <div>
          <Text size="xs" c="dimmed">
            Books
          </Text>
          <Text fw={500}>{library.bookCount.toLocaleString()}</Text>
        </div>
        <div>
          <Text size="xs" c="dimmed">
            Size
          </Text>
          <Text fw={500}>{formatBytes(library.totalSize)}</Text>
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
              {metrics.taskType.replace(/_/g, " ")}
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
          <Text size="sm">{formatDuration(metrics.avgDurationMs)}</Text>
        </Table.Td>
        <Table.Td>
          <Tooltip label="Median / 95th percentile">
            <Text size="sm" c="dimmed">
              {formatDuration(metrics.p50DurationMs)} /{" "}
              {formatDuration(metrics.p95DurationMs)}
            </Text>
          </Tooltip>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{metrics.itemsProcessed.toLocaleString()}</Text>
        </Table.Td>
        <Table.Td>
          {metrics.lastError ? (
            <Tooltip label={metrics.lastError}>
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
                      metrics.errorRatePct > 5
                        ? "red"
                        : metrics.errorRatePct > 1
                          ? "yellow"
                          : undefined
                    }
                  >
                    {metrics.errorRatePct.toFixed(2)}%
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Min Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.minDurationMs)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Max Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.maxDurationMs)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    P50 Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.p50DurationMs)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    P95 Duration
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.p95DurationMs)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Avg Queue Wait
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatDuration(metrics.avgQueueWaitMs)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Bytes Processed
                  </Text>
                  <Text size="sm" fw={500}>
                    {formatBytes(metrics.bytesProcessed)}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Throughput
                  </Text>
                  <Text size="sm" fw={500}>
                    {metrics.throughputPerSec.toFixed(1)}/sec
                  </Text>
                </div>
                {metrics.lastErrorAt && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Error At
                    </Text>
                    <Text size="sm" fw={500} c="red">
                      {new Date(metrics.lastErrorAt).toLocaleString()}
                    </Text>
                  </div>
                )}
              </SimpleGrid>
              {metrics.lastError && (
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
                    {metrics.lastError}
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
          value={(metrics.libraryCount ?? 0).toLocaleString()}
          icon={IconFolder}
          color="blue"
        />
        <StatCard
          title="Series"
          value={(metrics.seriesCount ?? 0).toLocaleString()}
          icon={IconBooks}
          color="teal"
        />
        <StatCard
          title="Books"
          value={(metrics.bookCount ?? 0).toLocaleString()}
          icon={IconBook}
          color="violet"
        />
        <StatCard
          title="Users"
          value={(metrics.userCount ?? 0).toLocaleString()}
          icon={IconUsers}
          color="orange"
        />
      </SimpleGrid>

      <SimpleGrid cols={{ base: 1, sm: 2 }}>
        <StatCard
          title="Total Book Size"
          value={formatBytes(metrics.totalBookSize ?? 0)}
          icon={IconDatabase}
          color="cyan"
        />
        <StatCard
          title="Database Size"
          value={formatBytes(metrics.databaseSize ?? 0)}
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
  const byType = metrics.byType ?? [];

  // Calculate aggregates from by_type data
  const totalRetried = byType.reduce((sum, t) => sum + (t.retried ?? 0), 0);
  const successRate =
    summary.totalExecuted > 0
      ? ((summary.totalSucceeded ?? 0) / summary.totalExecuted) * 100
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
            {(summary.totalExecuted ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Succeeded
          </Text>
          <Text fw={700} size="xl" c="green">
            {(summary.totalSucceeded ?? 0).toLocaleString()}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Failed
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.totalFailed ?? 0) > 0 ? "red" : undefined}
          >
            {(summary.totalFailed ?? 0).toLocaleString()}
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
            {(summary.tasksPerMinute ?? 0).toFixed(1)}/min
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
                {formatDuration(summary.avgDurationMs ?? 0)}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Avg Queue Wait
              </Text>
              <Text fw={500} size="lg">
                {formatDuration(summary.avgQueueWaitMs ?? 0)}
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
                {queue.pendingCount ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Processing
              </Text>
              <Text fw={500} size="lg">
                {queue.processingCount ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Stale
              </Text>
              <Text
                fw={500}
                size="lg"
                c={(queue.staleCount ?? 0) > 0 ? "red" : "green"}
              >
                {queue.staleCount ?? 0}
              </Text>
            </div>
            <div>
              <Text size="xs" c="dimmed">
                Oldest Pending
              </Text>
              <Text fw={500} size="lg">
                {queue.oldestPendingAgeMs
                  ? formatDuration(queue.oldestPendingAgeMs)
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
                .sort((a, b) => a.taskType.localeCompare(b.taskType))
                .map((taskMetrics) => (
                  <TaskTypeRow
                    key={taskMetrics.taskType}
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
    metrics.requestsTotal > 0
      ? (
          ((metrics.requestsSuccess ?? 0) / metrics.requestsTotal) *
          100
        ).toFixed(1)
      : "0";

  const healthColor =
    metrics.healthStatus === "healthy"
      ? "green"
      : metrics.healthStatus === "degraded"
        ? "yellow"
        : metrics.healthStatus === "unhealthy"
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
              {metrics.pluginName}
            </Text>
          </Group>
        </Table.Td>
        <Table.Td>
          <Text size="sm">{metrics.requestsTotal.toLocaleString()}</Text>
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
          <Text size="sm">{formatDuration(metrics.avgDurationMs ?? 0)}</Text>
        </Table.Td>
        <Table.Td>
          <Text size="sm">
            {(metrics.rateLimitRejections ?? 0).toLocaleString()}
          </Text>
        </Table.Td>
        <Table.Td>
          <Badge color={healthColor} size="sm" variant="light">
            {metrics.healthStatus}
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
                    {(metrics.requestsSuccess ?? 0).toLocaleString()}
                  </Text>
                </div>
                <div>
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                    Failed
                  </Text>
                  <Text
                    size="sm"
                    fw={500}
                    c={(metrics.requestsFailed ?? 0) > 0 ? "red" : undefined}
                  >
                    {(metrics.requestsFailed ?? 0).toLocaleString()}
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
                      (metrics.errorRatePct ?? 0) > 10
                        ? "red"
                        : (metrics.errorRatePct ?? 0) > 5
                          ? "yellow"
                          : undefined
                    }
                  >
                    {(metrics.errorRatePct ?? 0).toFixed(2)}%
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
                      (metrics.rateLimitRejections ?? 0) > 0
                        ? "yellow"
                        : undefined
                    }
                  >
                    {(metrics.rateLimitRejections ?? 0).toLocaleString()}
                  </Text>
                </div>
                {metrics.lastSuccess && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Success
                    </Text>
                    <Text size="sm" fw={500}>
                      {new Date(metrics.lastSuccess).toLocaleString()}
                    </Text>
                  </div>
                )}
                {metrics.lastFailure && (
                  <div>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Last Failure
                    </Text>
                    <Text size="sm" fw={500} c="red">
                      {new Date(metrics.lastFailure).toLocaleString()}
                    </Text>
                  </div>
                )}
              </SimpleGrid>

              {/* Method breakdown */}
              {metrics.byMethod && Object.keys(metrics.byMethod).length > 0 && (
                <Box mt="md">
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600} mb="xs">
                    By Method
                  </Text>
                  <SimpleGrid cols={{ base: 1, sm: 2, md: 3 }} spacing="xs">
                    {Object.entries(metrics.byMethod).map(
                      ([method, methodMetrics]) => (
                        <Paper key={method} p="xs" withBorder>
                          <Group justify="space-between" mb={4}>
                            <Text size="xs" fw={500}>
                              {method}
                            </Text>
                            <Badge size="xs" variant="light">
                              {methodMetrics.requestsTotal} calls
                            </Badge>
                          </Group>
                          <Group gap="xs">
                            <Text size="xs" c="dimmed">
                              {methodMetrics.requestsSuccess} ok
                            </Text>
                            {(methodMetrics.requestsFailed ?? 0) > 0 && (
                              <Text size="xs" c="red">
                                {methodMetrics.requestsFailed} failed
                              </Text>
                            )}
                            <Text size="xs" c="dimmed">
                              avg {formatDuration(methodMetrics.avgDurationMs)}
                            </Text>
                          </Group>
                        </Paper>
                      ),
                    )}
                  </SimpleGrid>
                </Box>
              )}

              {/* Failure breakdown */}
              {metrics.failureCounts &&
                Object.keys(metrics.failureCounts).length > 0 && (
                  <Box mt="md">
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600} mb="xs">
                      Failures by Type
                    </Text>
                    <Group gap="xs">
                      {Object.entries(metrics.failureCounts).map(
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
    summary.totalRequests > 0
      ? ((summary.totalSuccess ?? 0) / summary.totalRequests) * 100
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
            {summary.totalPlugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Healthy
          </Text>
          <Text fw={700} size="xl" c="green">
            {summary.healthyPlugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Degraded
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.degradedPlugins ?? 0) > 0 ? "yellow" : undefined}
          >
            {summary.degradedPlugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Unhealthy
          </Text>
          <Text
            fw={700}
            size="xl"
            c={(summary.unhealthyPlugins ?? 0) > 0 ? "red" : undefined}
          >
            {summary.unhealthyPlugins}
          </Text>
        </Paper>
        <Paper p="md" withBorder>
          <Text c="dimmed" size="xs" tt="uppercase" fw={700}>
            Total Requests
          </Text>
          <Text fw={700} size="xl">
            {(summary.totalRequests ?? 0).toLocaleString()}
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
                {(summary.totalSuccess ?? 0).toLocaleString()}
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
                c={(summary.totalFailed ?? 0) > 0 ? "red" : undefined}
              >
                {(summary.totalFailed ?? 0).toLocaleString()}
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
                  (summary.totalRateLimitRejections ?? 0) > 0
                    ? "yellow"
                    : undefined
                }
              >
                {(summary.totalRateLimitRejections ?? 0).toLocaleString()}
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
                .sort((a, b) => a.pluginName.localeCompare(b.pluginName))
                .map((plugin) => (
                  <PluginMetricsRow key={plugin.pluginId} metrics={plugin} />
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
        message: `Deleted ${result.deletedCount} old metric records`,
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
