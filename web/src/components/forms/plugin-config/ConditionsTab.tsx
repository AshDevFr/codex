import { Alert, Stack, Text } from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import {
  type AutoMatchConditions,
  ConditionsEditor,
} from "../ConditionsEditor";

interface ConditionsTabProps {
  autoMatchConditions: AutoMatchConditions | null;
  onAutoMatchConditionsChange: (conditions: AutoMatchConditions | null) => void;
}

export function ConditionsTab({
  autoMatchConditions,
  onAutoMatchConditionsChange,
}: ConditionsTabProps) {
  return (
    <Stack gap="md">
      <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
        <Text size="sm">
          Define conditions that control when auto-matching runs for this
          plugin. Without conditions, auto-matching will run for all series.
        </Text>
      </Alert>

      <ConditionsEditor
        value={autoMatchConditions}
        onChange={onAutoMatchConditionsChange}
        label="Auto-Match Conditions"
        description="Define conditions that must be met for auto-matching to run."
      />
    </Stack>
  );
}
