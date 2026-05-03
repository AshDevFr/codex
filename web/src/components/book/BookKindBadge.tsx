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
 * Two semantic colors:
 * - blue  = volume (a complete volume — chapter null)
 * - grape = chapter (a chapter, with or without a parent volume)
 *
 * Cases:
 * - volume set, chapter null  → blue  "Vol N"
 * - chapter set, volume null  → grape "Ch N"
 * - both set                  → grape "Vol V · Ch C" (still a chapter)
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

  // Chapter (with or without parent volume) — grape
  if (hasChapter) {
    return (
      <Badge variant={variant} color={"grape" as MantineColor} size={size}>
        {hasVolume
          ? `Vol ${volume} · Ch ${formatChapter(chapter)}`
          : `Ch ${formatChapter(chapter)}`}
      </Badge>
    );
  }

  // Volume only — blue
  if (hasVolume) {
    return (
      <Badge variant={variant} color={"blue" as MantineColor} size={size}>
        Vol {volume}
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
