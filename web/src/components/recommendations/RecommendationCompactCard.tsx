import { Badge, Box, Image, Stack, Text, Tooltip } from "@mantine/core";
import { IconCheck } from "@tabler/icons-react";
import type { RecommendationDto } from "@/api/recommendations";

interface RecommendationCompactCardProps {
  recommendation: RecommendationDto;
}

export function RecommendationCompactCard({
  recommendation,
}: RecommendationCompactCardProps) {
  const { title, coverUrl, score, reason, externalUrl, inLibrary } =
    recommendation;

  const scorePercent = `${Math.round(score * 100)}%`;

  const card = (
    <Box
      component={externalUrl ? "a" : "div"}
      href={externalUrl || undefined}
      target={externalUrl ? "_blank" : undefined}
      rel={externalUrl ? "noopener noreferrer" : undefined}
      style={{
        textDecoration: "none",
        color: "inherit",
        display: "block",
        cursor: externalUrl ? "pointer" : "default",
      }}
      data-testid="recommendation-compact-card"
    >
      <Stack gap={4}>
        {/* Cover */}
        <Box
          style={{
            aspectRatio: "150 / 212.125",
            borderRadius: "var(--mantine-radius-sm)",
            overflow: "hidden",
            position: "relative",
          }}
        >
          {coverUrl ? (
            <Image
              src={coverUrl}
              alt={title}
              w="100%"
              h="100%"
              fit="cover"
              fallbackSrc="data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 150 212'%3E%3Crect width='150' height='212' fill='%23e0e0e0'/%3E%3Ctext x='75' y='106' text-anchor='middle' dy='.3em' fill='%23999' font-size='14'%3ENo Cover%3C/text%3E%3C/svg%3E"
            />
          ) : (
            <Box
              w="100%"
              h="100%"
              bg="gray.2"
              style={{
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

          {/* Score badge */}
          <Badge
            size="xs"
            color="yellow"
            variant="filled"
            style={{
              position: "absolute",
              bottom: 4,
              right: 4,
            }}
          >
            {scorePercent}
          </Badge>

          {/* In library indicator */}
          {inLibrary && (
            <Badge
              size="xs"
              color="green"
              variant="filled"
              leftSection={<IconCheck size={10} />}
              style={{
                position: "absolute",
                top: 4,
                right: 4,
              }}
            >
              Owned
            </Badge>
          )}
        </Box>

        {/* Content area */}
        <Box h="3.5rem">
          <Text size="sm" fw={600} lineClamp={1}>
            {title}
          </Text>
          <Text size="xs" c="dimmed" lineClamp={1}>
            {reason}
          </Text>
        </Box>
      </Stack>
    </Box>
  );

  return (
    <Tooltip label={reason} multiline maw={250} withArrow openDelay={400}>
      {card}
    </Tooltip>
  );
}
