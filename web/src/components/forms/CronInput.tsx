import type { TextInputProps } from "@mantine/core";
import { Group, Stack, Text, TextInput } from "@mantine/core";
import { IconClock } from "@tabler/icons-react";
import { CronExpressionParser } from "cron-parser";
import { toString as cronToString } from "cronstrue";
import { format } from "date-fns";
import { useMemo } from "react";

export interface CronInputProps
  extends Omit<TextInputProps, "value" | "onChange"> {
  /** Current cron string. Optional so it composes with `form.getInputProps`. */
  value?: string;
  onChange: (value: string) => void;
  showNextRun?: boolean;
}

// Normalize a cron expression's step syntax. Older cron-parser releases
// tolerated a bare `/n` step (e.g. `0 /2 * * *`), but 5.6+ rejects it, as does
// cronstrue, both of which require the `*/n` form. We accept the lenient input
// for UX and rewrite it before handing it to either library.
function normalizeCron(expression: string): string {
  const parts = expression.trim().split(/\s+/);
  if (parts.length !== 5) return expression;

  // Convert a leading-slash step like `/2` into `*/2` in each field.
  return parts
    .map((part) => (part.startsWith("/") ? `*${part}` : part))
    .join(" ");
}

// Validate cron expression using the parser
function isValidCron(expression: string): boolean {
  if (!expression.trim()) return false;

  try {
    CronExpressionParser.parse(normalizeCron(expression));
    return true;
  } catch {
    return false;
  }
}

// Get human-readable description of cron expression
function getCronDescription(expression: string): string | null {
  if (!expression.trim()) return null;

  try {
    // Normalize (converts /n to */n) so both the parser and cronstrue accept it.
    const normalized = normalizeCron(expression);
    CronExpressionParser.parse(normalized);
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
    const interval = CronExpressionParser.parse(normalizeCron(expression));
    const nextDate = interval.next();
    return nextDate.toDate();
  } catch {
    // Silently fail - invalid cron expressions are handled by validation
    return null;
  }
}

// Concise human-readable description of a cron expression for read-only display
// (e.g. showing users the admin-set sync cadence). Tolerant of the 6-field
// (seconds-precision) form the backend stores, and of empty/invalid input,
// returning `null` so callers can fall back to a "not set up" message.
export function describeCron(
  expression: string | null | undefined,
): string | null {
  if (!expression?.trim()) return null;
  try {
    return cronToString(expression.trim(), {
      throwExceptionOnParseError: true,
      verbose: false,
    });
  } catch {
    return null;
  }
}

export function CronInput({
  value = "",
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
