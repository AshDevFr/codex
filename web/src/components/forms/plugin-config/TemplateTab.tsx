import {
  Alert,
  Badge,
  Card,
  Code,
  Group,
  Paper,
  Stack,
  Text,
  Textarea,
  Tooltip,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import { useMemo } from "react";
import { SAMPLE_SERIES_CONTEXT } from "@/utils/templateUtils";
import {
  type PluginConfigForm,
  renderTemplatePreview,
  TEMPLATE_HELPERS,
} from "./types";

interface TemplateTabProps {
  form: PluginConfigForm;
}

export function TemplateTab({ form }: TemplateTabProps) {
  const templatePreview = useMemo(
    () => renderTemplatePreview(form.values.searchQueryTemplate),
    [form.values.searchQueryTemplate],
  );

  return (
    <Stack gap="md">
      <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
        <Text size="sm">
          Customize the search query using Handlebars syntax. The template has
          access to series context data shown below.
        </Text>
      </Alert>

      <Stack gap="xs">
        <Text fw={500} size="sm">
          Search Query Template
        </Text>
        <Textarea
          placeholder="{{metadata.title}}"
          rows={2}
          styles={{ input: { fontFamily: "monospace" } }}
          {...form.getInputProps("searchQueryTemplate")}
        />

        <Group gap="xs" align="center">
          <Text size="xs" c="dimmed">
            Helpers:
          </Text>
          {TEMPLATE_HELPERS.map((helper) => (
            <Tooltip
              key={helper.name}
              label={`${helper.description} — ${helper.example}`}
            >
              <Badge
                size="xs"
                variant="light"
                color="blue"
                style={{ cursor: "help", textTransform: "none" }}
              >
                {helper.name}
              </Badge>
            </Tooltip>
          ))}
        </Group>

        <Paper p="xs" withBorder bg="var(--mantine-color-dark-7)">
          <Group gap="xs">
            <Text size="xs" c="dimmed">
              Result:
            </Text>
            <Text size="xs" ff="monospace">
              {templatePreview}
            </Text>
          </Group>
        </Paper>
      </Stack>

      <Card padding="sm" withBorder bg="var(--mantine-color-dark-7)">
        <Stack gap="xs">
          <Group justify="space-between" align="center">
            <Text size="xs" fw={500}>
              Available Context
            </Text>
            <Text size="xs" c="dimmed">
              Access fields using dot notation, e.g.,{" "}
              <Code style={{ fontSize: 10 }}>{"{{metadata.title}}"}</Code>
            </Text>
          </Group>
          <Textarea
            size="xs"
            value={JSON.stringify(SAMPLE_SERIES_CONTEXT, null, 2)}
            readOnly
            rows={10}
            styles={{
              input: { fontFamily: "monospace", fontSize: "11px" },
            }}
          />
        </Stack>
      </Card>
    </Stack>
  );
}
