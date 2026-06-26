import {
  Anchor,
  Box,
  Button,
  Group,
  Image,
  Loader,
  Stack,
  Text,
} from "@mantine/core";
import { IconArrowLeft, IconArrowRight } from "@tabler/icons-react";
import type { AdjacentBook } from "@/store/readerStore";
import { useAutoAdvanceCountdown } from "./hooks/useAutoAdvanceCountdown";
import { bookCoverUrl } from "./utils/coverUrl";

interface ChapterTransitionPanelProps {
  /** Whether this panel advances to the next book or returns to the previous one. */
  direction: "next" | "prev";
  /** The adjacent book, or null at a series edge (no further book that way). */
  book: AdjacentBook | null;
  /** Navigate to the adjacent book. */
  onContinue: () => void;
  /** Whether auto-advance is enabled (only honored for `direction === "next"`). */
  autoAdvance?: boolean;
  /** Called when the user cancels the auto-advance countdown. */
  onCancelAutoAdvance?: () => void;
  /** Countdown length override (seconds); primarily for tests. */
  countdownSeconds?: number;
  /** Reading direction; only "rtl" mirrors the continue arrow. */
  readingDirection?: "ltr" | "rtl";
}

interface TransitionLabels {
  series: string;
  primary: string;
  secondary: string | null;
}

/**
 * Build the display labels for the transition card, gracefully degrading when
 * series/volume/chapter metadata is missing:
 * - `primary`: "Ch. {number}" when a book number exists, otherwise the title.
 * - `secondary`: "S{vol} Chapter {chap}" when volume/chapter metadata exists;
 *   falls back to the title when only the number drove the primary line.
 */
export function formatTransitionLabels(book: AdjacentBook): TransitionLabels {
  const primary = book.number != null ? `Ch. ${book.number}` : book.title;

  const parts: string[] = [];
  if (book.volume != null) {
    parts.push(`S${String(book.volume).padStart(2, "0")}`);
  }
  if (book.chapter != null) {
    parts.push(`Chapter ${String(book.chapter).padStart(3, "0")}`);
  }

  let secondary: string | null = null;
  if (parts.length > 0) {
    secondary = parts.join(" ");
  } else if (book.number != null && book.title !== primary) {
    secondary = book.title;
  }

  return { series: book.seriesName, primary, secondary };
}

/**
 * Full-screen "Next Chapter" / "Previous Chapter" transition panel shown at the
 * boundaries of the reader (Komic-style). Displays the adjacent book's cover and
 * labels with a "Continue Reading" button. When advancing to the next book and
 * auto-advance is enabled, it runs a countdown that navigates automatically,
 * with a Cancel link. At a series edge (`book === null`) it shows an end/start
 * message with no button.
 */
export function ChapterTransitionPanel({
  direction,
  book,
  onContinue,
  autoAdvance = false,
  onCancelAutoAdvance,
  countdownSeconds,
  readingDirection = "ltr",
}: ChapterTransitionPanelProps) {
  const isNext = direction === "next";
  const heading = isNext ? "Next Chapter" : "Previous Chapter";

  // The arrow points in the direction of travel. RTL mirrors it: "next" goes
  // left, "prev" goes right.
  const isRtl = readingDirection === "rtl";
  const pointsRight = isNext !== isRtl;

  const countdownActive = isNext && autoAdvance && book != null;
  const { remaining, cancel, cancelled } = useAutoAdvanceCountdown({
    active: countdownActive,
    seconds: countdownSeconds,
    onElapsed: onContinue,
  });
  const showCountdown = countdownActive && !cancelled;

  const handleCancel = () => {
    cancel();
    onCancelAutoAdvance?.();
  };

  return (
    <Box
      data-testid={`chapter-transition-${direction}`}
      style={{
        width: "100%",
        height: "100dvh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        backgroundColor: "#000",
        padding: "32px 16px",
      }}
    >
      <Stack align="center" gap="lg" style={{ maxWidth: 360, width: "100%" }}>
        <Text size="xl" fw={700} c="white" ta="center">
          {book ? heading : isNext ? "End of series" : "Beginning of series"}
        </Text>

        {book ? (
          <TransitionCard book={book} />
        ) : (
          <Text c="dimmed" ta="center">
            {isNext
              ? "You've reached the last book."
              : "You're at the first book."}
          </Text>
        )}

        {book && (
          <Button
            size="md"
            radius="xl"
            variant="white"
            color="dark"
            onClick={onContinue}
            rightSection={
              pointsRight ? (
                <IconArrowRight size={18} />
              ) : (
                <IconArrowLeft size={18} />
              )
            }
          >
            Continue Reading
          </Button>
        )}

        {/* Reserve the countdown's vertical space for every "next" panel so the
            cover and button never shift when the countdown appears, disappears,
            or is cancelled. Only its contents toggle. */}
        {isNext && book && (
          <Box
            data-testid="auto-advance-slot"
            mih={48}
            w="100%"
            style={{ display: "flex", justifyContent: "center" }}
          >
            {showCountdown && (
              <Stack align="center" gap={4}>
                <Group gap={8} justify="center">
                  <Loader size="xs" color="gray" />
                  <Text size="sm" c="dimmed" ta="center">
                    Continuing to next chapter in {remaining}
                    {remaining === 1 ? " second" : " seconds"}
                  </Text>
                </Group>
                <Anchor
                  component="button"
                  type="button"
                  size="sm"
                  c="dimmed"
                  onClick={handleCancel}
                >
                  Cancel
                </Anchor>
              </Stack>
            )}
          </Box>
        )}
      </Stack>
    </Box>
  );
}

function TransitionCard({ book }: { book: AdjacentBook }) {
  const { series, primary, secondary } = formatTransitionLabels(book);

  return (
    <Stack align="center" gap="sm">
      <Box
        style={{
          width: 180,
          aspectRatio: "2 / 3",
          borderRadius: 12,
          overflow: "hidden",
          backgroundColor: "#1a1a1a",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        {/* Plain <img> like the rest of the app (MediaCard, BookDetail): the
            thumbnail endpoint is fetched directly, not through the API client
            whose baseURL would double the /api/v1 prefix. */}
        <Image
          src={bookCoverUrl(book.id)}
          alt={`Cover of ${book.title}`}
          w="100%"
          h="100%"
          fit="cover"
        />
      </Box>

      <Stack align="center" gap={2}>
        <Text size="sm" c="dimmed" ta="center">
          {series}
        </Text>
        <Text size="lg" fw={700} c="white" ta="center">
          {primary}
        </Text>
        {secondary && (
          <Text size="sm" c="dimmed" ta="center">
            {secondary}
          </Text>
        )}
      </Stack>
    </Stack>
  );
}
