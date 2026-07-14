import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  rectSortingStrategy,
  SortableContext,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ActionIcon, Group, Skeleton, Stack, Tooltip } from "@mantine/core";
import { IconChevronDown, IconChevronUp, IconX } from "@tabler/icons-react";
import { type ReactNode, useEffect, useState } from "react";
import { MediaCard } from "@/components/library/MediaCard";
import type { Book, Series } from "@/types";

export interface MediaGridItem {
  id: string;
  type: "book" | "series";
  /** Undefined while the item's payload is still loading (renders a skeleton). */
  data?: Book | Series;
}

interface MediaGridProps {
  items: MediaGridItem[];
  /** Whole-grid loading state (renders skeleton cards). */
  loading?: boolean;
  /** Remove affordance under each card. Omit to hide the control. */
  onRemove?: (item: MediaGridItem) => void;
  /** Tooltip / aria label for the remove control. */
  removeLabel?: string;
  /** Id of the item currently being removed (drives its spinner). */
  removingId?: string;
  /**
   * Manual reorder: drag & drop on the cards plus up/down chevrons as the
   * keyboard/touch fallback. Requires `onReorder`.
   */
  reorderable?: boolean;
  /** Called with the full id list in its new order. */
  onReorder?: (ids: string[]) => void;
  reorderPending?: boolean;
}

/**
 * The shared card grid for curated lists (collections, read lists, want to
 * read). Uses the same fluid auto-fill layout as the library browse so the
 * pages read as one app, and centralizes the remove/reorder affordances that
 * each page previously hand-rolled.
 */
export function MediaGrid({
  items,
  loading = false,
  onRemove,
  removeLabel = "Remove",
  removingId,
  reorderable = false,
  onReorder,
  reorderPending = false,
}: MediaGridProps) {
  // Optimistic order: drops apply immediately and the server round-trip
  // (mutation + query invalidation) catches up via the items prop.
  const [orderedItems, setOrderedItems] = useState(items);
  const [dragging, setDragging] = useState(false);
  useEffect(() => setOrderedItems(items), [items]);

  // The distance threshold keeps plain clicks navigating to the detail page;
  // only an actual 8px drag starts a sort.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
  );

  if (loading) {
    return (
      <div style={gridStyle}>
        {Array.from({ length: 6 }).map((_, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: static skeletons
          <Skeleton key={i} height={300} radius="md" />
        ))}
      </div>
    );
  }

  const applyOrder = (next: MediaGridItem[]) => {
    setOrderedItems(next);
    onReorder?.(next.map((item) => item.id));
  };

  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= orderedItems.length) return;
    applyOrder(arrayMove(orderedItems, index, target));
  };

  const handleDragEnd = (event: DragEndEvent) => {
    setDragging(false);
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = orderedItems.findIndex((item) => item.id === active.id);
    const newIndex = orderedItems.findIndex((item) => item.id === over.id);
    if (oldIndex < 0 || newIndex < 0) return;
    applyOrder(arrayMove(orderedItems, oldIndex, newIndex));
  };

  const renderItem = (item: MediaGridItem, index: number) => (
    <Stack gap={4}>
      {item.data ? (
        <MediaCard
          type={item.type}
          data={item.data}
          disableHoverPanel={dragging}
        />
      ) : (
        <Skeleton height={300} radius="md" />
      )}
      {(reorderable || onRemove) && (
        <Group gap={4} justify="center">
          {reorderable && (
            <>
              <Tooltip label="Move up">
                <ActionIcon
                  variant="subtle"
                  size="sm"
                  disabled={index === 0 || reorderPending}
                  onClick={() => move(index, -1)}
                  aria-label="Move up"
                >
                  <IconChevronUp size={16} />
                </ActionIcon>
              </Tooltip>
              <Tooltip label="Move down">
                <ActionIcon
                  variant="subtle"
                  size="sm"
                  disabled={index === orderedItems.length - 1 || reorderPending}
                  onClick={() => move(index, 1)}
                  aria-label="Move down"
                >
                  <IconChevronDown size={16} />
                </ActionIcon>
              </Tooltip>
            </>
          )}
          {onRemove && (
            <Tooltip label={removeLabel}>
              <ActionIcon
                variant="subtle"
                color="red"
                size="sm"
                loading={removingId === item.id}
                onClick={() => onRemove(item)}
                aria-label={removeLabel}
              >
                <IconX size={16} />
              </ActionIcon>
            </Tooltip>
          )}
        </Group>
      )}
    </Stack>
  );

  if (!reorderable) {
    return (
      <div style={gridStyle}>
        {orderedItems.map((item, index) => (
          <div key={item.id}>{renderItem(item, index)}</div>
        ))}
      </div>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={() => setDragging(true)}
      onDragCancel={() => setDragging(false)}
      onDragEnd={handleDragEnd}
    >
      <SortableContext
        items={orderedItems.map((item) => item.id)}
        strategy={rectSortingStrategy}
      >
        <div style={gridStyle}>
          {orderedItems.map((item, index) => (
            <SortableGridItem key={item.id} id={item.id} dragging={dragging}>
              {renderItem(item, index)}
            </SortableGridItem>
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}

// Same fluid layout as the library browse grid (SeriesSection).
const gridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
  gap: "var(--mantine-spacing-md)",
  width: "100%",
};

function SortableGridItem({
  id,
  dragging,
  children,
}: {
  id: string;
  dragging: boolean;
  children: ReactNode;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        zIndex: isDragging ? 2 : undefined,
        // While a card is mid-drag, the others must not swallow pointer
        // events (or trigger their own hover panels) under the moving card.
        pointerEvents: dragging && !isDragging ? "none" : undefined,
      }}
      {...attributes}
      {...listeners}
    >
      {children}
    </div>
  );
}
