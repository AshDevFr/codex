import {
  Alert,
  Badge,
  Button,
  Card,
  Group,
  Stack,
  TagsInput,
  Text,
  Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAlertCircle, IconShieldCheck } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { type SettingDto, settingsApi } from "@/api/settings";

const SETTING_KEY = "duplicate_detection.trusted_external_id_sources";

function parseSources(setting: SettingDto | undefined): string[] {
  if (!setting) return [];
  try {
    const parsed = JSON.parse(setting.value);
    if (Array.isArray(parsed)) {
      return parsed.filter((s): s is string => typeof s === "string");
    }
  } catch {
    /* fall through */
  }
  return [];
}

/**
 * Admin editor for `duplicate_detection.trusted_external_id_sources`.
 *
 * The duplicate detector's external-ID pass only groups series whose
 * `series_external_ids.source` is on this list. Empty (the default) disables
 * the pass; the library-scoped title pass still runs. We surface this on the
 * Duplicates page because that's the screen users will land on after a false
 * positive (e.g. an ANN ID reused across distinct series).
 */
export function TrustedSourcesEditor() {
  const queryClient = useQueryClient();

  const { data, isLoading, error } = useQuery({
    queryKey: ["setting", SETTING_KEY],
    queryFn: () => settingsApi.get(SETTING_KEY),
  });

  const [draft, setDraft] = useState<string[]>([]);
  useEffect(() => {
    setDraft(parseSources(data));
  }, [data]);

  const saved = parseSources(data);
  const dirty =
    draft.length !== saved.length || draft.some((s, i) => s !== saved[i]);

  const updateMutation = useMutation({
    mutationFn: async (sources: string[]) => {
      const normalized = sources
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
      return settingsApi.update(SETTING_KEY, {
        value: JSON.stringify(normalized),
        changeReason: "Updated trusted external-ID sources",
      });
    },
    onSuccess: (updated) => {
      queryClient.setQueryData(["setting", SETTING_KEY], updated);
      notifications.show({
        title: "Saved",
        message: "Trusted sources updated. Re-run the duplicate scan to apply.",
        color: "green",
      });
    },
    onError: (err: unknown) => {
      notifications.show({
        title: "Save failed",
        message: err instanceof Error ? err.message : "Unknown error",
        color: "red",
      });
    },
  });

  return (
    <Card withBorder>
      <Stack gap="sm">
        <Group gap="xs" wrap="nowrap">
          <IconShieldCheck size={20} />
          <Title order={4}>Trusted External-ID Sources</Title>
        </Group>
        <Text size="sm" c="dimmed">
          The high-confidence pass only groups series whose external-ID source
          is on this list (e.g. <code>plugin:mangabaka</code>,{" "}
          <code>api:anilist</code>). Leave empty if you don't trust any source
          enough to treat ID matches as duplicates &mdash; the title pass will
          still run.
        </Text>

        {error ? (
          <Alert color="red" icon={<IconAlertCircle size={16} />}>
            Failed to load the setting. You may need admin permissions.
          </Alert>
        ) : null}

        <TagsInput
          label="Sources"
          placeholder="e.g. plugin:mangabaka"
          description="Press Enter to add. Sources are the value before the last `:` in an external ID (e.g. for `api:animenewsnetwork:6872`, the source is `api:animenewsnetwork`)."
          value={draft}
          onChange={setDraft}
          disabled={isLoading || updateMutation.isPending}
          clearable
        />

        <Group justify="space-between" align="center">
          <Group gap="xs">
            {saved.length === 0 ? (
              <Badge variant="light" color="gray">
                External-ID pass disabled
              </Badge>
            ) : (
              <Badge variant="light" color="green">
                {saved.length} trusted source{saved.length === 1 ? "" : "s"}
              </Badge>
            )}
          </Group>
          <Group gap="xs">
            <Button
              variant="subtle"
              disabled={!dirty || updateMutation.isPending}
              onClick={() => setDraft(saved)}
            >
              Reset
            </Button>
            <Button
              disabled={!dirty}
              loading={updateMutation.isPending}
              onClick={() => updateMutation.mutate(draft)}
            >
              Save
            </Button>
          </Group>
        </Group>
      </Stack>
    </Card>
  );
}
