import type { TagsInputProps } from "@mantine/core";
import { ActionIcon, Group, TagsInput, Tooltip } from "@mantine/core";
import { IconLock, IconLockOpen } from "@tabler/icons-react";

export interface LockableChipInputProps
	extends Omit<TagsInputProps, "value" | "onChange"> {
	/** Current values */
	value: string[];
	/** Callback when values change */
	onChange: (value: string[]) => void;
	/** Whether the field is locked */
	locked: boolean;
	/** Callback when lock state changes */
	onLockChange: (locked: boolean) => void;
	/** Original values (for auto-lock detection) */
	originalValue?: string[];
	/** Whether to auto-lock when value differs from original */
	autoLock?: boolean;
}

/**
 * A chip/tag input with a lock icon to indicate if the field is protected
 * from automatic updates (e.g., scanner re-analysis).
 *
 * When locked, the field value will not be overwritten during automatic
 * metadata updates.
 */
export function LockableChipInput({
	value,
	onChange,
	locked,
	onLockChange,
	originalValue,
	autoLock = true,
	...props
}: LockableChipInputProps) {
	const arraysEqual = (a: string[], b: string[]): boolean => {
		if (a.length !== b.length) return false;
		const sortedA = [...a].sort();
		const sortedB = [...b].sort();
		return sortedA.every((val, index) => val === sortedB[index]);
	};

	const handleChange = (newValue: string[]) => {
		onChange(newValue);

		// Auto-lock when value differs from original
		if (autoLock && originalValue !== undefined && !locked) {
			if (!arraysEqual(newValue, originalValue)) {
				onLockChange(true);
			}
		}
	};

	const toggleLock = () => {
		onLockChange(!locked);
	};

	return (
		<Group gap="xs" wrap="nowrap" align="flex-end">
			<Tooltip
				label={
					locked
						? "Locked: Protected from automatic updates"
						: "Unlocked: Can be updated automatically"
				}
				position="left"
			>
				<ActionIcon
					variant="subtle"
					color={locked ? "orange" : "gray"}
					onClick={toggleLock}
					aria-label={locked ? "Unlock field" : "Lock field"}
				>
					{locked ? <IconLock size={18} /> : <IconLockOpen size={18} />}
				</ActionIcon>
			</Tooltip>
			<TagsInput
				value={value}
				onChange={handleChange}
				style={{ flex: 1 }}
				{...props}
			/>
		</Group>
	);
}
