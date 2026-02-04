import { Badge, Group, Stack, Text, Tooltip } from "@mantine/core";
import { IconAward, IconMedal, IconTrophy } from "@tabler/icons-react";
import type { BookAward } from "@/types/book-metadata";
import { parseAwardsJson } from "@/types/book-metadata";

interface AwardsListProps {
  /** Awards as JSON string or pre-parsed array */
  awards: string | BookAward[] | null | undefined;
  /** Maximum number of awards to display before collapsing */
  maxDisplay?: number;
  /** Size of badges */
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  /** Layout direction */
  direction?: "horizontal" | "vertical";
  /** Whether to show only won awards (hide nominations) */
  wonOnly?: boolean;
}

/**
 * Get icon based on award status
 */
function getAwardIcon(won: boolean) {
  return won ? IconTrophy : IconMedal;
}

/**
 * Component to display a list of awards and nominations.
 * Won awards are displayed prominently with a trophy icon.
 * Nominations are displayed with a medal icon.
 */
export function AwardsList({
  awards,
  maxDisplay,
  size = "sm",
  direction = "horizontal",
  wonOnly = false,
}: AwardsListProps) {
  // Parse awards if string
  let awardList: BookAward[] =
    typeof awards === "string" ? parseAwardsJson(awards) : (awards ?? []);

  // Filter to won only if requested
  if (wonOnly) {
    awardList = awardList.filter((a) => a.won);
  }

  if (awardList.length === 0) return null;

  // Sort: won awards first, then by year descending
  const sortedAwards = [...awardList].sort((a, b) => {
    if (a.won !== b.won) return a.won ? -1 : 1;
    if (a.year && b.year) return b.year - a.year;
    if (a.year) return -1;
    if (b.year) return 1;
    return 0;
  });

  // Apply maxDisplay limit
  const displayAwards = maxDisplay
    ? sortedAwards.slice(0, maxDisplay)
    : sortedAwards;
  const hiddenCount = sortedAwards.length - displayAwards.length;

  const Container = direction === "vertical" ? Stack : Group;
  const containerProps =
    direction === "vertical"
      ? { gap: "xs" as const }
      : { gap: "xs" as const, wrap: "wrap" as const };

  return (
    <Container {...containerProps}>
      {displayAwards.map((award, index) => {
        const Icon = getAwardIcon(award.won);
        const color = award.won ? "yellow" : "gray";
        const tooltipParts = [
          award.category,
          award.year?.toString(),
          award.won ? "Won" : "Nominated",
        ].filter(Boolean);

        const label = award.year ? `${award.name} (${award.year})` : award.name;

        return (
          <Tooltip
            key={`${award.name}-${award.year ?? index}`}
            label={tooltipParts.join(" - ")}
            disabled={!award.category && !award.year}
          >
            <Badge
              variant={award.won ? "light" : "outline"}
              color={color}
              size={size}
              leftSection={<Icon size={12} />}
            >
              {label}
            </Badge>
          </Tooltip>
        );
      })}
      {hiddenCount > 0 && (
        <Text size="xs" c="dimmed">
          +{hiddenCount} more
        </Text>
      )}
    </Container>
  );
}

/**
 * Compact display showing award count with icon.
 * Useful for card views where space is limited.
 */
export function AwardsCount({ awards }: Pick<AwardsListProps, "awards">) {
  const awardList: BookAward[] =
    typeof awards === "string" ? parseAwardsJson(awards) : (awards ?? []);

  if (awardList.length === 0) return null;

  const wonCount = awardList.filter((a) => a.won).length;
  const nominatedCount = awardList.length - wonCount;

  const tooltipContent = [
    wonCount > 0 ? `${wonCount} won` : null,
    nominatedCount > 0 ? `${nominatedCount} nominated` : null,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <Tooltip label={tooltipContent}>
      <Group gap={4}>
        <IconAward size={14} />
        <Text size="xs">{awardList.length}</Text>
      </Group>
    </Tooltip>
  );
}
