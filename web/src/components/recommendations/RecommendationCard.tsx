import { Badge, Box, Button, Card, Group, Image, Text } from "@mantine/core";
import {
  IconBook,
  IconCheck,
  IconExternalLink,
  IconLibrary,
  IconStar,
  IconTrendingUp,
  IconX,
} from "@tabler/icons-react";
import { truncate } from "es-toolkit/compat";
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

/** Get a top-border color based on library status */
function topBorderColor(
  inCodex?: boolean,
  inLibrary?: boolean,
): string | undefined {
  if (inCodex) return "var(--mantine-color-green-6)";
  if (inLibrary) return "var(--mantine-color-blue-6)";
  return undefined;
}

// =============================================================================
// RecommendationCard
// =============================================================================

interface RecommendationCardProps {
  recommendation: RecommendationDto;
  onDismiss: (externalId: string, reason: string) => void;
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

  const borderColor = topBorderColor(inCodex, inLibrary);

  return (
    <Card
      withBorder
      padding="md"
      data-testid="recommendation-card"
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        borderTopWidth: 3,
        borderTopColor: borderColor ?? "transparent",
      }}
    >
      {/* Main content area — cover floats left, text wraps around it */}
      <Box style={{ flex: 1 }}>
        {/* Floated cover */}
        <Box
          style={{
            float: "left",
            marginRight: "var(--mantine-spacing-md)",
            marginBottom: "var(--mantine-spacing-xs)",
          }}
        >
          {coverUrl ? (
            <Image
              src={coverUrl}
              alt={title}
              w={120}
              h={168}
              fit="cover"
              radius="sm"
              fallbackSrc="data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 168'%3E%3Crect width='120' height='168' fill='%23e0e0e0'/%3E%3Ctext x='60' y='84' text-anchor='middle' dy='.3em' fill='%23999' font-size='12'%3ENo Cover%3C/text%3E%3C/svg%3E"
            />
          ) : (
            <Box
              w={120}
              h={168}
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

        {/* Title + external link */}
        <Group gap="xs" align="center" wrap="nowrap" mb={8}>
          <Text fw={600} size="md" lineClamp={1} style={{ flex: 1 }}>
            {title}
          </Text>
          {externalUrl && (
            <a href={externalUrl} target="_blank" rel="noopener noreferrer">
              <IconExternalLink size={14} color="gray" />
            </a>
          )}
        </Group>

        {/* Match score */}
        <Text size="sm" fw={500} c="yellow.7" mb={8}>
          {formatScore(score)} match
        </Text>

        {/* Meta badges */}
        <Group gap={4} wrap="wrap" mb={12}>
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
          {inCodex && (
            <Badge
              size="xs"
              color="green"
              variant="light"
              leftSection={<IconCheck size={10} />}
            >
              Available
            </Badge>
          )}
          {inLibrary && (
            <Badge
              size="xs"
              color="blue"
              variant="light"
              leftSection={<IconCheck size={10} />}
            >
              In Anilist Library
            </Badge>
          )}
        </Group>

        {/* Reason */}
        <Text size="sm" c="dimmed" mb={8} lineClamp={2}>
          {reason}
        </Text>

        {/* Summary — truncated in JS so it wraps naturally around the
           floated cover. CSS lineClamp (-webkit-box) and overflow:hidden
           both prevent text from wrapping around floats. */}
        {summary && (
          <Text size="sm" mb={8}>
            {truncate(summary, { length: 280 })}
          </Text>
        )}
      </Box>

      {/* Bottom section: genres + actions, pinned to bottom */}
      <Box style={{ clear: "both" }}>
        {/* Genres */}
        {genres.length > 0 && (
          <Group gap={6} mt="sm">
            {genres.slice(0, 4).map((genre) => (
              <Badge key={genre} size="xs" variant="outline" color="gray">
                {genre}
              </Badge>
            ))}
            {genres.length > 4 && (
              <Text size="xs" c="dimmed">
                +{genres.length - 4} more
              </Text>
            )}
          </Group>
        )}

        {/* Actions */}
        <Group gap="xs" mt="sm">
          {codexSeriesId && (
            <Button
              size="compact-xs"
              variant="light"
              color="blue"
              leftSection={<IconLibrary size={14} />}
              component={Link}
              to={`/series/${codexSeriesId}`}
            >
              View in Library
            </Button>
          )}
          <Button
            size="compact-xs"
            variant="subtle"
            color="gray"
            leftSection={<IconX size={14} />}
            onClick={() =>
              onDismiss(
                externalId,
                inCodex ? "already_owned" : "not_interested",
              )
            }
            loading={dismissing}
          >
            Not Interested
          </Button>
        </Group>
      </Box>
    </Card>
  );
}
