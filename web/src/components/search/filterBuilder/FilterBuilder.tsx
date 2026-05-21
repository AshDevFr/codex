import {
  ActionIcon,
  Box,
  Button,
  Card,
  Group,
  Menu,
  SegmentedControl,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconChevronDown,
  IconFolderPlus,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";
import { useMemo } from "react";
import {
  appendChildAtPath,
  asGroup,
  type Condition,
  ensureRoot,
  isGroup,
  leafFieldKey,
  makeGroup,
  newLeaf,
  removeAtPath,
  replaceAtPath,
} from "./conditionUtils";
import {
  type FieldDef,
  type FieldTarget,
  fieldsForTarget,
  findField,
} from "./fieldCatalog";
import { LeafEditor } from "./LeafEditor";

interface FilterBuilderProps {
  condition: Condition | undefined;
  target: FieldTarget;
  onChange: (next: Condition | undefined) => void;
}

/**
 * Top-level filter builder. Always works against a normalized root group
 * (`allOf` by default). Emits `undefined` when the user empties the group,
 * so the SearchPage can treat it the same as "no condition".
 */
export function FilterBuilder({
  condition,
  target,
  onChange,
}: FilterBuilderProps) {
  const root = useMemo(() => ensureRoot(condition), [condition]);

  const emitRoot = (next: Condition) => {
    const group = asGroup(next);
    if (group && group.children.length === 0) {
      onChange(undefined);
      return;
    }
    onChange(next);
  };

  return (
    <GroupNodeView
      condition={root}
      path={[]}
      target={target}
      depth={0}
      onChange={emitRoot}
    />
  );
}

interface GroupNodeViewProps {
  condition: Condition;
  path: number[];
  target: FieldTarget;
  depth: number;
  onChange: (next: Condition) => void;
}

function GroupNodeView({
  condition,
  path,
  target,
  depth,
  onChange,
}: GroupNodeViewProps) {
  const group = asGroup(condition);
  if (!group) return null;

  const fields = fieldsForTarget(target);
  const defaultField = pickDefaultField(target, fields);

  const updateMode = (mode: "allOf" | "anyOf") => {
    onChange(makeGroup({ mode, children: group.children }));
  };

  const replaceChild = (index: number, next: Condition) => {
    onChange(replaceAtPath(condition, [index], next));
  };

  const removeChild = (index: number) => {
    onChange(removeAtPath(condition, [index]));
  };

  const addLeaf = () => {
    if (!defaultField) return;
    onChange(appendChildAtPath(condition, [], newLeaf(defaultField)));
  };

  const addGroup = (mode: "allOf" | "anyOf") => {
    onChange(
      appendChildAtPath(condition, [], makeGroup({ mode, children: [] })),
    );
  };

  const isRoot = depth === 0;

  const inner = (
    <Stack gap="xs">
      {!isRoot && (
        <Group justify="space-between" align="center" gap="xs">
          <Group gap="xs">
            <Text size="xs" fw={600} c="dimmed">
              {group.mode === "allOf" ? "MATCH ALL" : "MATCH ANY"}
            </Text>
            <SegmentedControl
              size="xs"
              value={group.mode}
              onChange={(value) => updateMode(value as "allOf" | "anyOf")}
              data={[
                { label: "All", value: "allOf" },
                { label: "Any", value: "anyOf" },
              ]}
            />
          </Group>
        </Group>
      )}

      {isRoot && (
        <Stack gap={4}>
          <Group justify="space-between" align="center" gap="xs">
            <Text size="sm" fw={600}>
              Match
            </Text>
            <SegmentedControl
              size="xs"
              value={group.mode}
              onChange={(value) => updateMode(value as "allOf" | "anyOf")}
              data={[
                { label: "All of", value: "allOf" },
                { label: "Any of", value: "anyOf" },
              ]}
            />
          </Group>
          <Text size="xs" c="dimmed">
            Filters under "Series only" or "Books only" apply on that tab only.
            Cross-tab rows stay visible and get a small note saying they'll be
            ignored on the current tab.
          </Text>
        </Stack>
      )}

      {group.children.length === 0 && (
        <Text size="sm" c="dimmed" fs="italic">
          No filters yet. Add a row below.
        </Text>
      )}

      {group.children.map((child, idx) => (
        <ChildRow
          // biome-ignore lint/suspicious/noArrayIndexKey: position is the identity in the tree
          key={idx}
          child={child}
          target={target}
          depth={depth}
          path={[...path, idx]}
          onChange={(next) => replaceChild(idx, next)}
          onRemove={() => removeChild(idx)}
        />
      ))}

      <Group gap="xs">
        <Button
          size="xs"
          variant="light"
          leftSection={<IconPlus size={12} />}
          onClick={addLeaf}
        >
          Add filter
        </Button>
        <Menu shadow="md" position="bottom-start">
          <Menu.Target>
            <Button
              size="xs"
              variant="subtle"
              leftSection={<IconFolderPlus size={12} />}
              rightSection={<IconChevronDown size={12} />}
            >
              Add group
            </Button>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Item onClick={() => addGroup("allOf")}>
              Match all of (AND)
            </Menu.Item>
            <Menu.Item onClick={() => addGroup("anyOf")}>
              Match any of (OR)
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </Group>
    </Stack>
  );

  if (isRoot) {
    return inner;
  }

  return (
    <Card withBorder p="sm" radius="sm">
      {inner}
    </Card>
  );
}

// Land the user on a text field by default so a freshly-added filter doesn't
// emit an invalid UUID and trip a 4xx round-trip.
function pickDefaultField(
  target: FieldTarget,
  fields: FieldDef[],
): FieldDef | undefined {
  const preferredKey = target === "series" ? "name" : "title";
  return findField(target, preferredKey) ?? fields[0];
}

interface ChildRowProps {
  child: Condition;
  target: FieldTarget;
  depth: number;
  path: number[];
  onChange: (next: Condition) => void;
  onRemove: () => void;
}

function ChildRow({
  child,
  target,
  depth,
  path,
  onChange,
  onRemove,
}: ChildRowProps) {
  if (isGroup(child)) {
    return (
      <Box style={{ position: "relative" }}>
        <GroupNodeView
          condition={child}
          path={path}
          target={target}
          depth={depth + 1}
          onChange={onChange}
        />
        <Tooltip label="Remove group">
          <ActionIcon
            variant="subtle"
            color="red"
            size="sm"
            onClick={onRemove}
            style={{ position: "absolute", top: 6, right: 6 }}
            aria-label="Remove group"
          >
            <IconTrash size={14} />
          </ActionIcon>
        </Tooltip>
      </Box>
    );
  }

  const key = leafFieldKey(child) ?? "";
  return (
    <LeafEditor
      condition={child}
      target={target}
      fieldKey={key}
      onChange={onChange}
      onRemove={onRemove}
    />
  );
}
