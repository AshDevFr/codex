import { Alert, Stack, Text } from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import {
  type PreprocessingRule,
  PreprocessingRulesEditor,
} from "../PreprocessingRulesEditor";

interface PreprocessingTabProps {
  preprocessingRules: PreprocessingRule[];
  onPreprocessingRulesChange: (rules: PreprocessingRule[]) => void;
  testTitle: string;
  onTestTitleChange: (value: string) => void;
}

export function PreprocessingTab({
  preprocessingRules,
  onPreprocessingRulesChange,
  testTitle,
  onTestTitleChange,
}: PreprocessingTabProps) {
  return (
    <Stack gap="md">
      <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
        <Text size="sm">
          Transform series titles before metadata search. Rules are applied in
          order, before the search query template.
        </Text>
      </Alert>

      <PreprocessingRulesEditor
        value={preprocessingRules}
        onChange={onPreprocessingRulesChange}
        testInput={testTitle}
        onTestInputChange={onTestTitleChange}
        label="Title Preprocessing Rules"
        description="Transform series titles before metadata search. Rules are applied in order."
      />
    </Stack>
  );
}
