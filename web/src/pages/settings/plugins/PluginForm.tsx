import {
  Alert,
  Badge,
  Box,
  Button,
  Code,
  Collapse,
  Divider,
  Group,
  JsonInput,
  NumberInput,
  Select,
  Stack,
  Switch,
  Table,
  Tabs,
  Text,
  Textarea,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import type { useForm } from "@mantine/form";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertCircle,
  IconChevronDown,
  IconChevronRight,
} from "@tabler/icons-react";
import { useState } from "react";
import { CREDENTIAL_DELIVERY_OPTIONS, type PluginDto } from "@/api/plugins";

// Plugin form values type
export interface PluginFormValues {
  name: string;
  displayName: string;
  description: string;
  command: string;
  args: string;
  envVars: { key: string; value: string }[];
  workingDirectory: string;
  credentialDelivery: string;
  credentials: string;
  config: string;
  enabled: boolean;
  rateLimitEnabled: boolean;
  rateLimitRequestsPerMinute: number;
  requestTimeoutOverrideEnabled: boolean;
  requestTimeoutSeconds: number;
  syncCronSchedule: string;
}

export const defaultFormValues: PluginFormValues = {
  name: "",
  displayName: "",
  description: "",
  command: "",
  args: "",
  envVars: [],
  workingDirectory: "",
  credentialDelivery: "env",
  credentials: "",
  config: "",
  enabled: false,
  rateLimitEnabled: true,
  rateLimitRequestsPerMinute: 60,
  requestTimeoutOverrideEnabled: false,
  requestTimeoutSeconds: 30,
  syncCronSchedule: "",
};

// Normalize plugin name to slug format (lowercase alphanumeric with hyphens)
// Matches backend validation: lowercase alphanumeric and hyphens only
// Cannot start or end with a hyphen
export function normalizePluginName(value: string): string {
  return value
    .toLowerCase()
    .trim()
    .replace(/[\s_]+/g, "-") // spaces and underscores -> hyphens
    .replace(/-+/g, "-") // collapse multiple hyphens to single
    .replace(/[^a-z0-9-]/g, "") // remove invalid chars
    .replace(/^-+|-+$/g, ""); // trim leading/trailing hyphens
}

/**
 * Safely parse JSON with try-catch to handle malformed input.
 * Returns undefined if parsing fails, showing an error notification to the user.
 */
export function safeJsonParse(
  jsonString: string,
  fieldName: string,
): Record<string, unknown> | undefined {
  try {
    return JSON.parse(jsonString);
  } catch {
    notifications.show({
      title: "Invalid JSON",
      message: `The ${fieldName} field contains invalid JSON. Please check the format.`,
      color: "red",
    });
    return undefined;
  }
}

// Config schema help component - displays available configuration options
export function ConfigSchemaHelp({
  schema,
  defaultExpanded = false,
}: {
  schema: NonNullable<PluginDto["manifest"]>["configSchema"];
  defaultExpanded?: boolean;
}) {
  const [opened, { toggle }] = useDisclosure(defaultExpanded);

  if (!schema || !schema.fields || schema.fields.length === 0) {
    return null;
  }

  return (
    <Alert variant="light" color="blue" p="sm">
      <UnstyledButton onClick={toggle} w="100%">
        <Group justify="space-between" wrap="nowrap">
          <Text size="sm" fw={600} c="blue.7">
            Available Configuration Options ({schema.fields.length})
          </Text>
          {opened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
        </Group>
      </UnstyledButton>
      <Collapse in={opened}>
        <Stack gap="xs" mt="sm">
          {schema.description && <Text size="sm">{schema.description}</Text>}
          <Table withTableBorder={false} fz="sm" verticalSpacing={6}>
            <Table.Thead>
              <Table.Tr>
                <Table.Th style={{ width: "30%" }}>Option</Table.Th>
                <Table.Th>Description</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {schema.fields.map((field) => (
                <Table.Tr key={field.key}>
                  <Table.Td style={{ verticalAlign: "top" }}>
                    <Stack gap={4}>
                      <Group gap={4} wrap="nowrap">
                        <Code>{field.key}</Code>
                        {field.required && (
                          <Text component="span" c="red" size="xs">
                            *
                          </Text>
                        )}
                      </Group>
                      <Group gap={4} wrap="wrap">
                        <Badge size="xs" variant="light">
                          {field.type}
                        </Badge>
                        {field.default !== undefined &&
                          field.default !== null && (
                            <Code fz={10}>{JSON.stringify(field.default)}</Code>
                          )}
                      </Group>
                    </Stack>
                  </Table.Td>
                  <Table.Td style={{ verticalAlign: "top" }}>
                    <Text size="xs">{field.description || "-"}</Text>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Stack>
      </Collapse>
    </Alert>
  );
}

// Plugin form props
export interface PluginFormProps {
  form: ReturnType<typeof useForm<PluginFormValues>>;
  onSubmit: (values: PluginFormValues) => void;
  isLoading: boolean;
  onCancel: () => void;
  isCreate?: boolean;
  manifest?: PluginDto["manifest"];
}

export function PluginForm({
  form,
  onSubmit,
  isLoading,
  onCancel,
  isCreate,
  manifest,
}: PluginFormProps) {
  const [activeTab, setActiveTab] = useState<string | null>("general");
  const [nameManuallyEdited, setNameManuallyEdited] = useState(false);

  // Check which tabs have errors
  const generalTabErrors = isCreate
    ? !!(form.errors.name || form.errors.displayName)
    : !!form.errors.displayName;
  const executionTabErrors = !!form.errors.command;

  // Handle form submission with tab navigation on error
  const handleSubmit = form.onSubmit(onSubmit, (errors) => {
    // Navigate to the first tab with errors
    if (isCreate && errors.name) {
      setActiveTab("general");
    } else if (errors.displayName) {
      setActiveTab("general");
    } else if (errors.command) {
      setActiveTab("execution");
    }
  });

  return (
    <form onSubmit={handleSubmit}>
      <Tabs value={activeTab} onChange={setActiveTab}>
        <Tabs.List>
          <Tabs.Tab value="general" c={generalTabErrors ? "red" : undefined}>
            General{generalTabErrors ? " *" : ""}
          </Tabs.Tab>
          <Tabs.Tab
            value="execution"
            c={executionTabErrors ? "red" : undefined}
          >
            Execution{executionTabErrors ? " *" : ""}
          </Tabs.Tab>
          <Tabs.Tab value="credentials">Credentials</Tabs.Tab>
        </Tabs.List>

        <Box mt="md">
          <Tabs.Panel value="general">
            <Stack gap="md">
              {isCreate && (
                <TextInput
                  label="Name"
                  placeholder="mangabaka"
                  description="Unique identifier (lowercase alphanumeric with hyphens)"
                  withAsterisk
                  {...form.getInputProps("name")}
                  onChange={(e) => {
                    const value = e.currentTarget.value;
                    form.setFieldValue("name", value);
                    setNameManuallyEdited(value.length > 0);
                  }}
                  onBlur={(e) => {
                    form.setFieldValue(
                      "name",
                      normalizePluginName(e.currentTarget.value),
                    );
                  }}
                />
              )}
              <TextInput
                label="Display Name"
                placeholder="MangaBaka"
                description="Human-readable name shown in the UI"
                withAsterisk
                {...form.getInputProps("displayName")}
                onChange={(e) => {
                  const displayName = e.currentTarget.value;
                  form.setFieldValue("displayName", displayName);
                  // Auto-generate name from display name until user manually edits it (create mode only)
                  if (isCreate && !nameManuallyEdited) {
                    form.setFieldValue(
                      "name",
                      normalizePluginName(displayName),
                    );
                  }
                }}
              />
              <Textarea
                label="Description"
                placeholder="Fetch manga metadata from MangaBaka (MangaUpdates)"
                description="Optional description of what this plugin does"
                rows={2}
                {...form.getInputProps("description")}
              />
              {isCreate && (
                <Switch
                  label="Enable immediately"
                  description="Start the plugin after creation"
                  {...form.getInputProps("enabled", { type: "checkbox" })}
                />
              )}
            </Stack>
          </Tabs.Panel>

          <Tabs.Panel value="execution">
            <Stack gap="md">
              <TextInput
                label="Command"
                placeholder="npx"
                description="The command to execute (e.g., node, python, npx)"
                withAsterisk
                {...form.getInputProps("command")}
              />
              <Textarea
                label="Arguments"
                placeholder={`-y\n@ashdev/codex-plugin-metadata-mangabaka@1.0.0`}
                description="Command arguments, one per line"
                rows={3}
                {...form.getInputProps("args")}
              />
              <TextInput
                label="Working Directory"
                placeholder="/opt/codex/plugins/mangabaka"
                description="Optional working directory for the plugin process"
                {...form.getInputProps("workingDirectory")}
              />
              <JsonInput
                label="Configuration"
                placeholder='{"timeout": 30}'
                description="Optional JSON configuration passed to the plugin"
                validationError="Invalid JSON"
                formatOnBlur
                autosize
                minRows={6}
                maxRows={20}
                styles={{ input: { fontFamily: "monospace" } }}
                {...form.getInputProps("config")}
              />
              {manifest?.configSchema && (
                <ConfigSchemaHelp schema={manifest.configSchema} />
              )}
              <Divider label="Rate Limiting" labelPosition="center" />
              <Switch
                label="Enable Rate Limiting"
                description="Limit the number of requests per minute to protect external APIs"
                {...form.getInputProps("rateLimitEnabled", {
                  type: "checkbox",
                })}
              />
              {form.values.rateLimitEnabled && (
                <NumberInput
                  label="Requests per Minute"
                  description="Maximum number of requests allowed per minute"
                  placeholder="60"
                  min={1}
                  max={1000}
                  {...form.getInputProps("rateLimitRequestsPerMinute")}
                />
              )}
              <Divider label="RPC Timeout" labelPosition="center" />
              <Switch
                label="Override default request timeout"
                description="Increase if the plugin runs long operations (e.g. polling many series). Off uses the server default."
                {...form.getInputProps("requestTimeoutOverrideEnabled", {
                  type: "checkbox",
                })}
              />
              {form.values.requestTimeoutOverrideEnabled && (
                <NumberInput
                  label="Request Timeout (seconds)"
                  description="Per-call deadline for host → plugin RPCs. Long-running handlers (e.g. release polls) may pass their own larger deadline that overrides this."
                  placeholder="30"
                  min={1}
                  max={3600}
                  {...form.getInputProps("requestTimeoutSeconds")}
                />
              )}
              {manifest?.capabilities?.userReadSync && (
                <>
                  <Divider label="Automatic Sync" labelPosition="center" />
                  <TextInput
                    label="Sync Schedule (cron)"
                    placeholder="0 0 */6 * * *"
                    description="Cron expression that drives automatic syncs for users who opted into auto-sync. Leave empty to disable scheduled syncs (users can still sync manually)."
                    {...form.getInputProps("syncCronSchedule")}
                  />
                </>
              )}
            </Stack>
          </Tabs.Panel>

          <Tabs.Panel value="credentials">
            <Stack gap="md">
              <Select
                label="Credential Delivery"
                description="How credentials are passed to the plugin"
                data={CREDENTIAL_DELIVERY_OPTIONS.map((o) => ({
                  value: o.value,
                  label: o.label,
                }))}
                {...form.getInputProps("credentialDelivery")}
              />
              <Textarea
                label="Credentials"
                placeholder='{"api_key": "your-api-key"}'
                description="JSON object with credentials (will be encrypted). Leave empty to keep existing credentials."
                rows={3}
                {...form.getInputProps("credentials")}
              />
              <Alert
                icon={<IconAlertCircle size={16} />}
                color="yellow"
                variant="light"
              >
                Credentials are encrypted before storage and never displayed.
                When editing, leave the credentials field empty to keep existing
                values.
              </Alert>
            </Stack>
          </Tabs.Panel>
        </Box>
      </Tabs>

      <Group justify="flex-end" mt="xl">
        <Button variant="subtle" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" loading={isLoading}>
          {isCreate ? "Create Plugin" : "Save Changes"}
        </Button>
      </Group>
    </form>
  );
}
