import {
  ActionIcon,
  Alert,
  Autocomplete,
  Badge,
  Button,
  Card,
  Divider,
  Group,
  Loader,
  SegmentedControl,
  Select,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconFilter,
  IconInfoCircle,
  IconShieldOff,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  type AccessGroupRulesDto,
  accessGroupsApi,
  type ReplaceAccessGroupRulesRequest,
  type RuleMatch,
} from "@/api/accessGroups";
import { genresApi } from "@/api/genres";
import { librariesApi } from "@/api/libraries";
import { type AccessMode, sharingTagsApi } from "@/api/sharingTags";
import { tagsApi } from "@/api/tags";

interface AccessGroupRulesCardProps {
  groupId: string;
}

/** Strip display-only fields so a rule set can be sent back as a replace request. */
function toRequest(rules: AccessGroupRulesDto): ReplaceAccessGroupRulesRequest {
  return {
    sharingTagMatch: rules.sharingTagMatch,
    tagMatch: rules.tagMatch,
    genreMatch: rules.genreMatch,
    sharingTags: rules.sharingTags.map(({ sharingTagId, accessMode }) => ({
      sharingTagId,
      accessMode,
    })),
    tags: rules.tags.map(({ name, accessMode }) => ({ name, accessMode })),
    genres: rules.genres.map(({ name, accessMode }) => ({ name, accessMode })),
    libraries: rules.libraries.map(({ libraryId, accessMode }) => ({
      libraryId,
      accessMode,
    })),
  };
}

/**
 * Editor for every rule of an access group: libraries, genres, tags and
 * sharing tags, each allow or deny. Every change replaces the whole rule set
 * on the server, so what is shown is always what is stored.
 */
export function AccessGroupRulesCard({ groupId }: AccessGroupRulesCardProps) {
  const queryClient = useQueryClient();
  const queryKey = ["access-group-rules", groupId];

  const { data: rules, isLoading } = useQuery({
    queryKey,
    queryFn: () => accessGroupsApi.getRules(groupId),
  });
  const { data: libraries = [] } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });
  const { data: genres = [] } = useQuery({
    queryKey: ["genres"],
    queryFn: genresApi.getAll,
  });
  const { data: tags = [] } = useQuery({
    queryKey: ["tags"],
    queryFn: tagsApi.getAll,
  });
  const { data: sharingTags = [] } = useQuery({
    queryKey: ["sharing-tags"],
    queryFn: sharingTagsApi.list,
  });

  const saveMutation = useMutation({
    mutationFn: (request: ReplaceAccessGroupRulesRequest) =>
      accessGroupsApi.replaceRules(groupId, request),
    onSuccess: (saved) => {
      queryClient.setQueryData(queryKey, saved);
      queryClient.invalidateQueries({ queryKey: ["access-group", groupId] });
      queryClient.invalidateQueries({ queryKey: ["access-groups"] });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to save rules",
        color: "red",
      });
    },
  });

  if (isLoading || !rules) {
    return (
      <Card withBorder p="md">
        <Group justify="center">
          <Loader size="sm" />
        </Group>
      </Card>
    );
  }

  const save = (change: Partial<ReplaceAccessGroupRulesRequest>) =>
    saveMutation.mutate({ ...toRequest(rules), ...change });
  const current = toRequest(rules);

  const usedLibraries = new Set(rules.libraries.map((r) => r.libraryId));
  const usedSharingTags = new Set(rules.sharingTags.map((r) => r.sharingTagId));
  const usedGenres = new Set(rules.genres.map((r) => r.name.toLowerCase()));
  const usedTags = new Set(rules.tags.map((r) => r.name.toLowerCase()));

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconFilter size={20} />
          <Title order={4}>Rules</Title>
        </Group>

        <Text size="sm" c="dimmed">
          Allow rules narrow what members see: different kinds of rules in this
          group must all match (library <i>and</i> genre). A member of several
          groups sees what any of their groups allows. Deny rules hide content
          everywhere, whatever another group allows. With no allow rules,
          members see everything that is not denied.
        </Text>

        <Alert
          variant="light"
          color="gray"
          icon={<IconShieldOff size={16} />}
          p="xs"
        >
          Admins and maintainers are never restricted by access groups, even
          when they are members.
        </Alert>

        <RuleSection
          singular="library"
          plural="libraries"
          rules={rules.libraries.map((r) => ({
            key: r.libraryId,
            label: r.name,
            accessMode: r.accessMode,
          }))}
          options={libraries
            .filter((l) => !usedLibraries.has(l.id))
            .map((l) => ({ value: l.id, label: l.name }))}
          onAdd={(libraryId, accessMode) =>
            save({
              libraries: [...current.libraries!, { libraryId, accessMode }],
            })
          }
          onRemove={(libraryId) =>
            save({
              libraries: current.libraries!.filter(
                (r) => r.libraryId !== libraryId,
              ),
            })
          }
          busy={saveMutation.isPending}
        />

        <Divider />

        <RuleSection
          singular="genre"
          plural="genres"
          freeText
          hint="Matched by name, ignoring case. A genre no series has yet still works once one does."
          rules={rules.genres.map((r) => ({
            key: r.name,
            label: r.name,
            accessMode: r.accessMode,
          }))}
          options={genres
            .filter((g) => !usedGenres.has(g.name.toLowerCase()))
            .map((g) => ({ value: g.name, label: g.name }))}
          match={rules.genreMatch}
          onMatchChange={(genreMatch) => save({ genreMatch })}
          onAdd={(name, accessMode) =>
            save({ genres: [...current.genres!, { name, accessMode }] })
          }
          onRemove={(name) =>
            save({ genres: current.genres!.filter((r) => r.name !== name) })
          }
          busy={saveMutation.isPending}
        />

        <Divider />

        <RuleSection
          singular="tag"
          plural="tags"
          freeText
          hint="Matched by name, ignoring case."
          rules={rules.tags.map((r) => ({
            key: r.name,
            label: r.name,
            accessMode: r.accessMode,
          }))}
          options={tags
            .filter((t) => !usedTags.has(t.name.toLowerCase()))
            .map((t) => ({ value: t.name, label: t.name }))}
          match={rules.tagMatch}
          onMatchChange={(tagMatch) => save({ tagMatch })}
          onAdd={(name, accessMode) =>
            save({ tags: [...current.tags!, { name, accessMode }] })
          }
          onRemove={(name) =>
            save({ tags: current.tags!.filter((r) => r.name !== name) })
          }
          busy={saveMutation.isPending}
        />

        <Divider />

        <RuleSection
          singular="sharing tag"
          plural="sharing tags"
          rules={rules.sharingTags.map((r) => ({
            key: r.sharingTagId,
            label: r.name,
            accessMode: r.accessMode,
          }))}
          options={sharingTags
            .filter((t) => !usedSharingTags.has(t.id))
            .map((t) => ({ value: t.id, label: t.name }))}
          match={rules.sharingTagMatch}
          onMatchChange={(sharingTagMatch) => save({ sharingTagMatch })}
          onAdd={(sharingTagId, accessMode) =>
            save({
              sharingTags: [
                ...current.sharingTags!,
                { sharingTagId, accessMode },
              ],
            })
          }
          onRemove={(sharingTagId) =>
            save({
              sharingTags: current.sharingTags!.filter(
                (r) => r.sharingTagId !== sharingTagId,
              ),
            })
          }
          busy={saveMutation.isPending}
        />
      </Stack>
    </Card>
  );
}

// ==================== One rule type ====================

interface RuleRow {
  key: string;
  label: string;
  accessMode: AccessMode;
}

interface RuleSectionProps {
  singular: string;
  plural: string;
  rules: RuleRow[];
  options: { value: string; label: string }[];
  /** Tags and genres accept any name, not only existing ones */
  freeText?: boolean;
  hint?: string;
  /** Omitted for types where "all of" cannot apply (libraries) */
  match?: RuleMatch;
  onMatchChange?: (match: RuleMatch) => void;
  onAdd: (value: string, accessMode: AccessMode) => void;
  onRemove: (key: string) => void;
  busy: boolean;
}

function RuleSection({
  singular,
  plural,
  rules,
  options,
  freeText = false,
  hint,
  match,
  onMatchChange,
  onAdd,
  onRemove,
  busy,
}: RuleSectionProps) {
  const [value, setValue] = useState<string | null>(null);
  const capitalized = plural.charAt(0).toUpperCase() + plural.slice(1);
  const allowCount = rules.filter((r) => r.accessMode === "allow").length;

  const add = (accessMode: AccessMode) => {
    const trimmed = value?.trim();
    if (!trimmed) return;
    onAdd(trimmed, accessMode);
    setValue(null);
  };

  return (
    <Stack gap="xs">
      <Group justify="space-between" wrap="wrap">
        <Group gap="xs">
          <Text fw={500}>{capitalized}</Text>
          <Badge variant="light" size="sm">
            {rules.length}
          </Badge>
        </Group>
        {match && onMatchChange && allowCount >= 2 && (
          <SegmentedControl
            size="xs"
            value={match}
            onChange={(v) => onMatchChange(v as RuleMatch)}
            disabled={busy}
            data={[
              { value: "any", label: `Any allowed ${singular}` },
              { value: "all", label: `All allowed ${plural}` },
            ]}
          />
        )}
      </Group>

      {hint && (
        <Group gap={4} wrap="nowrap">
          <IconInfoCircle size={14} style={{ flexShrink: 0 }} />
          <Text size="xs" c="dimmed">
            {hint}
          </Text>
        </Group>
      )}

      {rules.length > 0 ? (
        <Stack gap={4}>
          {rules.map((rule) => (
            <Group key={rule.key} justify="space-between">
              <Group gap="sm">
                <Badge
                  variant="light"
                  color={rule.accessMode === "deny" ? "red" : "green"}
                >
                  {rule.accessMode === "deny" ? "Deny" : "Allow"}
                </Badge>
                <Text size="sm">{rule.label}</Text>
              </Group>
              <Tooltip label="Remove rule">
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  color="red"
                  aria-label={`Remove ${singular} ${rule.label}`}
                  onClick={() => onRemove(rule.key)}
                  disabled={busy}
                >
                  <IconTrash size={14} />
                </ActionIcon>
              </Tooltip>
            </Group>
          ))}
        </Stack>
      ) : (
        <Text size="sm" c="dimmed">
          No {singular} rules.
        </Text>
      )}

      <Group gap="sm" align="flex-end">
        {freeText ? (
          <Autocomplete
            aria-label={`Add ${singular}`}
            placeholder={`Type a ${singular}...`}
            data={options}
            value={value ?? ""}
            onChange={setValue}
            style={{ flex: 1 }}
          />
        ) : (
          <Select
            aria-label={`Add ${singular}`}
            placeholder={`Select a ${singular}...`}
            data={options}
            value={value}
            onChange={setValue}
            searchable
            disabled={options.length === 0}
            style={{ flex: 1 }}
          />
        )}
        <Button
          variant="light"
          color="green"
          aria-label={`Allow ${singular}`}
          onClick={() => add("allow")}
          disabled={!value?.trim() || busy}
        >
          Allow
        </Button>
        <Button
          variant="light"
          color="red"
          aria-label={`Deny ${singular}`}
          onClick={() => add("deny")}
          disabled={!value?.trim() || busy}
        >
          Deny
        </Button>
      </Group>
    </Stack>
  );
}
