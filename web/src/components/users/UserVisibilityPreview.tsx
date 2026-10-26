import {
  Alert,
  Badge,
  Card,
  Group,
  Loader,
  Select,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { IconEye, IconShieldOff } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { accessGroupsApi, type VisibilityPreviewDto } from "@/api/accessGroups";
import { seriesApi } from "@/api/series";

type SeriesAccess = NonNullable<VisibilityPreviewDto["series"]>;
type MatchedRule = SeriesAccess["deniedBy"][number];
type Source = MatchedRule["source"];

interface UserVisibilityPreviewProps {
  userId: string;
}

const RULE_TYPE_LABELS: Record<MatchedRule["ruleType"], string> = {
  genre: "genre",
  tag: "tag",
  library: "library",
  sharingTag: "sharing tag",
};

function describeSource(source: Source): string {
  return source.kind === "group"
    ? `group ${source.groupName ?? ""}`.trim()
    : "a personal sharing-tag grant";
}

/**
 * Shows an admin what a user can see: how many series, which libraries, and
 * for a series they pick, the rule that shows or hides it.
 */
export function UserVisibilityPreview({ userId }: UserVisibilityPreviewProps) {
  const [search, setSearch] = useState("");
  const [debouncedSearch] = useDebouncedValue(search, 250);
  const [seriesId, setSeriesId] = useState<string | null>(null);

  const { data: preview, isLoading } = useQuery({
    queryKey: ["visibility-preview", userId],
    queryFn: () => accessGroupsApi.getVisibilityPreview(userId),
  });

  const { data: matches = [] } = useQuery({
    queryKey: ["visibility-preview-series-search", debouncedSearch],
    queryFn: async () =>
      (
        await seriesApi.search("all", {
          search: debouncedSearch,
          pageSize: 20,
        })
      ).data,
    enabled: debouncedSearch.trim().length > 0,
  });

  const { data: explained } = useQuery({
    queryKey: ["visibility-preview", userId, seriesId],
    queryFn: () =>
      accessGroupsApi.getVisibilityPreview(userId, seriesId ?? undefined),
    enabled: !!seriesId,
  });

  return (
    <Card withBorder p="md">
      <Stack gap="md">
        <Group gap="sm">
          <IconEye size={20} />
          <Title order={5}>What this user sees</Title>
        </Group>

        {isLoading || !preview ? (
          <Group justify="center">
            <Loader size="sm" />
          </Group>
        ) : (
          <>
            {preview.roleBypass ? (
              <Alert
                variant="light"
                color="gray"
                icon={<IconShieldOff size={16} />}
                p="xs"
              >
                This user's role is never restricted: admins and maintainers see
                everything, whatever their access groups say.
              </Alert>
            ) : (
              <Text size="sm">
                {preview.visibleSeriesCount} series visible,{" "}
                {preview.hiddenSeriesCount} hidden
                {!preview.restricted && " (no rule restricts this user)"}
              </Text>
            )}

            <Group gap={6}>
              {preview.libraries.map((library) => (
                <Badge
                  key={library.id}
                  variant="light"
                  color={library.visible ? "green" : "red"}
                  aria-label={`${library.name}: ${library.visible ? "visible" : "hidden"}`}
                >
                  {library.name}
                </Badge>
              ))}
            </Group>

            <Select
              label="Check a series"
              placeholder="Search for a series..."
              searchable
              searchValue={search}
              onSearchChange={setSearch}
              data={matches.map((s) => ({ value: s.id, label: s.title }))}
              value={seriesId}
              onChange={setSeriesId}
              filter={({ options }) => options}
              nothingFoundMessage={
                debouncedSearch ? "No series found" : undefined
              }
              clearable
            />

            {explained?.series && (
              <SeriesExplanation access={explained.series} />
            )}
          </>
        )}
      </Stack>
    </Card>
  );
}

function SeriesExplanation({ access }: { access: SeriesAccess }) {
  return (
    <Stack gap={4}>
      <Group gap="xs">
        <Badge color={access.visible ? "green" : "red"}>
          {access.visible ? "Visible" : "Hidden"}
        </Badge>
        <Text size="sm" fw={500}>
          {access.title}
        </Text>
      </Group>
      {access.decision === "denied" && (
        <>
          <Text size="sm">Hidden by:</Text>
          {access.deniedBy.map((rule) => (
            <Text
              size="sm"
              c="dimmed"
              key={`${rule.ruleType}-${rule.value}-${rule.source.groupId ?? "user"}`}
            >
              Deny {RULE_TYPE_LABELS[rule.ruleType]} "{rule.value}", from{" "}
              {describeSource(rule.source)}
            </Text>
          ))}
        </>
      )}
      {access.decision === "notAllowed" && (
        <Text size="sm" c="dimmed">
          Hidden: this user's groups only show what their allow rules match, and
          none of them matches this series.
        </Text>
      )}
      {access.decision === "allowed" && (
        <Text size="sm" c="dimmed">
          Shown by {access.allowedBy.map(describeSource).join(", ")}.
        </Text>
      )}
      {access.decision === "open" && (
        <Text size="sm" c="dimmed">
          Shown: no allow rule applies to this user, and no deny rule matches.
        </Text>
      )}
      {access.decision === "unrestricted" && (
        <Text size="sm" c="dimmed">
          Shown: this user's role is never restricted.
        </Text>
      )}
    </Stack>
  );
}
