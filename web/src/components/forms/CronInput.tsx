import type { TextInputProps } from "@mantine/core";
import { Group, Stack, Text, TextInput } from "@mantine/core";
import { IconClock } from "@tabler/icons-react";
import { CronExpressionParser } from "cron-parser";
import { toString as cronToString } from "cronstrue";
import { format } from "date-fns";
import { useMemo } from "react";

export interface CronInputProps
	extends Omit<TextInputProps, "value" | "onChange"> {
	value: string;
	onChange: (value: string) => void;
	showNextRun?: boolean;
}

// Validate cron expression using the parser
function isValidCron(expression: string): boolean {
	if (!expression.trim()) return false;

	try {
		CronExpressionParser.parse(expression);
		return true;
	} catch {
		return false;
	}
}

// Normalize cron expression for cronstrue compatibility
// cronstrue expects */n format, but cron-parser accepts /n as well
function normalizeForCronstrue(expression: string): string {
	const parts = expression.trim().split(/\s+/);
	if (parts.length !== 5) return expression;

	// Convert /n to */n in each field for cronstrue compatibility
	return parts
		.map((part) => {
			// If part starts with / (like /2), convert to */2
			if (part.startsWith("/")) {
				return `*${part}`;
			}
			return part;
		})
		.join(" ");
}

// Get human-readable description of cron expression
function getCronDescription(expression: string): string | null {
	if (!expression.trim()) return null;

	try {
		// Validate first with parser
		CronExpressionParser.parse(expression);
		// Normalize for cronstrue (converts /n to */n)
		const normalized = normalizeForCronstrue(expression);
		return cronToString(normalized, {
			throwExceptionOnParseError: false,
			verbose: true,
		});
	} catch {
		return null;
	}
}

// Get next run time for cron expression
function getNextRunTime(expression: string): Date | null {
	if (!expression.trim()) return null;

	try {
		const interval = CronExpressionParser.parse(expression);
		const nextDate = interval.next();
		return nextDate.toDate();
	} catch {
		// Silently fail - invalid cron expressions are handled by validation
		return null;
	}
}

export function CronInput({
	value,
	onChange,
	showNextRun = true,
	error,
	...props
}: CronInputProps) {
	const isValid = useMemo(() => {
		if (!value.trim()) return true; // Empty is valid (not required by default)
		return isValidCron(value);
	}, [value]);

	const description = useMemo(() => {
		if (!isValid) return null;
		return getCronDescription(value);
	}, [value, isValid]);
	const nextRun = useMemo(() => {
		if (!isValid) return null;
		return getNextRunTime(value);
	}, [value, isValid]);

	const displayError =
		error ||
		(!isValid && value.trim()
			? "Invalid cron expression. Format: minute hour day month weekday"
			: undefined);

	return (
		<Stack gap="xs">
			<TextInput
				{...props}
				value={value}
				onChange={(e) => onChange(e.currentTarget.value)}
				error={displayError}
				styles={{
					input: {
						fontFamily: "monospace",
					},
				}}
			/>

			{value.trim() && isValid && description && description.trim() && (
				<Group gap="xs" align="center">
					<Text size="sm" c="blue">
						{description}
					</Text>
					{showNextRun && nextRun && (
						<>
							<Text size="sm" c="dimmed">
								-
							</Text>
							<IconClock size={14} style={{ opacity: 0.7 }} />
							<Text size="sm" c="dimmed">
								{format(nextRun, "yyyy-MM-dd HH:mm:ss")}
							</Text>
						</>
					)}
				</Group>
			)}
		</Stack>
	);
}
