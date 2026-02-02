import { Button, Group, Tooltip } from "@mantine/core";
import styles from "./AlphabetFilter.module.css";

const LETTERS = [
  "#",
  "A",
  "B",
  "C",
  "D",
  "E",
  "F",
  "G",
  "H",
  "I",
  "J",
  "K",
  "L",
  "M",
  "N",
  "O",
  "P",
  "Q",
  "R",
  "S",
  "T",
  "U",
  "V",
  "W",
  "X",
  "Y",
  "Z",
] as const;

export type AlphabetLetter = (typeof LETTERS)[number] | "ALL";

/** Map of first character to count */
export type AlphabetCounts = Map<string, number>;

interface AlphabetFilterProps {
  /** Currently selected letter (null means "ALL") */
  selected: AlphabetLetter | null;
  /** Callback when a letter is selected */
  onSelect: (letter: AlphabetLetter | null) => void;
  /** Optional counts per letter from alphabetical-groups endpoint */
  counts?: AlphabetCounts;
  /** Total count (for ALL button) */
  totalCount?: number;
}

/**
 * Alphabetical A-Z filter bar for filtering series by the first letter of their sort title.
 *
 * - "ALL" shows all series (no filter)
 * - "#" shows series starting with a number or special character
 * - A-Z shows series starting with that letter (case-insensitive)
 */
export function AlphabetFilter({
  selected,
  onSelect,
  counts,
  totalCount,
}: AlphabetFilterProps) {
  const handleClick = (letter: AlphabetLetter) => {
    if (letter === "ALL") {
      onSelect(null);
    } else {
      // If clicking the already selected letter, deselect (go back to ALL)
      onSelect(selected === letter ? null : letter);
    }
  };

  // Get count for a letter
  const getCount = (letter: AlphabetLetter): number | undefined => {
    if (!counts) return undefined;

    if (letter === "ALL") {
      return totalCount;
    }

    if (letter === "#") {
      // Sum all numeric and special character counts
      let sum = 0;
      for (const [key, value] of counts) {
        // If key is not a-z, it's a number or special char
        if (!/^[a-z]$/.test(key)) {
          sum += value;
        }
      }
      return sum > 0 ? sum : undefined;
    }

    // Regular letter (case-insensitive)
    return counts.get(letter.toLowerCase());
  };

  const allCount = getCount("ALL");
  const hasFilter = selected !== null;

  return (
    <Group gap={4} wrap="nowrap" justify="center" className={styles.container}>
      {/* ALL button - highlighted when no filter active */}
      <Tooltip
        label={allCount ? `${allCount} series` : "All series"}
        position="bottom"
        withArrow
        openDelay={500}
      >
        <Button
          size="compact-xs"
          variant={!hasFilter ? "filled" : "subtle"}
          color={!hasFilter ? "orange" : "gray"}
          className={styles.letterButton}
          onClick={() => handleClick("ALL")}
          data-selected={!hasFilter || undefined}
        >
          ALL
        </Button>
      </Tooltip>

      {/* Letter buttons */}
      {LETTERS.map((letter) => {
        const isSelected = selected === letter;
        const count = getCount(letter);
        const hasCount = count !== undefined && count > 0;
        const isEmpty = counts !== undefined && !hasCount;

        const button = (
          <Button
            key={letter}
            size="compact-xs"
            variant={isSelected ? "filled" : "subtle"}
            color={isSelected ? "orange" : "gray"}
            className={styles.letterButton}
            onClick={() => handleClick(letter)}
            data-selected={isSelected || undefined}
            data-empty={isEmpty || undefined}
            disabled={isEmpty}
          >
            {letter}
          </Button>
        );

        // Wrap in tooltip if has count
        if (hasCount && count > 0) {
          return (
            <Tooltip
              key={letter}
              label={`${count} series`}
              position="bottom"
              withArrow
              openDelay={500}
            >
              {button}
            </Tooltip>
          );
        }

        return button;
      })}
    </Group>
  );
}
