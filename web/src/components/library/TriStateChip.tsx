import { Badge, UnstyledButton } from "@mantine/core";
import { IconCheck, IconX } from "@tabler/icons-react";
import type { TriState } from "@/types";
import classes from "./TriStateChip.module.css";

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
}

/**
 * A tri-state chip component for filter selection.
 *
 * States cycle through: neutral → include → exclude → neutral
 *
 * Visual indicators:
 * - neutral: outlined, no icon
 * - include: filled blue, checkmark icon
 * - exclude: filled red, X icon
 */
export function TriStateChip({
	label,
	state,
	onChange,
	count,
	disabled = false,
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

	const getVariant = (): "outline" | "filled" | "light" => {
		switch (state) {
			case "include":
				return "filled";
			case "exclude":
				return "filled";
			default:
				return "outline";
		}
	};

	const getColor = (): string => {
		switch (state) {
			case "include":
				return "blue";
			case "exclude":
				return "red";
			default:
				return "gray";
		}
	};

	const getIcon = () => {
		switch (state) {
			case "include":
				return <IconCheck size={12} />;
			case "exclude":
				return <IconX size={12} />;
			default:
				return null;
		}
	};

	return (
		<UnstyledButton
			onClick={handleClick}
			disabled={disabled}
			className={classes.button}
		>
			<Badge
				variant={getVariant()}
				color={getColor()}
				size="lg"
				radius="sm"
				leftSection={getIcon()}
				className={classes.badge}
				data-state={state}
				data-disabled={disabled || undefined}
			>
				{label}
				{count !== undefined && (
					<Badge
						size="xs"
						variant="light"
						color={getColor()}
						ml={6}
						className={classes.count}
					>
						{count}
					</Badge>
				)}
			</Badge>
		</UnstyledButton>
	);
}
