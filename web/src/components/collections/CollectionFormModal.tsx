import {
  Alert,
  Button,
  Checkbox,
  Group,
  Modal,
  SegmentedControl,
  Stack,
  Text,
  Textarea,
  TextInput,
} from "@mantine/core";
import { IconInfoCircle } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import type { Collection } from "@/api/collections";
import {
  type Condition,
  ensureRoot,
  normalizeForEmit,
} from "@/components/search/filterBuilder/conditionUtils";
import { FilterBuilder } from "@/components/search/filterBuilder/FilterBuilder";
import {
  useCreateCollection,
  useUpdateCollection,
} from "@/hooks/useCollections";
import type { SeriesCondition } from "@/types/filters";
import { describesPersonalData } from "@/utils/collectionRules";

interface CollectionFormModalProps {
  opened: boolean;
  onClose: () => void;
  /** When provided, the modal edits this collection instead of creating one. */
  collection?: Collection;
  /** Called with the created collection (create mode only). */
  onCreated?: (collection: Collection) => void;
  /** Pre-seed an automatic collection with this rule (e.g. copied from a preset). */
  initialCondition?: SeriesCondition;
}

type Mode = "manual" | "automatic";

export function CollectionFormModal({
  opened,
  onClose,
  collection,
  onCreated,
  initialCondition,
}: CollectionFormModalProps) {
  const isEdit = Boolean(collection);
  const [name, setName] = useState("");
  const [summary, setSummary] = useState("");
  const [ordered, setOrdered] = useState(false);
  const [mode, setMode] = useState<Mode>("manual");
  const [condition, setCondition] = useState<Condition | undefined>(undefined);

  // Seed fields when (re)opening.
  useEffect(() => {
    if (!opened) return;
    setName(collection?.name ?? "");
    setSummary(collection?.summary ?? "");
    setOrdered(collection?.ordered ?? false);
    const existing = (collection?.condition ?? initialCondition ?? undefined) as
      | SeriesCondition
      | undefined;
    setMode(existing ? "automatic" : "manual");
    setCondition(existing ? ensureRoot(existing as Condition) : undefined);
  }, [opened, collection, initialCondition]);

  const createMutation = useCreateCollection();
  const updateMutation = useUpdateCollection(collection?.id ?? "");
  const pending = createMutation.isPending || updateMutation.isPending;

  // The API rejects an empty rule (an empty allOf would match the whole
  // library), so an automatic collection needs at least one complete leaf.
  const emitted = condition
    ? (normalizeForEmit(condition, "series") as SeriesCondition | undefined)
    : undefined;
  const ruleIsUsable = mode === "manual" || emitted !== undefined;
  const personalized = emitted ? describesPersonalData(emitted) : false;

  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed || !ruleIsUsable) return;
    const trimmedSummary = summary.trim();
    // An automatic collection has no manual arrangement, and the API rejects
    // `ordered` alongside a rule.
    const isAuto = mode === "automatic";

    if (isEdit) {
      updateMutation.mutate(
        {
          name: trimmed,
          summary: trimmedSummary || null,
          ordered: isAuto ? false : ordered,
          // Explicit null clears the rule and converts back to manual.
          condition: isAuto
            ? (emitted as UpdateCondition)
            : (null as UpdateCondition),
        },
        { onSuccess: () => onClose() },
      );
    } else {
      createMutation.mutate(
        {
          name: trimmed,
          summary: trimmedSummary || undefined,
          ordered: isAuto ? false : ordered,
          condition: isAuto ? (emitted as CreateCondition) : undefined,
        },
        {
          onSuccess: (created) => {
            onCreated?.(created);
            onClose();
          },
        },
      );
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEdit ? "Edit collection" : "New collection"}
      centered
      size={mode === "automatic" ? "xl" : "md"}
    >
      <Stack gap="md">
        <TextInput
          label="Name"
          placeholder="e.g. Batman"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          data-autofocus
          required
        />
        <Textarea
          label="Summary"
          placeholder="Optional description"
          value={summary}
          onChange={(e) => setSummary(e.currentTarget.value)}
          autosize
          minRows={2}
        />

        <Stack gap={4}>
          <Text size="sm" fw={500}>
            Membership
          </Text>
          <SegmentedControl
            value={mode}
            onChange={(value) => setMode(value as Mode)}
            data={[
              { label: "Hand-picked", value: "manual" },
              { label: "Automatic", value: "automatic" },
            ]}
            aria-label="Membership mode"
          />
          <Text size="xs" c="dimmed">
            {mode === "manual"
              ? "You choose which series belong, one at a time."
              : "Series matching the rule below belong automatically. The contents stay current as your library changes, and cannot be edited by hand."}
          </Text>
        </Stack>

        {mode === "manual" ? (
          <Checkbox
            label="Default to manual order"
            description="When off, members default to sorting by title. Either way, every sort (including Manual) stays available on the collection page."
            checked={ordered}
            onChange={(e) => setOrdered(e.currentTarget.checked)}
          />
        ) : (
          <Stack gap="xs">
            <FilterBuilder
              condition={condition}
              target="series"
              onChange={setCondition}
            />
            {!ruleIsUsable && (
              <Text size="xs" c="dimmed">
                Add at least one filter. A rule with no conditions would match
                your entire library.
              </Text>
            )}
            {personalized && (
              <Alert
                variant="light"
                color="yellow"
                icon={<IconInfoCircle size={16} />}
              >
                This rule uses your own ratings or reading progress, so each
                person will see a different set of series in this collection.
              </Alert>
            )}
          </Stack>
        )}

        <Group justify="flex-end">
          <Button variant="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={submit}
            loading={pending}
            disabled={!name.trim() || !ruleIsUsable}
          >
            {isEdit ? "Save" : "Create"}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

// The generated request types model `condition` as an opaque object, so the
// typed condition needs a cast at the boundary. Named aliases keep the two
// call sites readable.
type CreateCondition = NonNullable<
  Parameters<ReturnType<typeof useCreateCollection>["mutate"]>[0]["condition"]
>;
type UpdateCondition = Parameters<
  ReturnType<typeof useUpdateCollection>["mutate"]
>[0]["condition"];
