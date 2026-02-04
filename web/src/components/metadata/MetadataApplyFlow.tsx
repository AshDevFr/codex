import { Center, Group, Modal, Stack, Text, ThemeIcon } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconCheck, IconSearch } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import type { PluginActionDto, PluginSearchResultDto } from "@/api/plugins";
import { MetadataPreview } from "./MetadataPreview";
import { MetadataSearchModal } from "./MetadataSearchModal";

export interface MetadataApplyFlowProps {
  /** Whether the flow is active */
  opened: boolean;
  /** Callback to close the flow */
  onClose: () => void;
  /** The plugin to use */
  plugin: PluginActionDto;
  /** Entity ID (series or book) */
  entityId: string;
  /** Entity title for search */
  entityTitle: string;
  /** Author name to refine search results (for book searches) */
  entityAuthor?: string;
  /** Content type */
  contentType?: "series" | "book";
  /** Callback when metadata is successfully applied */
  onApplySuccess?: () => void;
}

type FlowStep = "search" | "preview" | "success";

/**
 * Orchestrates the full metadata apply flow:
 * 1. Search for metadata using a plugin
 * 2. Preview changes before applying
 * 3. Apply and show success
 *
 * This component manages the state machine between steps.
 */
export function MetadataApplyFlow({
  opened,
  onClose,
  plugin,
  entityId,
  entityTitle,
  entityAuthor,
  contentType = "series",
  onApplySuccess,
}: MetadataApplyFlowProps) {
  const [step, setStep] = useState<FlowStep>("search");
  const [selectedResult, setSelectedResult] =
    useState<PluginSearchResultDto | null>(null);
  const [appliedFields, setAppliedFields] = useState<string[]>([]);

  // Reset state when modal opens
  useEffect(() => {
    if (opened) {
      setStep("search");
      setSelectedResult(null);
      setAppliedFields([]);
    }
  }, [opened]);

  // Handle search result selection
  const handleSearchSelect = (result: PluginSearchResultDto) => {
    setSelectedResult(result);
    setStep("preview");
  };

  // Handle going back to search from preview
  const handleBackToSearch = () => {
    setStep("search");
    setSelectedResult(null);
  };

  // Handle apply completion
  const handleApplyComplete = (success: boolean, fields: string[]) => {
    if (success) {
      setAppliedFields(fields);
      setStep("success");
      notifications.show({
        title: "Metadata Applied",
        message: `Updated ${fields.length} field${fields.length !== 1 ? "s" : ""} from ${plugin.pluginDisplayName}`,
        color: "green",
        icon: <IconCheck size={16} />,
      });
      onApplySuccess?.();
    } else {
      notifications.show({
        title: "No Changes Applied",
        message: "No fields were updated. Check field locks and permissions.",
        color: "yellow",
      });
    }
  };

  // Handle close with cleanup
  const handleClose = () => {
    onClose();
    // Reset state after modal animation completes
    setTimeout(() => {
      setStep("search");
      setSelectedResult(null);
      setAppliedFields([]);
    }, 200);
  };

  // Use searchModal directly for the search step, preview modal for preview/success
  if (step === "search") {
    return (
      <MetadataSearchModal
        opened={opened}
        onClose={handleClose}
        plugin={plugin}
        initialQuery={entityTitle}
        author={entityAuthor}
        contentType={contentType}
        onSelect={handleSearchSelect}
      />
    );
  }

  // Preview and success steps use a different modal
  return (
    <Modal
      opened={opened}
      onClose={handleClose}
      title={
        <Group gap="xs">
          {step === "success" ? (
            <>
              <ThemeIcon color="green" size="sm" radius="xl">
                <IconCheck size={14} />
              </ThemeIcon>
              <Text fw={600}>Metadata Applied</Text>
            </>
          ) : (
            <>
              <IconSearch size={20} />
              <Text fw={600}>Preview Changes</Text>
            </>
          )}
        </Group>
      }
      size="lg"
    >
      {step === "preview" && selectedResult && (
        <MetadataPreview
          seriesId={entityId}
          pluginId={plugin.pluginId}
          externalId={selectedResult.externalId}
          pluginName={plugin.pluginDisplayName}
          contentType={contentType}
          onApplyComplete={handleApplyComplete}
          onBack={handleBackToSearch}
        />
      )}

      {step === "success" && (
        <Center py="xl">
          <Stack align="center" gap="md">
            <ThemeIcon color="green" size={60} radius="xl">
              <IconCheck size={32} />
            </ThemeIcon>
            <Text size="lg" fw={500}>
              Successfully updated metadata
            </Text>
            <Text c="dimmed" ta="center">
              Applied {appliedFields.length} field
              {appliedFields.length !== 1 ? "s" : ""}:{" "}
              {appliedFields.join(", ")}
            </Text>
          </Stack>
        </Center>
      )}
    </Modal>
  );
}
