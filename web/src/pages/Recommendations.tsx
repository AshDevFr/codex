import {
  Alert,
  Box,
  Button,
  Group,
  Loader,
  SimpleGrid,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconPlugConnected,
  IconRefresh,
  IconSparkles,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import {
  type RecommendationsResponse,
  recommendationsApi,
} from "@/api/recommendations";
import { RecommendationCard } from "@/components/recommendations/RecommendationCard";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { ApiError } from "@/types";

export function Recommendations() {
  useDocumentTitle("Recommendations");

  const queryClient = useQueryClient();
  const { activeTasks } = useTaskProgress();

  // Track whether we have an active recommendation task (via SSE)
  const recTask = activeTasks.find(
    (t) => t.taskType === "user_plugin_recommendations",
  );
  const prevRecTaskRef = useRef(recTask);

  // Track whether a task is running — drives query polling as a fallback for SSE.
  // Set true on refresh or when the API reports an active task; cleared when the
  // API response no longer has a taskStatus and SSE shows no active task.
  const [taskRunning, setTaskRunning] = useState(false);

  // When a recommendation task completes, invalidate the query to fetch fresh data
  useEffect(() => {
    const prev = prevRecTaskRef.current;
    prevRecTaskRef.current = recTask;

    // If previous task existed and was running/pending, but now it's gone or completed
    if (
      prev &&
      (prev.status === "running" || prev.status === "pending") &&
      (!recTask || recTask.status === "completed")
    ) {
      queryClient.invalidateQueries({ queryKey: ["recommendations"] });
    }
  }, [recTask, queryClient]);

  // Fetch recommendations — polls every 3s while a task is running so the page
  // updates promptly even when SSE events don't reach the browser (e.g. in
  // split web/worker deployments).
  const {
    data: recData,
    isLoading,
    error,
  } = useQuery<RecommendationsResponse, ApiError>({
    queryKey: ["recommendations"],
    queryFn: recommendationsApi.get,
    retry: false,
    refetchInterval: taskRunning ? 3000 : false,
  });

  // Determine if a task is active (from response or SSE)
  const isTaskActive =
    recData?.taskStatus === "pending" ||
    recData?.taskStatus === "running" ||
    (recTask != null &&
      (recTask.status === "running" || recTask.status === "pending"));

  // Sync isTaskActive back to taskRunning so refetchInterval stays in sync
  useEffect(() => {
    setTaskRunning(isTaskActive);
  }, [isTaskActive]);

  // Refresh mutation
  const refreshMutation = useMutation({
    mutationFn: recommendationsApi.refresh,
    onSuccess: (data) => {
      setTaskRunning(true);
      notifications.show({
        title: "Refreshing recommendations",
        message: data.message,
        color: "blue",
      });
      // Invalidate to pick up the new task status
      queryClient.invalidateQueries({ queryKey: ["recommendations"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to refresh recommendations",
        color: "red",
      });
    },
  });

  // Dismiss mutation
  const dismissMutation = useMutation({
    mutationFn: ({
      externalId,
      reason,
    }: {
      externalId: string;
      reason: string;
    }) => recommendationsApi.dismiss(externalId, reason),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["recommendations"] });
      notifications.show({
        title: "Dismissed",
        message: "Recommendation removed from your list.",
        color: "green",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to dismiss recommendation",
        color: "red",
      });
    },
  });

  // Loading state
  if (isLoading) {
    return (
      <Box py="xl" px="md">
        <Stack align="center" gap="md" py="xl">
          <Loader />
          <Text c="dimmed">Loading recommendations...</Text>
        </Stack>
      </Box>
    );
  }

  // No plugin enabled (404 from backend)
  if (error) {
    const isNoPlugin = error.error === "No recommendation plugin enabled";

    if (isNoPlugin) {
      return (
        <Box py="xl" px="md">
          <Stack gap="xl">
            <Title order={1}>Recommendations</Title>
            <Alert
              icon={<IconPlugConnected size={16} />}
              title="No recommendation plugin enabled"
              color="blue"
              variant="light"
            >
              Enable a recommendation plugin in{" "}
              <Text
                component="a"
                href="/settings/integrations"
                c="blue"
                td="underline"
                span
              >
                Settings &gt; Integrations
              </Text>{" "}
              to get personalized suggestions based on your library.
            </Alert>
          </Stack>
        </Box>
      );
    }

    return (
      <Box py="xl" px="md">
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Error loading recommendations"
          color="red"
        >
          {error.message || error.error || "An unexpected error occurred"}
        </Alert>
      </Box>
    );
  }

  const recommendations = recData?.recommendations ?? [];
  const isEmpty = recommendations.length === 0;

  return (
    <Box py="xl" px="md">
      <Stack gap="xl">
        {/* Header */}
        <Group justify="space-between" align="center">
          <Group gap="sm" align="center">
            <Title order={1}>Recommendations</Title>
            {recData?.cached && (
              <Text size="sm" c="dimmed">
                (cached)
              </Text>
            )}
          </Group>
          <Group gap="sm">
            {isTaskActive && (
              <Group gap={4}>
                <Loader size={14} />
                <Text size="sm" c="dimmed">
                  Generating...
                </Text>
              </Group>
            )}
            <Button
              leftSection={<IconRefresh size={16} />}
              variant="light"
              onClick={() => refreshMutation.mutate()}
              loading={refreshMutation.isPending}
              disabled={isTaskActive}
            >
              Refresh
            </Button>
          </Group>
        </Group>

        {/* Plugin info */}
        {recData && (
          <Text size="sm" c="dimmed">
            Powered by {recData.pluginName}
            {recData.generatedAt &&
              ` \u00B7 Generated ${new Date(recData.generatedAt).toLocaleDateString()}`}
          </Text>
        )}

        {/* Empty state */}
        {isEmpty && !isTaskActive && (
          <Alert
            icon={<IconSparkles size={16} />}
            title="No recommendations yet"
            color="blue"
            variant="light"
          >
            Your recommendation plugin hasn&apos;t generated any suggestions
            yet. Try clicking Refresh to generate recommendations based on your
            library.
          </Alert>
        )}
        {isEmpty && isTaskActive && (
          <Alert
            icon={<Loader size={16} />}
            title="Generating recommendations"
            color="blue"
            variant="light"
          >
            Your recommendations are being generated. This page will update
            automatically when they&apos;re ready.
          </Alert>
        )}

        {/* Recommendation cards */}
        <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="md">
          {recommendations.map((rec) => (
            <RecommendationCard
              key={rec.externalId}
              recommendation={rec}
              onDismiss={(id, reason) =>
                dismissMutation.mutate({ externalId: id, reason })
              }
              dismissing={
                dismissMutation.isPending &&
                dismissMutation.variables?.externalId === rec.externalId
              }
            />
          ))}
        </SimpleGrid>
      </Stack>
    </Box>
  );
}
