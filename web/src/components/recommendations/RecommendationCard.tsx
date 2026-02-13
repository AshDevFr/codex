import {
  Badge,
  Box,
  Button,
  Card,
  Group,
  Image,
  Stack,
  Text,
} from "@mantine/core";
import {
  IconBook,
  IconCheck,
  IconExternalLink,
  IconLibrary,
  IconStar,
  IconTrendingUp,
  IconX,
} from "@tabler/icons-react";
import { Link } from "react-router-dom";
import type { RecommendationDto } from "@/api/recommendations";

// =============================================================================
// Helpers
// =============================================================================

/** Format a score (0.0-1.0) as a percentage */
function formatScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

/** Capitalize a status string (e.g., "ongoing" → "Ongoing") */
function formatStatus(status: string): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

/** Get badge color for a series status */
function statusColor(status: string): string {
  switch (status) {
    case "ended":
      return "green";
    case "hiatus":
      return "yellow";
    case "abandoned":
      return "red";
    default:
      return "blue";
  }
}

// =============================================================================
// RecommendationCard
// =============================================================================

interface RecommendationCardProps {
  recommendation: RecommendationDto;
  onDismiss: (externalId: string) => void;
  dismissing?: boolean;
}

export function RecommendationCard({
  recommendation,
  onDismiss,
  dismissing,
}: RecommendationCardProps) {
  const {
    externalId,
    externalUrl,
    title,
    coverUrl,
    summary,
    genres = [],
    score,
    reason,
    inLibrary,
    codexSeriesId,
    inCodex,
    status,
    totalBookCount,
    rating,
    popularity,
  } = recommendation;

  return (
    <Card withBorder padding="lg" data-testid="recommendation-card">
      <Group align="flex-start" gap="lg" wrap="nowrap">
        {/* Cover image */}
        <Box w={100} miw={100} style={{ flexShrink: 0 }}>
          {coverUrl ? (
            <Image
              src={coverUrl}
              alt={title}
              w={100}
              h={140}
              fit="cover"
              radius="sm"
              fallbackSrc="data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 140'%3E%3Crect width='100' height='140' fill='%23e0e0e0'/%3E%3Ctext x='50' y='70' text-anchor='middle' dy='.3em' fill='%23999' font-size='12'%3ENo Cover%3C/text%3E%3C/svg%3E"
            />
          ) : (
            <Box
              w={100}
              h={140}
              bg="gray.2"
              style={{
                borderRadius: "var(--mantine-radius-sm)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Text size="xs" c="dimmed">
                No Cover
              </Text>
            </Box>
          )}
        </Box>

        {/* Content */}
        <Stack gap="xs" style={{ flex: 1 }}>
          <Group justify="space-between" align="flex-start">
            <Box>
              <Group gap="xs" align="center">
                <Text fw={600} size="lg" lineClamp={1}>
                  {title}
                </Text>
                {externalUrl && (
                  <a
                    href={externalUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <IconExternalLink size={16} color="gray" />
                  </a>
                )}
              </Group>
              <Group gap="xs" mt={2}>
                <Text size="sm" fw={500} c="yellow.7">
                  {formatScore(score)} match
                </Text>
                {status && status !== "unknown" && (
                  <Badge size="xs" variant="filled" color={statusColor(status)}>
                    {formatStatus(status)}
                  </Badge>
                )}
                {totalBookCount != null && (
                  <Badge
                    size="xs"
                    variant="light"
                    color="gray"
                    leftSection={<IconBook size={10} />}
                  >
                    {totalBookCount} vol
                  </Badge>
                )}
                {rating != null && (
                  <Badge
                    size="xs"
                    variant="light"
                    color="yellow"
                    leftSection={<IconStar size={10} />}
                  >
                    {rating}%
                  </Badge>
                )}
                {popularity != null && (
                  <Badge
                    size="xs"
                    variant="light"
                    color="grape"
                    leftSection={<IconTrendingUp size={10} />}
                  >
                    {popularity.toLocaleString()}
                  </Badge>
                )}
              </Group>
            </Box>
            <Group gap={4}>
              {inCodex && (
                <Badge
                  color="green"
                  variant="light"
                  leftSection={<IconCheck size={12} />}
                >
                  Available
                </Badge>
              )}
              {inLibrary && (
                <Badge
                  color="blue"
                  variant="light"
                  leftSection={<IconCheck size={12} />}
                >
                  In Anilist Library
                </Badge>
              )}
            </Group>
          </Group>

          {/* Reason */}
          <Text size="sm" c="dimmed" lineClamp={2}>
            {reason}
          </Text>

          {/* Summary */}
          {summary && (
            <Text size="sm" lineClamp={2}>
              {summary}
            </Text>
          )}

          {/* Genres */}
          {genres.length > 0 && (
            <Group gap={4}>
              {genres.slice(0, 5).map((genre) => (
                <Badge key={genre} size="xs" variant="outline" color="gray">
                  {genre}
                </Badge>
              ))}
              {genres.length > 5 && (
                <Text size="xs" c="dimmed">
                  +{genres.length - 5} more
                </Text>
              )}
            </Group>
          )}

          {/* Actions */}
          <Group gap="xs" mt="xs">
            {codexSeriesId && (
              <Button
                size="xs"
                variant="light"
                color="blue"
                leftSection={<IconLibrary size={14} />}
                component={Link}
                to={`/series/${codexSeriesId}`}
              >
                View in Library
              </Button>
            )}
            {!inCodex && (
              <Button
                size="xs"
                variant="subtle"
                color="gray"
                leftSection={<IconX size={14} />}
                onClick={() => onDismiss(externalId)}
                loading={dismissing}
              >
                Not Interested
              </Button>
            )}
          </Group>
        </Stack>
      </Group>
    </Card>
  );
}
