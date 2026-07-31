import {
  type CollisionDetection,
  closestCorners,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragStartEvent,
  KeyboardSensor,
  PointerSensor,
  pointerWithin,
  TouchSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
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
  IconGripVertical,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";
import { type ReactNode, useMemo, useState } from "react";
import {
  appendChildAtPath,
  applyDragMove,
  asGroup,
  type Condition,
  conditionAtPath,
  dragId,
  dragIdParentKey,
  dragIdToPath,
  ensureRoot,
  isGroup,
  leafFieldKey,
  makeGroup,
  newLeaf,
  parsePathKey,
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

  const sensors = useSensors(
    // A small threshold so clicking a select inside a row never reads as a drag.
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    // On touch the builder lives inside a scrollable modal, so a drag has to
    // be a deliberate press-and-hold or vertical scrolling would break.
    useSensor(TouchSensor, {
      activationConstraint: { delay: 200, tolerance: 5 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const emitRoot = (next: Condition) => {
    const group = asGroup(next);
    if (group && group.children.length === 0) {
      onChange(undefined);
      return;
    }
    onChange(next);
  };

  const [activeId, setActiveId] = useState<string | null>(null);

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    setActiveId(null);
    if (!over) return;
    const next = applyDragMove(root, String(active.id), String(over.id));
    if (next) emitRoot(next);
  };

  // The row travelling with the cursor is rendered into the drag overlay, so
  // resolve it back out of the tree from the id being dragged.
  const activePath = activeId ? dragIdToPath(activeId) : null;
  const activeCondition = activePath ? conditionAtPath(root, activePath) : null;

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={siblingCollisionDetection}
      onDragStart={({ active }: DragStartEvent) =>
        setActiveId(String(active.id))
      }
      onDragEnd={handleDragEnd}
      onDragCancel={() => setActiveId(null)}
    >
      <GroupNodeView
        condition={root}
        path={[]}
        target={target}
        depth={0}
        onChange={emitRoot}
      />
      {/* No drop animation: dnd-kit flies the overlay to the dragged node's
          rect, but rows are keyed by index so that node never moves. The
          overlay would glide back to the slot the row was picked up from
          while the list below already shows the new order, which reads as
          the move reverting and then happening again. */}
      <DragOverlay dropAnimation={null}>
        {activeCondition && activePath ? (
          <DragPreviewRow>
            {/* Inert copy: the overlay renders inside a nullified dnd context,
                so nothing here registers as a droppable. */}
            <ChildRow
              child={activeCondition}
              target={target}
              depth={activePath.length - 1}
              path={[]}
              onChange={noop}
              onRemove={noop}
            />
          </DragPreviewRow>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

function noop() {}

/**
 * Only siblings are valid drop targets, so narrow the candidates before
 * measuring rather than rejecting a bad drop afterwards. That keeps the
 * preview honest: the gap that opens is always where the row will land.
 *
 * Two things go wrong without it. A nested group's own rows are droppables
 * too, so dragging a root row over a group's body would resolve to a row
 * inside it and the drop would be silently discarded. And a group card is
 * several times taller than a filter row, so measuring by centre (the
 * `closestCenter` default) lands the drop a slot past the pointer, since a
 * tall card grabbed by its grip has its centre well below the cursor.
 * Pointer position decides while it is over a sibling, and corner distance
 * takes over in the gaps between rows, where it isn't over anything.
 */
const siblingCollisionDetection: CollisionDetection = (args) => {
  const activeParent = dragIdParentKey(String(args.active.id));
  const siblings = args.droppableContainers.filter(
    (container) => dragIdParentKey(String(container.id)) === activeParent,
  );
  const scoped = { ...args, droppableContainers: siblings };
  const underPointer = pointerWithin(scoped);
  return underPointer.length > 0 ? underPointer : closestCorners(scoped);
};

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

  // Computed before the early return to keep hook order stable. dnd-kit
  // compares the `items` array by reference to decide whether the list
  // changed, so rebuilding it on every render would leave it permanently
  // convinced it had.
  const pathKey = path.join(".");
  const childCount = group?.children.length ?? 0;
  const childIds = useMemo(
    () =>
      Array.from({ length: childCount }, (_, idx) =>
        dragId(parsePathKey(pathKey), idx),
      ),
    [childCount, pathKey],
  );

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
  // Nothing to reorder in a one-row group. The gutter still renders so rows
  // stay aligned with sibling groups; only the grip itself is hidden.
  const canReorder = group.children.length > 1;

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

      <SortableContext items={childIds} strategy={verticalListSortingStrategy}>
        {group.children.map((child, idx) => (
          <SortableRow
            // biome-ignore lint/suspicious/noArrayIndexKey: position is the identity in the tree
            key={idx}
            id={childIds[idx]}
            canReorder={canReorder}
            label={isGroup(child) ? "Reorder group" : "Reorder filter"}
          >
            <ChildRow
              child={child}
              target={target}
              depth={depth}
              path={[...path, idx]}
              onChange={(next) => replaceChild(idx, next)}
              onRemove={() => removeChild(idx)}
            />
          </SortableRow>
        ))}
      </SortableContext>

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
  // "title" is now a shared field that works on both targets, so the
  // default-field picker no longer needs target-specific logic.
  return findField(target, "title") ?? fields[0];
}

interface SortableRowProps {
  id: string;
  canReorder: boolean;
  /** Accessible name for the grip, e.g. "Reorder filter". */
  label: string;
  children: ReactNode;
}

/**
 * Wraps a sibling (leaf row or nested group card) in a drag gutter. Wrapping
 * at this level means both kinds of sibling share one grip column and the
 * LeafEditor stays unaware of dragging entirely.
 */
function SortableRow({ id, canReorder, label, children }: SortableRowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    disabled: !canReorder,
    // Sortable ids encode position, so a row's id never travels with it and
    // the DOM node it lives in never actually moves. dnd-kit's layout
    // animation measures a move that didn't happen, so opt out of it.
    animateLayoutChanges: () => false,
  });

  return (
    <Group
      ref={setNodeRef}
      gap="xs"
      wrap="nowrap"
      align="flex-start"
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        // The row travels with the cursor in the drag overlay, so the original
        // gives up its space here. Keeping it visible would show the same row
        // twice; displacing it instead is what made the drop read as a revert.
        opacity: isDragging ? 0 : undefined,
      }}
    >
      <ActionIcon
        ref={setActivatorNodeRef}
        variant="subtle"
        color="gray"
        size="sm"
        mt={4}
        aria-label={label}
        title={canReorder ? "Drag to reorder" : undefined}
        style={{
          cursor: canReorder ? "grab" : "default",
          // Without this the browser claims the gesture for scrolling and the
          // drag never starts on touch.
          touchAction: "none",
          visibility: canReorder ? undefined : "hidden",
        }}
        {...attributes}
        {...listeners}
      >
        <IconGripVertical size={14} />
      </ActionIcon>
      <Box style={{ flex: 1, minWidth: 0 }}>{children}</Box>
    </Group>
  );
}

/**
 * The row travelling with the cursor. Mirrors SortableRow's gutter so the
 * preview sits exactly where the original was picked up, and renders the real
 * ChildRow rather than a summary so it looks like the row itself moving.
 */
function DragPreviewRow({ children }: { children: ReactNode }) {
  return (
    <Group
      gap="xs"
      wrap="nowrap"
      align="flex-start"
      style={{ cursor: "grabbing" }}
    >
      <ActionIcon
        component="div"
        variant="subtle"
        color="gray"
        size="sm"
        mt={4}
        aria-hidden
        tabIndex={-1}
      >
        <IconGripVertical size={14} />
      </ActionIcon>
      <Box style={{ flex: 1, minWidth: 0 }}>{children}</Box>
    </Group>
  );
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
