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
  IconBook,
  IconBooks,
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
import {
  getAvailableHelpers,
  renderTemplate,
  validateTemplate,
} from "@/utils/templateEngine";
import {
  type BookContextWithCustomMetadata,
  SAMPLE_BOOK_CONTEXT,
  SAMPLE_SERIES_CONTEXT,
  type TemplateContext,
} from "@/utils/templateUtils";

type LayoutMode = "side-by-side" | "stacked";
type TestDataViewMode = "tree" | "json";
type ContextPreviewType = "series" | "book";

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
   * Initial sample context for preview (defaults to SAMPLE_SERIES_CONTEXT).
   * The customMetadata field is editable, while other fields are read-only.
   */
  initialContext?: TemplateContext;
  /**
   * Externally controlled context (when provided, overrides internal state)
   */
  context?: TemplateContext;
  /**
   * Callback when context changes (for external control)
   */
  onContextChange?: (context: TemplateContext) => void;
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

  // Context preview type toggle (series vs book)
  const [contextPreviewType, setContextPreviewType] =
    useState<ContextPreviewType>(
      initialContext.type === "book" ? "book" : "series",
    );

  // Context state - use external if provided, otherwise internal
  const [internalContext, setInternalContext] =
    useState<TemplateContext>(initialContext);

  // Use external context if provided, otherwise use internal
  const context = externalContext ?? internalContext;
  const setContext = (ctx: TemplateContext) => {
    if (onContextChange) {
      onContextChange(ctx);
    } else {
      setInternalContext(ctx);
    }
  };

  const isBookContext = contextPreviewType === "book";

  // Handle context type switching
  const handleContextTypeChange = (newType: string) => {
    const type = newType as ContextPreviewType;
    setContextPreviewType(type);
    // Preserve current customMetadata when switching
    const currentCustomMetadata = context.customMetadata;
    const newContext =
      type === "book"
        ? { ...SAMPLE_BOOK_CONTEXT, customMetadata: currentCustomMetadata }
        : { ...SAMPLE_SERIES_CONTEXT, customMetadata: currentCustomMetadata };
    setContext(newContext);
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
  const [selectedHelper, setSelectedHelper] = useState<string | null>(null);

  // Helper reference: description + example for each helper
  const helperReference: Record<string, { desc: string; example: string }> = {
    formatDate: {
      desc: "Format a date string",
      example: '{{formatDate customMetadata.started_date "MMM d, yyyy"}}',
    },
    ifEquals: {
      desc: "Block: render content when two values match",
      example:
        '{{#ifEquals metadata.status "ongoing"}}Currently running{{/ifEquals}}',
    },
    ifNotEquals: {
      desc: "Block: render content when two values differ",
      example:
        '{{#ifNotEquals metadata.status "ended"}}Not finished{{/ifNotEquals}}',
    },
    eq: {
      desc: "Inline equality — returns boolean for subexpressions",
      example: '{{#if (eq type "book")}}This is a book{{/if}}',
    },
    ne: {
      desc: "Inline not-equal — returns boolean for subexpressions",
      example: '{{#if (ne metadata.status "ended")}}Still running{{/if}}',
    },
    oneOf: {
      desc: "Check if a value matches any of the given options",
      example:
        '{{#if (oneOf (lowercase this.label) "en" "romaji" "native")}}{{this.title}}{{/if}}',
    },
    inArray: {
      desc: "Check if an array contains a value",
      example: '{{#if (inArray metadata.genres "Action")}}Action genre!{{/if}}',
    },
    json: { desc: "Output JSON representation", example: "{{json metadata}}" },
    truncate: {
      desc: "Truncate a string to a max length",
      example: '{{truncate metadata.summary 50 "..."}}',
    },
    lowercase: {
      desc: "Convert to lowercase",
      example: "{{lowercase metadata.title}}",
    },
    uppercase: {
      desc: "Convert to uppercase",
      example: "{{uppercase metadata.status}}",
    },
    capitalize: {
      desc: "Capitalize the first letter",
      example: "{{capitalize metadata.status}}",
    },
    first: {
      desc: "Block: render the first N items of an array",
      example: "{{#first metadata.genres 3}}{{this}}, {{/first}}",
    },
    join: {
      desc: "Join array items with a separator",
      example: '{{join metadata.genres ", "}}',
    },
    exists: {
      desc: "Block: render content if a value is not null/empty",
      example:
        "{{#exists metadata.publisher}}Publisher: {{metadata.publisher}}{{/exists}}",
    },
    length: {
      desc: "Get the length of an array or string",
      example: "{{length metadata.genres}} genres",
    },
    gt: {
      desc: "Block: render if first value is greater than second",
      example: "{{#gt (length metadata.genres) 3}}Many genres{{/gt}}",
    },
    lt: {
      desc: "Block: render if first value is less than second",
      example: "{{#lt bookCount 5}}Small series{{/lt}}",
    },
    and: {
      desc: "Block: render if all conditions are truthy (variadic)",
      example:
        '{{#and metadata.publisher (eq metadata.status "ongoing")}}Active publisher{{/and}}',
    },
    or: {
      desc: "Block: render if any condition is truthy (variadic)",
      example:
        '{{#or (eq (lowercase this.label) "en") (eq (lowercase this.label) "romaji") (eq (lowercase this.label) "native")}}{{this.title}}{{/or}}',
    },
    lookup: {
      desc: "Access an object property dynamically",
      example: "{{lookup metadata fieldName}}",
    },
    default: {
      desc: "Provide a fallback for missing values",
      example: '{{default metadata.publisher "Unknown"}}',
    },
    urlencode: {
      desc: "URL-encode a string",
      example: "[Search](https://example.com?q={{urlencode metadata.title}})",
    },
    replace: {
      desc: "Replace all occurrences of a substring",
      example: '{{replace metadata.title " " "-"}}',
    },
    split: {
      desc: "Split a string and get an item by index",
      example: '{{split metadata.title "-" 0}}',
    },
    includes: {
      desc: "Block: render if a string contains a substring",
      example: '{{#includes metadata.title "manga"}}Is Manga{{/includes}}',
    },
    math: {
      desc: "Basic arithmetic (+, -, *, /, %)",
      example: '{{math metadata.year "+" 1}}',
    },
    padStart: {
      desc: "Pad a value with leading characters",
      example: '{{padStart number 3 "0"}}',
    },
    trim: { desc: "Trim whitespace", example: "{{trim metadata.title}}" },
  };

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
            rootName="customMetadata"
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
          : "Edit the raw JSON directly. Changes are validated automatically."}{" "}
        Access via <code>{"{{customMetadata.field}}"}</code> (or{" "}
        <code>{"{{custom_metadata.field}}"}</code>).
      </Text>
    </Box>
  );

  // Context section - read-only display of full context (metadata, externalIds, etc.)
  const [metadataOpened, { toggle: toggleMetadata }] = useDisclosure(false);

  // Build the JSON viewer data based on context type
  const contextViewerData = isBookContext
    ? ({
        type: "book",
        bookId: (context as BookContextWithCustomMetadata).bookId,
        seriesId: (context as BookContextWithCustomMetadata).seriesId,
        libraryId: (context as BookContextWithCustomMetadata).libraryId,
        fileFormat: (context as BookContextWithCustomMetadata).fileFormat,
        pageCount: (context as BookContextWithCustomMetadata).pageCount,
        fileSize: (context as BookContextWithCustomMetadata).fileSize,
        metadata: context.metadata,
        externalIds: context.externalIds,
        customMetadata: context.customMetadata,
        series: (context as BookContextWithCustomMetadata).series,
      } as unknown as Record<string, unknown>)
    : ({
        type: "series",
        seriesId: (context as { seriesId?: string }).seriesId,
        bookCount: (context as { bookCount?: number }).bookCount,
        metadata: context.metadata,
        externalIds: context.externalIds,
        customMetadata: context.customMetadata,
      } as unknown as Record<string, unknown>);

  const metadataSection = (
    <Card withBorder padding="sm">
      <Group
        onClick={toggleMetadata}
        style={{ cursor: "pointer" }}
        justify="space-between"
      >
        <Group gap="xs">
          <Text size="sm" fw={500}>
            {isBookContext ? "Book" : "Series"} Context (Mock)
          </Text>
          <Text size="xs" c="dimmed">
            {isBookContext ? (
              <>
                Available as <code>bookId</code>, <code>seriesId</code>,{" "}
                <code>metadata.*</code>, <code>externalIds.*</code>,{" "}
                <code>series.*</code>
              </>
            ) : (
              <>
                Available as <code>seriesId</code>, <code>bookCount</code>,{" "}
                <code>metadata.*</code>, <code>externalIds.*</code>
              </>
            )}
          </Text>
        </Group>
        <Group gap="xs">
          <SegmentedControl
            size="xs"
            value={contextPreviewType}
            onChange={handleContextTypeChange}
            onClick={(e) => e.stopPropagation()}
            data={[
              {
                value: "series",
                label: (
                  <Group gap={4} wrap="nowrap" align="center">
                    <IconBooks size={14} style={{ flexShrink: 0 }} />
                    <Text size="xs" lh={1}>
                      Series
                    </Text>
                  </Group>
                ),
              },
              {
                value: "book",
                label: (
                  <Group gap={4} wrap="nowrap" align="center">
                    <IconBook size={14} style={{ flexShrink: 0 }} />
                    <Text size="xs" lh={1}>
                      Book
                    </Text>
                  </Group>
                ),
              },
            ]}
          />
          {metadataOpened ? (
            <IconChevronDown size={16} />
          ) : (
            <IconChevronRight size={16} />
          )}
        </Group>
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
            maxHeight: 400,
            backgroundColor:
              colorScheme === "dark"
                ? "var(--mantine-color-dark-6)"
                : "var(--mantine-color-gray-0)",
          }}
          className="metadata-json-editor"
        >
          <JsonEditor
            data={contextViewerData}
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
          {isBookContext ? (
            <>
              This mock data represents the book context available in templates.
              Use <code>{"{{bookId}}"}</code>,{" "}
              <code>{"{{metadata.title}}"}</code>,{" "}
              <code>{"{{metadata.authors}}"}</code>,{" "}
              <code>{"{{series.metadata.title}}"}</code>, etc. The{" "}
              <code>type</code> field is <code>&quot;book&quot;</code>.
            </>
          ) : (
            <>
              This mock data represents the series context available in
              templates. Use <code>{"{{seriesId}}"}</code>,{" "}
              <code>{"{{bookCount}}"}</code>,{" "}
              <code>{"{{metadata.title}}"}</code>,{" "}
              <code>{"{{externalIds.plugin:source.id}}"}</code>, etc. The{" "}
              <code>type</code> field is <code>&quot;series&quot;</code>.
            </>
          )}
        </Text>
      </Collapse>
    </Card>
  );

  // Translate cryptic Handlebars errors into actionable messages
  const describeRenderError = (error: string): string => {
    if (error.includes("options.inverse is not a function")) {
      return `A block helper received the wrong number of arguments. Check that helpers like #ifEquals only receive exactly 2 values. To compare multiple conditions, use: {{#or (eq a "x") (eq b "y")}}`;
    }
    if (error.includes("options.fn is not a function")) {
      return `A block helper was called incorrectly. Make sure you're using the #block syntax: {{#helperName arg1 arg2}}...{{/helperName}}`;
    }
    if (error.includes("is not a function")) {
      return `${error}. This usually means a helper is being called with the wrong syntax. Check the Template Syntax Help section.`;
    }
    if (error.includes("not defined") || error.includes("unknown helper")) {
      return `${error}. Check that the helper name is spelled correctly. See Available Helpers in the help section.`;
    }
    return error;
  };

  // Render template for preview — we do this ourselves so we can show errors and empty output
  const previewResult = useMemo(() => {
    if (!localValue.trim()) return null;
    if (!validation.valid) return null;
    const templateContext: Record<string, unknown> = {
      ...context,
      custom_metadata: context.customMetadata,
    };
    return renderTemplate(localValue, templateContext);
  }, [localValue, context, validation.valid]);

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
        ) : previewResult && !previewResult.success ? (
          <Alert
            icon={<IconAlertCircle size={16} />}
            color="red"
            title="Render Error"
          >
            {describeRenderError(
              previewResult.error || "Failed to render template",
            )}
          </Alert>
        ) : previewResult?.success && !previewResult.output.trim() ? (
          <Text size="sm" c="dimmed" fs="italic">
            Template rendered but produced empty output
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
                Available Helpers{" "}
                <Text component="span" size="xs" c="dimmed" fw={400}>
                  (click for example)
                </Text>
              </Text>
              <Group gap={4}>
                {helpers.map((helper) => {
                  const isSelected = selectedHelper === helper;
                  return (
                    <Text
                      key={helper}
                      size="xs"
                      onClick={() =>
                        setSelectedHelper(isSelected ? null : helper)
                      }
                      style={{
                        fontFamily: "monospace",
                        backgroundColor: isSelected
                          ? colorScheme === "dark"
                            ? "var(--mantine-color-blue-9)"
                            : "var(--mantine-color-blue-1)"
                          : colorScheme === "dark"
                            ? "var(--mantine-color-dark-6)"
                            : "var(--mantine-color-gray-2)",
                        color: isSelected
                          ? colorScheme === "dark"
                            ? "var(--mantine-color-blue-2)"
                            : "var(--mantine-color-blue-7)"
                          : undefined,
                        padding: "3px 8px",
                        borderRadius: 4,
                        cursor: "pointer",
                        userSelect: "none",
                        transition: "background-color 0.15s",
                      }}
                    >
                      {helper}
                    </Text>
                  );
                })}
              </Group>
              {selectedHelper && helperReference[selectedHelper] && (
                <Box
                  mt="xs"
                  p="xs"
                  style={{
                    backgroundColor:
                      colorScheme === "dark"
                        ? "var(--mantine-color-dark-6)"
                        : "var(--mantine-color-gray-1)",
                    borderRadius: "var(--mantine-radius-sm)",
                    border:
                      colorScheme === "dark"
                        ? "1px solid var(--mantine-color-dark-4)"
                        : "1px solid var(--mantine-color-gray-3)",
                  }}
                >
                  <Text size="xs" c="dimmed" mb={4}>
                    {helperReference[selectedHelper].desc}
                  </Text>
                  <Text
                    size="xs"
                    style={{
                      fontFamily: "monospace",
                      wordBreak: "break-all",
                    }}
                  >
                    {helperReference[selectedHelper].example}
                  </Text>
                </Box>
              )}
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Available Variables
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <p style={{ margin: "0 0 8px 0" }}>
                  Templates receive a context object. Use the <code>type</code>{" "}
                  field (<code>&quot;series&quot;</code> or{" "}
                  <code>&quot;book&quot;</code>) to branch between them:
                </p>
                <p
                  style={{
                    margin: "0 0 8px 0",
                    fontWeight: 500,
                    color: "var(--mantine-color-text)",
                  }}
                >
                  Both contexts:
                </p>
                <ul style={{ margin: "0 0 8px 0", paddingLeft: 20 }}>
                  <li>
                    <code>type</code> - <code>&quot;series&quot;</code> or{" "}
                    <code>&quot;book&quot;</code>
                  </li>
                  <li>
                    <code>metadata.*</code> - Built-in metadata fields
                  </li>
                  <li>
                    <code>externalIds.*</code> - External IDs from plugins
                  </li>
                  <li>
                    <code>customMetadata.*</code> - User-defined custom fields
                  </li>
                </ul>
                <p
                  style={{
                    margin: "0 0 8px 0",
                    fontWeight: 500,
                    color: "var(--mantine-color-text)",
                  }}
                >
                  Series-only:
                </p>
                <ul style={{ margin: "0 0 8px 0", paddingLeft: 20 }}>
                  <li>
                    <code>seriesId</code>, <code>bookCount</code>
                  </li>
                </ul>
                <p
                  style={{
                    margin: "0 0 8px 0",
                    fontWeight: 500,
                    color: "var(--mantine-color-text)",
                  }}
                >
                  Book-only:
                </p>
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>bookId</code>, <code>seriesId</code>,{" "}
                    <code>libraryId</code>
                  </li>
                  <li>
                    <code>fileFormat</code>, <code>pageCount</code>,{" "}
                    <code>fileSize</code>
                  </li>
                  <li>
                    <code>series.*</code> - Parent series context (e.g.,{" "}
                    <code>series.metadata.title</code>)
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Series Metadata Fields
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
                    <code>metadata.status</code>,{" "}
                    <code>metadata.readingDirection</code> - Series status and
                    layout
                  </li>
                  <li>
                    <code>metadata.genres</code>, <code>metadata.tags</code> -
                    Arrays of strings
                  </li>
                  <li>
                    <code>metadata.authors</code>,{" "}
                    <code>metadata.alternateTitles</code>,{" "}
                    <code>metadata.externalRatings</code> - Structured arrays
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Book Metadata Fields
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>metadata.title</code>, <code>metadata.subtitle</code>,{" "}
                    <code>metadata.summary</code> - Basic info
                  </li>
                  <li>
                    <code>metadata.authors</code>,{" "}
                    <code>metadata.publisher</code>,{" "}
                    <code>metadata.translator</code> - Credits
                  </li>
                  <li>
                    <code>metadata.year</code>, <code>metadata.month</code>,{" "}
                    <code>metadata.day</code> - Publication date
                  </li>
                  <li>
                    <code>metadata.isbns</code>, <code>metadata.edition</code>,{" "}
                    <code>metadata.bookType</code> - Identification
                  </li>
                  <li>
                    <code>metadata.awards</code> - Array of awards (name, year,
                    category, won)
                  </li>
                  <li>
                    <code>metadata.genres</code>, <code>metadata.tags</code>,{" "}
                    <code>metadata.subjects</code> - Classification
                  </li>
                </ul>
              </Text>
            </Box>

            <Box>
              <Text size="sm" fw={500} mb="xs">
                Type-Aware Templates
              </Text>
              <Text size="xs" c="dimmed" component="div">
                <ul style={{ margin: 0, paddingLeft: 20 }}>
                  <li>
                    <code>{'{{#ifEquals type "book"}}...{{/ifEquals}}'}</code> -
                    Render only for books
                  </li>
                  <li>
                    <code>{'{{#ifEquals type "series"}}...{{/ifEquals}}'}</code>{" "}
                    - Render only for series
                  </li>
                  <li>
                    <code>{"{{externalIds.plugin:source.id}}"}</code> - External
                    ID value
                  </li>
                  <li>
                    <code>{"{{externalIds.plugin:source.url}}"}</code> -
                    External URL
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
