import { Box, Stack, Text, Transition } from "@mantine/core";
import {
  IconChevronLeft,
  IconChevronRight,
  IconCircleCheck,
} from "@tabler/icons-react";

interface BoundaryNotificationProps {
  /** The message to display */
  message: string | null;
  /** Whether the notification is visible */
  visible: boolean;
  /** The boundary type - affects the icon shown */
  type: "at-start" | "at-end" | "none";
  /** Reading direction - affects which side the chevron appears on */
  readingDirection?: "ltr" | "rtl";
  /** Whether this is the end of the series (no more books) */
  isSeriesEnd?: boolean;
}

/**
 * Notification bar that appears at the top of the reader when at book boundaries.
 * Shows a message indicating the user can press again to navigate to adjacent book,
 * or that they've reached the end/beginning of the series.
 *
 * The chevron direction respects reading direction:
 * - LTR: at-start shows left chevron (go back), at-end shows right chevron (go forward)
 * - RTL: at-start shows right chevron (go back), at-end shows left chevron (go forward)
 */
export function BoundaryNotification({
  message,
  visible,
  type,
  readingDirection = "ltr",
  isSeriesEnd = false,
}: BoundaryNotificationProps) {
  // Determine which chevron to show based on boundary type and reading direction
  // The chevron should point in the direction of navigation:
  // - "at-start" means going backward: LTR=left chevron, RTL=right chevron
  // - "at-end" means going forward: LTR=right chevron, RTL=left chevron
  const showLeftChevron =
    !isSeriesEnd &&
    ((type === "at-start" && readingDirection === "ltr") ||
      (type === "at-end" && readingDirection === "rtl"));
  const showRightChevron =
    !isSeriesEnd &&
    ((type === "at-end" && readingDirection === "ltr") ||
      (type === "at-start" && readingDirection === "rtl"));

  return (
    <Transition
      mounted={visible && !!message}
      transition="slide-down"
      duration={200}
    >
      {(styles) => (
        <Box
          style={{
            ...styles,
            position: "absolute",
            top: "40%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            zIndex: 101,
            backgroundColor: isSeriesEnd
              ? "rgba(34, 139, 34, 0.92)"
              : "rgba(0, 0, 0, 0.92)",
            borderRadius: 12,
            padding: "20px 32px",
            maxWidth: "90%",
            boxShadow: "0 6px 20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {message && (
            <Stack gap={4} align="center">
              {showLeftChevron && <IconChevronLeft size={32} color="white" />}
              {showRightChevron && <IconChevronRight size={32} color="white" />}
              {isSeriesEnd && <IconCircleCheck size={32} color="white" />}
              <Text
                size="lg"
                c="white"
                fw={600}
                ta="center"
                style={{ whiteSpace: "nowrap" }}
              >
                {message.split("\n")[0]}
              </Text>
              {message.includes("\n") && (
                <Text
                  size="md"
                  ta="center"
                  style={{
                    whiteSpace: "nowrap",
                    color: "rgba(255,255,255,0.7)",
                  }}
                >
                  {message.split("\n")[1]}
                </Text>
              )}
            </Stack>
          )}
        </Box>
      )}
    </Transition>
  );
}
