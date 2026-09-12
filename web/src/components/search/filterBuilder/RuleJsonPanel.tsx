import { Alert, Button, Group, JsonInput, Stack, Text } from "@mantine/core";
import { useClipboard } from "@mantine/hooks";
import { IconAlertTriangle, IconCheck, IconCopy } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { librariesApi } from "@/api/libraries";
import {
  asGroup,
  type Condition,
  ensureRoot,
  leafFieldKey,
  normalizeForEmit,
  parseCondition,
} from "./conditionUtils";
import type { FieldTarget } from "./fieldCatalog";

interface RuleJsonPanelProps {
  /** The builder's current tree, incomplete rows and all. */
  condition: Condition | undefined;
  target: FieldTarget;
  /** Called with a parsed, root-wrapped tree when the user applies an edit. */
  onChange: (next: Condition) => void;
}

/** Flatten a tree into `[fieldKey, operatorNode]` pairs, groups excluded. */
function collectLeaves(
  c: Condition | undefined,
): Array<[string, Record<string, unknown>]> {
  if (!c) return [];
  const group = asGroup(c);
  if (group) return group.children.flatMap(collectLeaves);
  const key = leafFieldKey(c);
  if (!key) return [];
  const node = (c as Record<string, unknown>)[key];
  if (typeof node !== "object" || node === null) return [];
  return [[key, node as Record<string, unknown>]];
}

/** Every library uuid the rule names, in either the single or list form. */
function referencedLibraryIds(c: Condition | undefined): string[] {
  const ids = new Set<string>();
  for (const [key, node] of collectLeaves(c)) {
    if (key !== "libraryId") continue;
    const { value, values } = node;
    if (typeof value === "string" && value) ids.add(value);
    if (Array.isArray(values)) {
      for (const v of values) if (typeof v === "string" && v) ids.add(v);
    }
  }
  return [...ids];
}

/**
 * The rule as JSON, for copying out and pasting in.
 *
 * The builder above is the structured editor; this is the transport. Rebuilding
 * a tuned rule row by row in a second collection is the thing it exists to
 * avoid, and it doubles as something you can paste into a bug report.
 *
 * The editor mirrors the builder until the user types, then holds their text
 * until they Apply or Revert - a row changing underneath must not wipe a
 * half-written edit. What it mirrors is `normalizeForEmit`, i.e. the rule that
 * would actually be saved, so a row mid-fill is absent rather than shown in a
 * shape the API would reject.
 */
export function RuleJsonPanel({
  condition,
  target,
  onChange,
}: RuleJsonPanelProps) {
  const serialized = useMemo(() => {
    const emitted = condition ? normalizeForEmit(condition, target) : undefined;
    return emitted ? JSON.stringify(emitted, null, 2) : "";
  }, [condition, target]);

  // `null` means clean: the editor tracks the builder. A string means the user
  // has taken it over.
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const clipboard = useClipboard({ timeout: 1500 });

  const text = draft ?? serialized;
  const dirty = draft !== null;

  const libraryIds = useMemo(
    () => referencedLibraryIds(condition),
    [condition],
  );
  // Shares a cache key with the LeafEditor's library picker, and only a rule
  // that names a library can have an unresolvable one - so this costs no
  // request that the Library row has not already made.
  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: () => librariesApi.getAll(),
    enabled: libraryIds.length > 0,
    staleTime: 5 * 60 * 1000,
  });
  const unresolved = libraries
    ? libraryIds.filter((id) => !libraries.some((l) => l.id === id))
    : [];

  const mentionsRating = useMemo(
    () =>
      collectLeaves(condition).some(
        ([key]) => key === "userRating" || key === "communityRating",
      ),
    [condition],
  );

  const apply = () => {
    const result = parseCondition(text, target);
    if (!result.ok) {
      setError(result.error);
      return;
    }
    setError(null);
    setDraft(null);
    onChange(ensureRoot(result.condition));
  };

  const revert = () => {
    setDraft(null);
    setError(null);
  };

  return (
    <Stack gap="xs">
      <Group justify="space-between" align="flex-end" wrap="nowrap">
        <Text size="xs" c="dimmed">
          The rule exactly as it is stored. Copy it to rebuild this filter
          elsewhere, or paste one in and apply it.
          {mentionsRating &&
            " Ratings here use the stored 1-100 scale, so 7.5 in the picker reads as 75."}
        </Text>
        <Group gap="xs" wrap="nowrap">
          <Button
            size="xs"
            variant="default"
            disabled={!text}
            onClick={() => clipboard.copy(text)}
            leftSection={
              clipboard.copied ? (
                <IconCheck size={14} />
              ) : (
                <IconCopy size={14} />
              )
            }
          >
            {clipboard.copied ? "Copied" : "Copy"}
          </Button>
          {dirty && (
            <Button size="xs" variant="subtle" onClick={revert}>
              Revert
            </Button>
          )}
          <Button size="xs" disabled={!dirty} onClick={apply}>
            Apply
          </Button>
        </Group>
      </Group>

      <JsonInput
        aria-label="Rule JSON"
        value={text}
        onChange={(next) => {
          setDraft(next);
          setError(null);
        }}
        placeholder="Add at least one complete filter above, or paste a rule here."
        autosize
        minRows={6}
        maxRows={20}
        spellCheck={false}
        styles={{ input: { fontFamily: "monospace" } }}
      />

      {error && (
        <Alert
          variant="light"
          color="red"
          icon={<IconAlertTriangle size={16} />}
        >
          {error}
        </Alert>
      )}

      {unresolved.length > 0 && (
        <Alert
          variant="light"
          color="yellow"
          icon={<IconAlertTriangle size={16} />}
        >
          This rule filters on a library that is not on this server, so it will
          match nothing until you pick a real one: {unresolved.join(", ")}
        </Alert>
      )}
    </Stack>
  );
}
