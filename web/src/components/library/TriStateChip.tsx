import { UnstyledButton } from "@mantine/core";
import { IconCheck, IconEye, IconProgress, IconX } from "@tabler/icons-react";
import type { TriState } from "@/types";
import classes from "./TriStateChip.module.css";

export type TriStateChipVariant =
  | "status"
  | "progress"
  | "metadata"
  | "neutral";

interface TriStateChipProps {
  /** The label to display */
  label: string;
  /** Current state of the chip */
  state: TriState;
  /** Callback when state changes */
  onChange: (state: TriState) => void;
  /** Optional count to display */
  count?: number;
  /** Whether the chip is disabled */
  disabled?: boolean;
  /**
   * Shape language for the chip. Variants describe shape only; behaviour is
   * identical across all four.
   * - `status` (Ongoing / Ended / Hiatus / ...): capsule, leading 6px dot.
   * - `progress` (Unread / In Progress / Read): capsule, leading 14px icon.
   * - `metadata` (Genres, Tags, default): square radius, no leading slot.
   * - `neutral` (Has Rating, Tracked, ...): square radius, no leading slot.
   * Defaults to `metadata`.
   */
  variant?: TriStateChipVariant;
  /**
   * Discriminator used by `status` and `progress` variants to pick the
   * leading decoration (dot color / icon). Typically the option `value`.
   * Ignored for `metadata` and `neutral`.
   */
  decorationKey?: string;
}

const STATUS_DOT_CLASS: Record<string, string> = {
  ongoing: classes.statusDotGreen,
  ended: classes.statusDotBlue,
  hiatus: classes.statusDotAmber,
  abandoned: classes.statusDotGrey,
  unknown: classes.statusDotMuted,
};

function renderProgressIcon(decorationKey?: string) {
  switch (decorationKey) {
    case "unread":
      return <IconEye size={14} aria-hidden />;
    case "in_progress":
      return <IconProgress size={14} aria-hidden />;
    case "read":
      return <IconCheck size={14} aria-hidden />;
    default:
      return null;
  }
}

/**
 * A tri-state chip component for filter selection.
 *
 * States cycle through: neutral → include → exclude → neutral
 *
 * Visual indicators:
 * - neutral: outlined, no leading icon (variant decoration shows for
 *   `status`/`progress`).
 * - include: filled blue, leading checkmark replaces any variant decoration.
 * - exclude: filled red, leading X replaces any variant decoration.
 *
 * Shape language is controlled via `variant` (see prop docs). For the
 * `status` and `progress` variants, pass `decorationKey` to select the
 * category dot color or progress icon.
 */
export function TriStateChip({
  label,
  state,
  onChange,
  count,
  disabled = false,
  variant = "metadata",
  decorationKey,
}: TriStateChipProps) {
  const handleClick = () => {
    if (disabled) return;

    // Cycle through states: neutral → include → exclude → neutral
    const transitions: Record<TriState, TriState> = {
      neutral: "include",
      include: "exclude",
      exclude: "neutral",
    };
    onChange(transitions[state]);
  };

  const isSelected = state === "include" || state === "exclude";
  const variantHasOwnSlot = variant === "status" || variant === "progress";
  const showLeadingSlot = isSelected || variantHasOwnSlot;

  const renderLeadingContent = () => {
    if (isSelected) {
      return state === "include" ? (
        <IconCheck size={12} aria-hidden />
      ) : (
        <IconX size={12} aria-hidden />
      );
    }
    if (variant === "status") {
      const dotColor =
        (decorationKey && STATUS_DOT_CLASS[decorationKey]) ??
        classes.statusDotMuted;
      return (
        <span className={`${classes.statusDot} ${dotColor}`} aria-hidden />
      );
    }
    if (variant === "progress") {
      return renderProgressIcon(decorationKey);
    }
    return null;
  };

  return (
    <UnstyledButton
      onClick={handleClick}
      disabled={disabled}
      className={classes.button}
      data-disabled={disabled || undefined}
      data-variant={variant}
      data-state={state}
    >
      <span
        className={classes.badge}
        data-state={state}
        data-variant={variant}
        data-disabled={disabled || undefined}
        data-has-leading={showLeadingSlot || undefined}
      >
        {showLeadingSlot && (
          <span
            className={classes.leading}
            data-selected={isSelected || undefined}
            // React's key forces a re-mount when the slot switches between
            // its decoration and the selection icon, replaying the 120ms
            // pop animation (collapsed to instant under reduced motion by
            // the global guard in index.css).
            key={isSelected ? `sel-${state}` : `dec-${variant}`}
          >
            {renderLeadingContent()}
          </span>
        )}
        <span className={classes.label}>{label}</span>
        {count !== undefined && (
          <span className={classes.count} data-testid="tri-state-chip-count">
            {count}
          </span>
        )}
      </span>
    </UnstyledButton>
  );
}
