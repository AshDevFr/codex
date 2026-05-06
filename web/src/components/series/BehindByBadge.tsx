import { Badge, Tooltip } from "@mantine/core";
import { useNavigate } from "react-router-dom";

export type BehindByVariant = "translation" | "upstream";
export type BehindByAxis = "chapter" | "volume";

export interface BehindByBadgeProps {
  /** Translation = warm/orange/clickable (Releases tab); upstream = grey informational. */
  variant: BehindByVariant;
  axis: BehindByAxis;
  /** Positive integer; the badge is hidden when <= 0. */
  delta: number;
  /** Required to navigate when the translation badge is clicked. */
  seriesId: string;
  /** Provider/source name shown in the tooltip ("MangaBaka", "MangaUpdates", ...). */
  provider?: string | null;
  /** Language list for translation badges; rendered as e.g. "en". */
  language?: string;
}

/**
 * Compact "+N ch" / "+N vol" badge near the series header. Two variants:
 *
 * - `translation` (orange, actionable): `latestKnownChapter > localMaxChapter`.
 *   Click navigates to the Releases tab. Phase 6 (MangaUpdates) is the writer.
 * - `upstream` (grey, informational): `upstreamChapterGap > 0`. Phase 5 metadata
 *   gap signal — not actionable, no Releases tab to send the user to.
 */
export function BehindByBadge({
  variant,
  axis,
  delta,
  seriesId,
  provider,
  language,
}: BehindByBadgeProps) {
  const navigate = useNavigate();

  if (!Number.isFinite(delta) || delta <= 0) {
    return null;
  }

  const unit = axis === "chapter" ? "ch" : "vol";
  const label =
    variant === "translation"
      ? `+${delta} ${unit} (translation)`
      : `+${delta} ${unit} (upstream)`;

  const tooltip =
    variant === "translation"
      ? `${provider ?? "A release source"} announced ${delta} more ${
          axis === "chapter" ? "chapters" : "volumes"
        }${language ? ` in ${language}` : ""} — open Releases.`
      : `${provider ?? "An external metadata provider"} reports ${delta} more ${
          axis === "chapter" ? "chapters" : "volumes"
        } in the original language.`;

  const color = variant === "translation" ? "orange" : "gray";

  const badge = (
    <Badge
      color={color}
      variant="light"
      size="sm"
      style={{
        cursor: variant === "translation" ? "pointer" : "default",
        textTransform: "none",
      }}
      onClick={
        variant === "translation"
          ? () => navigate(`/series/${seriesId}#releases`)
          : undefined
      }
      data-testid={`behind-by-${variant}-${axis}`}
    >
      {label}
    </Badge>
  );

  return <Tooltip label={tooltip}>{badge}</Tooltip>;
}
