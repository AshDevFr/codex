import { Group, Text, Tooltip } from "@mantine/core";
import { IconUsers } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { ratingsApi, storageToDisplayRating } from "@/api/ratings";

interface CommunityRatingProps {
  seriesId: string;
}

export function CommunityRating({ seriesId }: CommunityRatingProps) {
  const { data, isLoading } = useQuery({
    queryKey: ["series-average-rating", seriesId],
    queryFn: () => ratingsApi.getSeriesAverageRating(seriesId),
  });

  if (isLoading) {
    return null;
  }

  // Don't show if no community ratings
  if (!data?.average || data.count === 0) {
    return null;
  }

  const displayRating = storageToDisplayRating(data.average);
  const tooltipLabel = `${displayRating.toFixed(1)} average from ${data.count} ${data.count === 1 ? "user" : "users"}`;

  return (
    <Tooltip label={tooltipLabel} position="top">
      <Group gap={4}>
        <IconUsers size={14} style={{ color: "var(--mantine-color-blue-5)" }} />
        <Text size="sm" fw={500}>
          Community: {displayRating.toFixed(1)}
        </Text>
        <Text size="xs" c="dimmed">
          ({data.count})
        </Text>
      </Group>
    </Tooltip>
  );
}
