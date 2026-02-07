import {
  Alert,
  Box,
  Button,
  Group,
  Loader,
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
import { recommendationsApi } from "@/api/recommendations";
import { RecommendationCard } from "@/components/recommendations/RecommendationCard";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import type { ApiError } from "@/types";

export function Recommendations() {
  useDocumentTitle("Recommendations");

  const queryClient = useQueryClient();

  // Fetch recommendations
  const {
    data: recData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["recommendations"],
    queryFn: recommendationsApi.get,
    retry: false,
  });

  // Refresh mutation
  const refreshMutation = useMutation({
    mutationFn: recommendationsApi.refresh,
    onSuccess: (data) => {
      notifications.show({
        title: "Refreshing recommendations",
        message: data.message,
        color: "blue",
      });
      // Invalidate after a short delay to allow the task to start
      setTimeout(() => {
        queryClient.invalidateQueries({ queryKey: ["recommendations"] });
      }, 2000);
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
    mutationFn: (externalId: string) =>
      recommendationsApi.dismiss(externalId, "not_interested"),
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
    const apiError = error as ApiError;
    const isNoPlugin =
      apiError.error === "No recommendation plugin enabled";

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
              <Text component="a" href="/settings/integrations" c="blue" td="underline" span>
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
          {apiError.message || apiError.error || "An unexpected error occurred"}
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
          <Button
            leftSection={<IconRefresh size={16} />}
            variant="light"
            onClick={() => refreshMutation.mutate()}
            loading={refreshMutation.isPending}
          >
            Refresh
          </Button>
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
        {isEmpty && (
          <Alert
            icon={<IconSparkles size={16} />}
            title="No recommendations yet"
            color="blue"
            variant="light"
          >
            Your recommendation plugin hasn&apos;t generated any suggestions yet.
            Try clicking Refresh to generate recommendations based on your library.
          </Alert>
        )}

        {/* Recommendation cards */}
        {recommendations.map((rec) => (
          <RecommendationCard
            key={rec.externalId}
            recommendation={rec}
            onDismiss={(id) => dismissMutation.mutate(id)}
            dismissing={
              dismissMutation.isPending &&
              dismissMutation.variables === rec.externalId
            }
          />
        ))}
      </Stack>
    </Box>
  );
}
