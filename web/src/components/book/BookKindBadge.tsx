import { Badge, type MantineColor, Tooltip } from "@mantine/core";

interface BookKindBadgeProps {
  volume: number | null | undefined;
  chapter: number | null | undefined;
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  variant?: "filled" | "light" | "outline" | "dot" | "gradient";
}

function formatChapter(chapter: number): string {
  // Drop trailing .0 for whole numbers (42.0 → "42") but keep fractions (42.5 → "42.5")
  return Number.isInteger(chapter) ? chapter.toString() : chapter.toString();
}

/**
 * Badge component classifying a book by its (volume, chapter) populated fields.
 *
 * Four cases:
 * - volume set, chapter null  → "Vol N"
 * - chapter set, volume null  → "Ch N"
 * - both set                  → "Vol V · Ch C"
 * - neither set               → muted "Vol" with hover tooltip explaining the gap
 */
export function BookKindBadge({
  volume,
  chapter,
  size = "sm",
  variant = "light",
}: BookKindBadgeProps) {
  const hasVolume = volume !== null && volume !== undefined;
  const hasChapter = chapter !== null && chapter !== undefined;

  // Both set: combined badge
  if (hasVolume && hasChapter) {
    return (
      <Badge variant={variant} color={"blue" as MantineColor} size={size}>
        Vol {volume} · Ch {formatChapter(chapter)}
      </Badge>
    );
  }

  // Volume only
  if (hasVolume) {
    return (
      <Badge variant={variant} color={"blue" as MantineColor} size={size}>
        Vol {volume}
      </Badge>
    );
  }

  // Chapter only
  if (hasChapter) {
    return (
      <Badge variant={variant} color={"grape" as MantineColor} size={size}>
        Ch {formatChapter(chapter)}
      </Badge>
    );
  }

  // Neither: muted default-to-volume with explanatory tooltip
  return (
    <Tooltip
      label="Volume number not set. Edit metadata to assign one."
      withArrow
    >
      <Badge variant="outline" color={"gray" as MantineColor} size={size}>
        Vol
      </Badge>
    </Tooltip>
  );
}
