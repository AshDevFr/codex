import { Box, Group, SegmentedControl, Stack, Text } from "@mantine/core";
import { TriStateChip } from "./TriStateChip";
import type { FilterGroupState, FilterMode, TriState } from "@/types";
import classes from "./FilterGroup.module.css";

interface FilterOption {
	value: string;
	label: string;
	count?: number;
}

interface FilterGroupProps {
	/** Group title (e.g., "Genres", "Tags") */
	title: string;
	/** Available options to filter by */
	options: FilterOption[];
	/** Current filter state */
	state: FilterGroupState;
	/** Callback when a value's state changes */
	onValueChange: (value: string, state: TriState) => void;
	/** Callback when mode changes */
	onModeChange: (mode: FilterMode) => void;
	/** Whether to show the mode toggle (default: true) */
	showModeToggle?: boolean;
	/** Whether the group is disabled */
	disabled?: boolean;
}

/**
 * A filter group component that displays a collection of tri-state chips
 * with an optional mode toggle (All/Any).
 *
 * Usage:
 * - "All selected" (allOf): All included values must match
 * - "Any selected" (anyOf): Any included value can match
 * - Excluded values are always AND-ed (must NOT have any of them)
 */
export function FilterGroup({
	title,
	options,
	state,
	onValueChange,
	onModeChange,
	showModeToggle = true,
	disabled = false,
}: FilterGroupProps) {
	// Get the current state for a value
	const getValueState = (value: string): TriState => {
		return state.values.get(value) || "neutral";
	};

	return (
		<Stack gap="xs" className={classes.container}>
			<Group justify="space-between" align="center">
				<Text size="sm" fw={600} c="dimmed">
					{title}
				</Text>
				{showModeToggle && (
					<SegmentedControl
						size="xs"
						value={state.mode}
						onChange={(value) => onModeChange(value as FilterMode)}
						disabled={disabled}
						data={[
							{ label: "All", value: "allOf" },
							{ label: "Any", value: "anyOf" },
						]}
						className={classes.modeToggle}
					/>
				)}
			</Group>

			<Box className={classes.chipsContainer}>
				<Group gap="xs" wrap="wrap">
					{options.map((option) => (
						<TriStateChip
							key={option.value}
							label={option.label}
							state={getValueState(option.value)}
							onChange={(newState) => onValueChange(option.value, newState)}
							count={option.count}
							disabled={disabled}
						/>
					))}
				</Group>
			</Box>

			{options.length === 0 && (
				<Text size="sm" c="dimmed" fs="italic">
					No options available
				</Text>
			)}
		</Stack>
	);
}
