import {
  Alert,
  Box,
  Button,
  Card,
  Collapse,
  Grid,
  Group,
  SegmentedControl,
  Stack,
  Text,
  useComputedColorScheme,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconAlertCircle,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconCode,
  IconHelp,
  IconLayoutColumns,
  IconLayoutRows,
  IconTree,
} from "@tabler/icons-react";
import { githubDarkTheme, githubLightTheme, JsonEditor } from "json-edit-react";
import Prism from "prismjs";
import { useEffect, useMemo, useState } from "react";
import Editor from "react-simple-code-editor";
// Load dependencies in correct order for handlebars syntax highlighting
import "prismjs/components/prism-markup";
import "prismjs/components/prism-markup-templating";
import "prismjs/components/prism-handlebars";
import "prismjs/components/prism-markdown";
import { CustomMetadataDisplay } from "@/components/series";
import { getAvailableHelpers, validateTemplate } from "@/utils/templateEngine";
import {
  SAMPLE_SERIES_CONTEXT,
  type SeriesContextWithCustomMetadata,
} from "@/utils/templateUtils";

type LayoutMode = "side-by-side" | "stacked";
type TestDataViewMode = "tree" | "json";

export interface TemplateEditorProps {
  /**
   * The current template value
   */
  value: string;
  /**
   * Callback when the template value changes
   */
  onChange: (value: string) => void;
  /**
   * Whether the editor is disabled
   */
  disabled?: boolean;
  /**
   * Label for the editor
   */
  label?: string;
  /**
   * Description text
   */
  description?: string;
  /**
   * Initial sample series context for preview (defaults to SAMPLE_SERIES_CONTEXT).
   * The customMetadata field is editable, while other fields are read-only.
   */
  initialContext?: SeriesContextWithCustomMetadata;
  /**
   * Externally controlled series context (when provided, overrides internal state)
   */
  context?: SeriesContextWithCustomMetadata;
  /**
   * Callback when context changes (for external control)
   */
  onContextChange?: (context: SeriesContextWithCustomMetadata) => void;
}

/**
 * A code editor for Handlebars templates with syntax highlighting and live preview
 */
export function TemplateEditor({
  value,
  onChange,
  disabled = false,
  label = "Template",
  description,
  initialContext = SAMPLE_SERIES_CONTEXT,
  context: externalContext,
  onContextChange,
}: TemplateEditorProps) {
  const colorScheme = useComputedColorScheme("dark");
  const [helpOpened, { toggle: toggleHelp }] = useDisclosure(false);
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("side-by-side");
  const [testDataViewMode, setTestDataViewMode] =
    useState<TestDataViewMode>("tree");
  const [localValue, setLocalValue] = useState(value);
  const [rawJson, setRawJson] = useState<string>("");
  const [jsonError, setJsonError] = useState<string | null>(null);

  // Context state - use external if provided, otherwise internal
  const [internalContext, setInternalContext] =
    useState<SeriesContextWithCustomMetadata>(initialContext);

  // Use external context if provided, otherwise use internal
  const context = externalContext ?? internalContext;
  const setContext = (ctx: SeriesContextWithCustomMetadata) => {
    if (onContextChange) {
      onContextChange(ctx);
    } else {
      setInternalContext(ctx);
    }
  };

  // For backwards compatibility with the JsonEditor, expose customMetadata as testData
  const testData = context.customMetadata ?? {};
  const setTestData = (data: Record<string, unknown>) => {
    setContext({ ...context, customMetadata: data });
  };

  // Sync local value with prop
  useEffect(() => {
    setLocalValue(value);
  }, [value]);

  // Sync rawJson with testData when switching to JSON view or when testData changes
  useEffect(() => {
    setRawJson(JSON.stringify(testData, null, 2));
    setJsonError(null);
  }, [testData]);

  // Validate template
  const validation = useMemo(() => {
    return validateTemplate(localValue);
  }, [localValue]);

  // Handle value changes
  const handleChange = (newValue: string) => {
    setLocalValue(newValue);
    onChange(newValue);
  };

  // Handle test data changes from JsonEditor (tree view)
  const handleTestDataChange = (newData: unknown) => {
    setTestData(newData as Record<string, unknown>);
  };

  // Handle raw JSON changes (JSON view)
  const handleRawJsonChange = (newJson: string) => {
    setRawJson(newJson);
    try {
      const parsed = JSON.parse(newJson) as Record<string, unknown>;
      if (typeof parsed !== "object" || Array.isArray(parsed)) {
        setJsonError("Test data must be a JSON object");
        return;
      }
      setTestData(parsed);
      setJsonError(null);
    } catch (e) {
      setJsonError(e instanceof Error ? e.message : "Invalid JSON");
    }
  };

  // Syntax highlighting function for Prism
  const highlight = (code: string) => {
    return Prism.highlight(code, Prism.languages.handlebars, "handlebars");
  };

  const helpers = getAvailableHelpers();
  const jsonTheme = colorScheme === "dark" ? githubDarkTheme : githubLightTheme;

  // Fixed height for both editors to ensure visual alignment
  const editorHeight = layoutMode === "side-by-side" ? 400 : 300;

  // Template editor component
  const templateEditorSection = (
    <Box>
      <Group justify="space-between" mb="xs">
        <Text fw={500} size="sm">
          Template
        </Text>
        {validation.valid && localValue.trim() && (
          <Group gap="xs">
            <IconCheck size={14} color="var(--mantine-color-green-6)" />
            <Text size="xs" c="green">
              Valid
            </Text>
          </Group>
        )}
      </Group>
      <Box
        style={{
          border:
            colorScheme === "dark"
              ? "1px solid var(--mantine-color-dark-4)"
              : "1px solid var(--mantine-color-gray-4)",
          borderRadius: "var(--mantine-radius-sm)",
          overflow: "auto",
          height: editorHeight,
        }}
      >
        <Editor
          value={localValue}
          onValueChange={handleChange}
          highlight={highlight}
          disabled={disabled}
          padding={12}
          style={{
            fontFamily:
              'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
            fontSize: 13,
            lineHeight: 1.5,
            minHeight: editorHeight - 2, // Account for border
            backgroundColor:
              colorScheme === "dark"
                ? disabled
                  ? "var(--mantine-color-dark-7)"
                  : "var(--mantine-color-dark-6)"
                : disabled
                  ? "var(--mantine-color-gray-2)"
                  : "var(--mantine-color-gray-0)",
            color: "var(--mantine-color-text)",
          }}
          textareaClassName="template-editor-textarea"
        />
      </Box>
      {!validation.valid && (
        <Alert icon={<IconAlertCircle size={16} />} color="red" mt="xs">
          {validation.error || "Invalid template syntax"}
        </Alert>
      )}
    </Box>
  );

  // Test data editor component
  const testDataSection = (
    <Box>
      <Group justify="space-between" mb="xs">
        <Text fw={500} size="sm">
          Test Data
        </Text>
        <Group gap="xs">
          <SegmentedControl
            size="xs"
            value={testDataViewMode}
            onChange={(v) => setTestDataViewMode(v as TestDataViewMode)}
            data={[
              {
                value: "tree",
                label: (
                  <Group gap={4} wrap="nowrap" align="center">
                    <IconTree size={14} style={{ flexShrink: 0 }} />
                    <Text size="xs" lh={1}>
                      Tree
                    </Text>
                  </Group>
                ),
              },
              {
                value: "json",
                label: (
                  <Group gap={4} wrap="nowrap" align="center">
                    <IconCode size={14} style={{ flexShrink: 0 }} />
                    <Text size="xs" lh={1}>
                      JSON
                    </Text>
                  </Group>
                ),
              },
            ]}
          />
          <Button
            variant="subtle"
            size="xs"
            onClick={() => {
              const initialCustomMetadata = initialContext.customMetadata ?? {};
              setTestData(initialCustomMetadata);
              // Also reset rawJson directly since the effect may not trigger
              // if testData value doesn't change (e.g., when external state was undefined)
              setRawJson(JSON.stringify(initialCustomMetadata, null, 2));
              setJsonError(null);
            }}
          >
            Reset
          </Button>
        </Group>
      </Group>
      {jsonError && (
        <Alert
          icon={<IconAlertCircle size={16} />}
          color="red"
          variant="light"
          mb="xs"
        >
          {jsonError}
        </Alert>
      )}
      <Box
        style={{
          border:
            colorScheme === "dark"
              ? "1px solid var(--mantine-color-dark-4)"
              : "1px solid var(--mantine-color-gray-4)",
          borderRadius: "var(--mantine-radius-sm)",
          overflow: "auto",
          height: editorHeight,
          backgroundColor:
            colorScheme === "dark"
              ? "var(--mantine-color-dark-6)"
              : "var(--mantine-color-gray-0)",
        }}
      >
        {testDataViewMode === "tree" ? (
          <JsonEditor
            data={testData}
            setData={handleTestDataChange}
            theme={{
              ...jsonTheme,
              styles: {
                ...jsonTheme.styles,
                container: {
                  ...(typeof jsonTheme.styles?.container === "object"
                    ? jsonTheme.styles.container
                    : {}),
                  fontSize: 12,
                  fontFamily:
                    'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
                },
              },
            }}
            rootName="custom_metadata"
            collapse={2}
            enableClipboard={false}
            restrictEdit={false}
            restrictDelete={false}
            restrictAdd={false}
            restrictTypeSelection={false}
          />
        ) : (
          <textarea
            value={rawJson}
            onChange={(e) => handleRawJsonChange(e.target.value)}
            style={{
              width: "100%",
              height: "100%",
              fontFamily:
                'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
              fontSize: "12px",
              lineHeight: 1.5,
              padding: "12px",
              border: "none",
              outline: "none",
              resize: "none",
              backgroundColor: "transparent",
              color: "inherit",
            }}
            placeholder="{}"
            spellCheck={false}
          />
        )}
      </Box>
      <Text size="xs" c="dimmed" mt="xs">
        {testDataViewMode === "tree"
          ? "Click on values to edit them. Use the + button to add new fields."
          : "Edit the raw JSON directly. Changes are validated automatically."}
      </Text>
    </Box>
  );

  // Context section - read-only display of full series context (metadata, externalIds, etc.)
  const [metadataOpened, { toggle: toggleMetadata }] = useDisclosure(false);
  const metadataSection = (
    <Card withBorder padding="sm">
      <Group
        onClick={toggleMetadata}
        style={{ cursor: "pointer" }}
        justify="space-between"
      >
        <Group gap="xs">
          <Text size="sm" fw={500}>
            Series Context (Mock)
          </Text>
          <Text size="xs" c="dimmed">
            Available as <code>seriesId</code>, <code>bookCount</code>,{" "}
            <code>metadata.*</code>, <code>externalIds.*</code>
          </Text>
        </Group>
        {metadataOpened ? (
          <IconChevronDown size={16} />
        ) : (
          <IconChevronRight size={16} />
        )}
      </Group>
      <Collapse in={metadataOpened}>
        <Box
          mt="sm"
          style={{
            border:
              colorScheme === "dark"
                ? "1px solid var(--mantine-color-dark-4)"
                : "1px solid var(--mantine-color-gray-4)",
            borderRadius: "var(--mantine-radius-sm)",
            overflow: "auto",
            maxHeight: 300,
            backgroundColor:
              colorScheme === "dark"
                ? "var(--mantine-color-dark-6)"
                : "var(--mantine-color-gray-0)",
          }}
          className="metadata-json-editor"
        >
          <JsonEditor
            data={
              {
                seriesId: context.seriesId,
                bookCount: context.bookCount,
                metadata: context.metadata,
                externalIds: context.externalIds,
              } as unknown as Record<string, unknown>
            }
            setData={() => {}}
            theme={{
              ...jsonTheme,
              styles: {
                ...jsonTheme.styles,
                container: {
                  ...(typeof jsonTheme.styles?.container === "object"
                    ? jsonTheme.styles.container
                    : {}),
                  fontSize: 12,
                  fontFamily:
                    'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
                },
              },
            }}
            maxWidth="100%"
            rootName="context"
            collapse={1}
            enableClipboard={false}
            restrictEdit={true}
            restrictDelete={true}
            restrictAdd={true}
            restrictTypeSelection={true}
          />
        </Box>
        <Text size="xs" c="dimmed" mt="xs">
          This mock data represents the series context available in templates.
          Use <code>{"{{seriesId}}"}</code>, <code>{"{{bookCount}}"}</code>,{" "}
          <code>{"{{metadata.title}}"}</code>,{" "}
          <code>{"{{externalIds.plugin:source.id}}"}</code>, etc.
        </Text>
      </Collapse>
    </Card>
  );

  // Preview component - uses CustomMetadataDisplay to show exactly how it will render
  const previewSection = (
    <Box>
      <Text fw={500} size="sm" mb="xs">
        Live Preview
      </Text>
      <Box
        style={{
          backgroundColor:
            colorScheme === "dark"
              ? "var(--mantine-color-dark-6)"
              : "var(--mantine-color-gray-0)",
          padding: 12,
          borderRadius: "var(--mantine-radius-sm)",
          border:
            colorScheme === "dark"
              ? "1px solid var(--mantine-color-dark-4)"
              : "1px solid var(--mantine-color-gray-4)",
          minHeight: 100,
          maxHeight: 300,
          overflow: "auto",
        }}
      >
        {!validation.valid ? (
          <Text size="sm" c="dimmed" fs="italic">
            Fix template errors to see preview
          </Text>
        ) : (
          <CustomMetadataDisplay
            context={context}
            template={localValue}
            showErrors
          />
        )}
      </Box>
    </Box>
  );

  return (
    <Stack gap="md">
      {/* Label, description, and layout toggle */}
      <Group justify="space-between" align="flex-start">
        {label && (
          <Box>
            <Text fw={500} size="sm" mb={4}>
              {label}
            </Text>
            {description && (
              <Text size="xs" c="dimmed">
                {description}
              </Text>
            )}
          </Box>
        )}
        <SegmentedControl
          size="xs"
          value={layoutMode}
          onChange={(v) => setLayoutMode(v as LayoutMode)}
          data={[
            {
              value: "side-by-side",
              label: (
                <Group gap={4} wrap="nowrap" align="center">
                  <IconLayoutColumns size={14} style={{ flexShrink: 0 }} />
                  <Text size="xs" lh={1}>
                    Side by Side
                  </Text>
                </Group>
              ),
            },
            {
              value: "stacked",
              label: (
                <Group gap={4} wrap="nowrap" align="center">
                  <IconLayoutRows size={14} style={{ flexShrink: 0 }} />
                  <Text size="xs" lh={1}>
                    Stacked
                  </Text>
                </Group>
              ),
            },
          ]}
        />
      </Group>

      {/* Main editor area */}
      {layoutMode === "side-by-side" ? (
        <Stack gap="md">
          <Grid gutter="md">
            <Grid.Col span={6}>{templateEditorSection}</Grid.Col>
            <Grid.Col span={6}>{testDataSection}</Grid.Col>
          </Grid>
          {previewSection}
        </Stack>
      ) : (
        <Stack gap="md">
          {templateEditorSection}
          {testDataSection}
          {previewSection}
        </Stack>
      )}

      {/* Metadata section - shows available series metadata fields */}
      {metadataSection}

      {/* Help section */}
      <Card withBorder padding="sm">
        <Group
          onClick={toggleHelp}
          style={{ cursor: "pointer" }}
          justify="space-between"
        >
          <Group gap="xs">
            <IconHelp size={16} />
            <Text size="sm" fw={500}>
              Template Syntax Help
            </Text>
          </Group>
          {helpOpened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
        </Group>
        <Collapse in={helpOpened}>
          <Stack gap="md" mt="md">
            <Box>
              <Text size="sm" fw={500} mb="xs">
                Basic Syntax
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>{"{{field}}"}</code> - Output a field value
                  </li>
                  <li>
                    <code>{"{{customMetadata.field}}"}</code> - Access nested
                    fields
                  </li>
                  <li>
                    <code>{"{{#if field}}...{{/if}}"}</code> - Conditional
                    rendering
                  </li>
                  <li>
                    <code>{"{{#each array}}...{{/each}}"}</code> - Loop over
                    arrays
                  </li>
                  <li>
                    <code>{"{{@key}}"}</code> - Current key when iterating
                    objects
                  </li>
                  <li>
                    <code>{"{{this}}"}</code> - Current value in loops
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Available Helpers
              </Text>
              <Group gap="xs">
                {helpers.map((helper) => (
                  <Text
                    key={helper}
                    size="xs"
                    style={{
                      fontFamily: "monospace",
                      backgroundColor:
                        colorScheme === "dark"
                          ? "var(--mantine-color-dark-6)"
                          : "var(--mantine-color-gray-2)",
                      padding: "2px 6px",
                      borderRadius: 4,
                    }}
                  >
                    {helper}
                  </Text>
                ))}
              </Group>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Helper Examples
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>
                      {
                        '{{formatDate customMetadata.started_date "MMM d, yyyy"}}'
                      }
                    </code>{" "}
                    - Format dates
                  </li>
                  <li>
                    <code>{'{{truncate metadata.summary 50 "..."}}'}</code> -
                    Truncate text
                  </li>
                  <li>
                    <code>{'{{join metadata.genres ", "}}'}</code> - Join array
                    items
                  </li>
                  <li>
                    <code>{"{{json customMetadata}}"}</code> - Output as JSON
                  </li>
                  <li>
                    <code>
                      {
                        '{{#ifEquals metadata.status "ongoing"}}...{{/ifEquals}}'
                      }
                    </code>{" "}
                    - Compare values
                  </li>
                  <li>
                    <code>{'{{default metadata.publisher "Unknown"}}'}</code> -
                    Default for missing values
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Available Variables
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <p style={{ margin: "0 0 8px 0" }}>
                  Templates have access to the full series context:
                </p>
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>seriesId</code> - Series UUID
                  </li>
                  <li>
                    <code>bookCount</code> - Number of books in the series
                  </li>
                  <li>
                    <code>metadata.*</code> - Built-in series metadata
                  </li>
                  <li>
                    <code>externalIds.*</code> - External IDs from plugins
                  </li>
                  <li>
                    <code>customMetadata.*</code> - User-defined custom fields
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Metadata Fields
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>metadata.title</code>, <code>metadata.summary</code>,{" "}
                    <code>metadata.publisher</code> - Basic info
                  </li>
                  <li>
                    <code>metadata.year</code>, <code>metadata.ageRating</code>,{" "}
                    <code>metadata.language</code> - Publication details
                  </li>
                  <li>
                    <code>metadata.status</code> - Series status (ongoing,
                    ended, hiatus, etc.)
                  </li>
                  <li>
                    <code>metadata.readingDirection</code> - Reading direction
                    (ltr, rtl, ttb, webtoon)
                  </li>
                  <li>
                    <code>metadata.genres</code>, <code>metadata.tags</code> -
                    Arrays of strings
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                External IDs
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>{"externalIds.plugin:source.id"}</code> - External ID
                    value
                  </li>
                  <li>
                    <code>{"externalIds.plugin:source.url"}</code> - External
                    URL
                  </li>
                </ul>
              </Text>
            </Box>
          </Stack>
        </Collapse>
      </Card>
    </Stack>
  );
}
