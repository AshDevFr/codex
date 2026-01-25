import { ActionIcon, Badge, Group, Text } from "@mantine/core";
import { IconX } from "@tabler/icons-react";
import { useBookFilterState } from "@/hooks/useBookFilterState";
import type { BookFilterState, FilterGroupState, TriState } from "@/types";
import styles from "./ActiveFilters.module.css";

/**
 * Displays active book filter chips with the ability to remove individual filters.
 *
 * Shows chips for each active filter value, grouped by category.
 * Include filters are shown in blue, exclude filters in red.
 * Clicking the X on a chip removes that filter.
 */
export function ActiveBookFilters() {
	const {
		filters,
		hasActiveFilters,
		setGenreState,
		setTagState,
		setReadStatusState,
		setHasErrorState,
		clearAll,
	} = useBookFilterState();

	if (!hasActiveFilters) {
		return null;
	}

	// Helper to create chips for a filter group
	const renderGroupChips = (
		group: FilterGroupState,
		groupName: keyof Omit<BookFilterState, "hasError">,
		label: string,
		onRemove: (value: string, state: TriState) => void,
	) => {
		const chips: React.ReactNode[] = [];

		for (const [value, state] of group.values) {
			if (state === "neutral") continue;

			const isExclude = state === "exclude";
			chips.push(
				<Badge
					key={`${groupName}-${value}`}
					variant="filled"
					color={isExclude ? "red" : "blue"}
					size="md"
					className={styles.chip}
					rightSection={
						<ActionIcon
							size="xs"
							variant="transparent"
							color="white"
							onClick={() => onRemove(value, "neutral")}
							aria-label={`Remove ${value} filter`}
						>
							<IconX size={12} />
						</ActionIcon>
					}
				>
					{isExclude ? "NOT " : ""}
					{label}: {value}
				</Badge>,
			);
		}

		return chips;
	};

	// Render hasError chip
	const renderHasErrorChip = () => {
		if (filters.hasError === "neutral") return [];

		const isExclude = filters.hasError === "exclude";
		return [
			<Badge
				key="hasError"
				variant="filled"
				color={isExclude ? "red" : "blue"}
				size="md"
				className={styles.chip}
				rightSection={
					<ActionIcon
						size="xs"
						variant="transparent"
						color="white"
						onClick={() => setHasErrorState("neutral")}
						aria-label="Remove has error filter"
					>
						<IconX size={12} />
					</ActionIcon>
				}
			>
				{isExclude ? "Has Error: No" : "Has Error: Yes"}
			</Badge>,
		];
	};

	const allChips = [
		...renderGroupChips(filters.genres, "genres", "Genre", setGenreState),
		...renderGroupChips(filters.tags, "tags", "Tag", setTagState),
		...renderGroupChips(
			filters.readStatus,
			"readStatus",
			"Read Status",
			setReadStatusState,
		),
		...renderHasErrorChip(),
	];

	return (
		<Group gap="xs" className={styles.container}>
			<Text size="sm" c="dimmed" fw={500}>
				Filters:
			</Text>
			{allChips}
			<Badge
				variant="outline"
				color="gray"
				size="md"
				className={styles.clearButton}
				onClick={clearAll}
				style={{ cursor: "pointer" }}
			>
				Clear all
			</Badge>
		</Group>
	);
}
