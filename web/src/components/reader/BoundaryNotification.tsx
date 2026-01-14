import { Box, Text, Transition } from "@mantine/core";
import { IconChevronLeft, IconChevronRight } from "@tabler/icons-react";

interface BoundaryNotificationProps {
	/** The message to display */
	message: string | null;
	/** Whether the notification is visible */
	visible: boolean;
	/** The boundary type - affects the icon shown */
	type: "at-start" | "at-end" | "none";
}

/**
 * Notification bar that appears at the top of the reader when at book boundaries.
 * Shows a message indicating the user can press again to navigate to adjacent book.
 */
export function BoundaryNotification({
	message,
	visible,
	type,
}: BoundaryNotificationProps) {
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
						top: 60, // Below toolbar
						left: "50%",
						transform: "translateX(-50%)",
						zIndex: 101,
						backgroundColor: "rgba(0, 0, 0, 0.85)",
						borderRadius: 8,
						padding: "8px 16px",
						display: "flex",
						alignItems: "center",
						gap: 8,
						maxWidth: "90%",
					}}
				>
					{type === "at-start" && <IconChevronLeft size={18} color="white" />}
					<Text size="sm" c="white" style={{ whiteSpace: "nowrap" }}>
						{message}
					</Text>
					{type === "at-end" && <IconChevronRight size={18} color="white" />}
				</Box>
			)}
		</Transition>
	);
}
