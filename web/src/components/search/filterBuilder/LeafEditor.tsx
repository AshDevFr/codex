import {
  ActionIcon,
  Group,
  NumberInput,
  Select,
  Stack,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconTrash } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { librariesApi } from "@/api/libraries";
import type {
  DateOperator,
  FieldOperator,
  NumberOperator,
  UuidOperator,
} from "@/types/filters";
import {
  type Condition,
  newLeaf,
  OPERATOR_LABELS,
  operatorsForField,
  updateLeafOperator,
  updateLeafValue,
} from "./conditionUtils";
import {
  type EnumOption,
  type FieldDef,
  type FieldTarget,
  fieldPickerGroups,
  findFieldAnyTarget,
} from "./fieldCatalog";

interface LeafEditorProps {
  /** The leaf condition being edited. */
  condition: Condition;
  /** Active target — determines which fields are pickable. */
  target: FieldTarget;
  onChange: (next: Condition) => void;
  onRemove: () => void;
  /** Field key on the leaf. */
  fieldKey: string;
}

/**
 * Render a single leaf row: field picker · operator picker · value input ·
 * delete button. Stays tightly coupled to the catalog so it can adapt the
 * value input to the field's operator family.
 */
export function LeafEditor({
  condition,
  target,
  onChange,
  onRemove,
  fieldKey,
}: LeafEditorProps) {
  const pickerGroups = useMemo(() => fieldPickerGroups(target), [target]);
  const field = findFieldAnyTarget(fieldKey);
  // Below the drawer/sheet breakpoint the three controls + delete button can't
  // share one row without crushing the value input to an unusable width, so we
  // stack them vertically and let each control fill the row instead.
  const isMobile = useMediaQuery("(max-width: 768px)");

  const removeButton = (
    <Tooltip label="Remove">
      <ActionIcon
        variant="subtle"
        color="red"
        onClick={onRemove}
        aria-label="Remove filter"
      >
        <IconTrash size={14} />
      </ActionIcon>
    </Tooltip>
  );

  // Field truly missing (malformed condition): fall back to a bare picker so
  // the user can either pick something or remove the row.
  if (!field) {
    return (
      <Group gap="xs" wrap="nowrap">
        <Select
          value={null}
          placeholder={`Unknown field: ${fieldKey}`}
          data={pickerGroups}
          onChange={(nextKey) => {
            if (nextKey) {
              const nextField = findFieldAnyTarget(nextKey);
              if (nextField) onChange(newLeaf(nextField));
            }
          }}
          w={isMobile ? undefined : 220}
          flex={isMobile ? 1 : undefined}
        />
        {removeButton}
      </Group>
    );
  }

  const op = (condition as Record<string, { operator: string }>)[fieldKey]
    .operator;
  const ops = operatorsForField(field);
  const opLabels = OPERATOR_LABELS[field.operatorType];

  // Leaf is for the other tab: render the editor as usual but flag that the
  // current /list query will skip it. The prune-on-emit logic does the drop;
  // this note tells the user why their filter looks like it's doing nothing.
  const appliesToActiveTab = field.targets.includes(target);
  const ignoredFor = appliesToActiveTab
    ? null
    : target === "series"
      ? "Series"
      : "Books";

  const fieldSelect = (
    <Select
      value={field.key}
      data={pickerGroups}
      onChange={(nextKey) => {
        if (nextKey && nextKey !== field.key) {
          const nextField = findFieldAnyTarget(nextKey);
          if (nextField) {
            // Caller (parent) replaces the entire leaf; we do that here
            // by emitting a fresh leaf shape.
            const fresh = makeFreshLeaf(nextField);
            onChange(fresh);
          }
        }
      }}
      searchable
      w={isMobile ? undefined : 180}
      flex={isMobile ? 1 : undefined}
    />
  );

  const operatorSelect = (
    <Select
      value={op}
      data={ops.map((value) => ({ value, label: opLabels[value] ?? value }))}
      onChange={(nextOp) => {
        if (nextOp && nextOp !== op) {
          onChange(updateLeafOperator(condition, field, nextOp));
        }
      }}
      w={isMobile ? "100%" : 150}
    />
  );

  const valueInput = (
    <ValueInput
      condition={condition}
      field={field}
      operator={op}
      onChange={onChange}
      fullWidth={isMobile}
    />
  );

  // Mobile: stack the controls so the value input gets the full row width.
  // The field picker shares its row with the delete button to keep the
  // remove affordance visible without a third column squeezing everything.
  const row = isMobile ? (
    <Stack gap="xs">
      <Group gap="xs" wrap="nowrap" align="center">
        {fieldSelect}
        {removeButton}
      </Group>
      {operatorSelect}
      {valueInput}
    </Stack>
  ) : (
    <Group gap="xs" wrap="nowrap" align="flex-start">
      {fieldSelect}
      {operatorSelect}
      {valueInput}
      {removeButton}
    </Group>
  );

  if (!ignoredFor) return row;

  return (
    <Stack gap={2}>
      {row}
      <Text size="xs" c="dimmed" pl={4}>
        Will be ignored for {ignoredFor} — switch tabs to apply.
      </Text>
    </Stack>
  );
}

interface ValueInputProps {
  condition: Condition;
  field: FieldDef;
  operator: string;
  onChange: (next: Condition) => void;
  /** Stretch the input(s) to fill the row (mobile stacked layout). */
  fullWidth?: boolean;
}

function ValueInput({
  condition,
  field,
  operator,
  onChange,
  fullWidth,
}: ValueInputProps) {
  // Operators without a value: render nothing (the operator label itself
  // carries the meaning).
  if (
    operator === "isNull" ||
    operator === "isNotNull" ||
    operator === "isTrue" ||
    operator === "isFalse"
  ) {
    return null;
  }

  if (field.operatorType === "field") {
    const node = (condition as Record<string, FieldOperator>)[field.key];
    const value = "value" in node ? node.value : "";
    if (field.enumValues) {
      return (
        <EnumSelect
          options={field.enumValues}
          value={value}
          onChange={(next) =>
            onChange(updateLeafValue(condition, field, { value: next }))
          }
          fullWidth={fullWidth}
        />
      );
    }
    return (
      <TextInput
        value={value}
        onChange={(e) =>
          onChange(
            updateLeafValue(condition, field, { value: e.currentTarget.value }),
          )
        }
        placeholder={field.hint === "path" ? "/path/fragment" : "value"}
        w={fullWidth ? "100%" : undefined}
        flex={fullWidth ? undefined : 1}
      />
    );
  }

  if (field.operatorType === "uuid") {
    return (
      <UuidValueInput
        condition={condition}
        field={field}
        onChange={onChange}
        fullWidth={fullWidth}
      />
    );
  }

  if (field.operatorType === "number") {
    if (operator === "between") {
      const node = (condition as Record<string, NumberOperator>)[
        field.key
      ] as Extract<NumberOperator, { operator: "between" }>;
      return (
        <Group
          gap="xs"
          wrap="nowrap"
          grow={fullWidth}
          w={fullWidth ? "100%" : undefined}
        >
          <NumberInput
            value={node.min ?? ""}
            onChange={(v) =>
              onChange(
                updateLeafValue(condition, field, {
                  min: typeof v === "number" ? v : null,
                }),
              )
            }
            placeholder="min"
            w={fullWidth ? undefined : 100}
          />
          <NumberInput
            value={node.max ?? ""}
            onChange={(v) =>
              onChange(
                updateLeafValue(condition, field, {
                  max: typeof v === "number" ? v : null,
                }),
              )
            }
            placeholder="max"
            w={fullWidth ? undefined : 100}
          />
        </Group>
      );
    }
    const node = (condition as Record<string, NumberOperator>)[
      field.key
    ] as Extract<NumberOperator, { value: number }>;
    return (
      <NumberInput
        value={node.value}
        onChange={(v) =>
          onChange(
            updateLeafValue(condition, field, {
              value: typeof v === "number" ? v : 0,
            }),
          )
        }
        placeholder={field.hint === "year" ? "YYYY" : "value"}
        w={fullWidth ? "100%" : 120}
      />
    );
  }

  if (field.operatorType === "date") {
    if (operator === "between") {
      const node = (condition as Record<string, DateOperator>)[
        field.key
      ] as Extract<DateOperator, { operator: "between" }>;
      return (
        <Group
          gap="xs"
          wrap="nowrap"
          grow={fullWidth}
          w={fullWidth ? "100%" : undefined}
        >
          <DateLocalInput
            value={node.start ?? null}
            onChange={(next) =>
              onChange(updateLeafValue(condition, field, { start: next }))
            }
            placeholder="from"
            fullWidth={fullWidth}
          />
          <DateLocalInput
            value={node.end ?? null}
            onChange={(next) =>
              onChange(updateLeafValue(condition, field, { end: next }))
            }
            placeholder="to"
            fullWidth={fullWidth}
          />
        </Group>
      );
    }
    const node = (condition as Record<string, DateOperator>)[
      field.key
    ] as Extract<DateOperator, { value: string }>;
    return (
      <DateLocalInput
        value={node.value}
        onChange={(next) =>
          onChange(updateLeafValue(condition, field, { value: next ?? "" }))
        }
        fullWidth={fullWidth}
      />
    );
  }

  return null;
}

function EnumSelect({
  options,
  value,
  onChange,
  fullWidth,
}: {
  options: EnumOption[];
  value: string;
  onChange: (next: string) => void;
  fullWidth?: boolean;
}) {
  return (
    <Select
      value={value}
      data={options}
      onChange={(next) => {
        if (next) onChange(next);
      }}
      w={fullWidth ? "100%" : 180}
    />
  );
}

function UuidValueInput({
  condition,
  field,
  onChange,
  fullWidth,
}: {
  condition: Condition;
  field: FieldDef;
  onChange: (next: Condition) => void;
  fullWidth?: boolean;
}) {
  // Only `libraryId` currently has a curated picker; `seriesId` falls back
  // to a free-text input until we wire a series autocomplete.
  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: () => librariesApi.getAll(),
    enabled: field.key === "libraryId",
    staleTime: 5 * 60 * 1000,
  });

  const node = (condition as Record<string, UuidOperator>)[field.key];
  const value = "value" in node ? node.value : "";

  if (field.key === "libraryId" && libraries) {
    return (
      <Select
        value={value || null}
        data={libraries.map((l) => ({ value: l.id, label: l.name }))}
        onChange={(next) => {
          if (next) {
            onChange(updateLeafValue(condition, field, { value: next }));
          }
        }}
        placeholder="Pick a library"
        searchable
        w={fullWidth ? "100%" : 220}
      />
    );
  }

  return (
    <TextInput
      value={value}
      onChange={(e) =>
        onChange(
          updateLeafValue(condition, field, { value: e.currentTarget.value }),
        )
      }
      placeholder="uuid"
      w={fullWidth ? "100%" : undefined}
      flex={fullWidth ? undefined : 1}
    />
  );
}

/**
 * Plain HTML datetime-local input wrapped in Mantine styles. We avoid
 * pulling `@mantine/dates` for the sake of one input. ISO-string in/out so
 * the value plugs straight into the API DTOs.
 */
function DateLocalInput({
  value,
  onChange,
  placeholder,
  fullWidth,
}: {
  value: string | null;
  onChange: (next: string | null) => void;
  placeholder?: string;
  fullWidth?: boolean;
}) {
  const localValue = value ? isoToLocalInput(value) : "";
  return (
    <TextInput
      type="datetime-local"
      value={localValue}
      onChange={(e) => {
        const raw = e.currentTarget.value;
        onChange(raw ? localInputToIso(raw) : null);
      }}
      placeholder={placeholder}
      w={fullWidth ? "100%" : 200}
    />
  );
}

function isoToLocalInput(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function localInputToIso(local: string): string {
  return new Date(local).toISOString();
}

function makeFreshLeaf(field: FieldDef): Condition {
  return newLeaf(field);
}
