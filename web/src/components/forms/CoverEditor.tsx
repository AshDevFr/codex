import {
  ActionIcon,
  Box,
  Button,
  Card,
  Group,
  Image,
  SimpleGrid,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { Dropzone, IMAGE_MIME_TYPE } from "@mantine/dropzone";
import { notifications } from "@mantine/notifications";
import {
  IconCheck,
  IconLock,
  IconLockOpen,
  IconPhoto,
  IconRefresh,
  IconTrash,
  IconUpload,
  IconX,
} from "@tabler/icons-react";
import { useState } from "react";

/** Minimal cover shape shared by BookCoverDto and SeriesCoverDto */
export interface CoverItem {
  id: string;
  isSelected: boolean;
  source: string;
}

export interface CoverEditorProps {
  /** List of existing covers */
  covers: CoverItem[];

  /** Whether the cover lock is enabled */
  coverLocked: boolean;
  /** Callback when cover lock is toggled */
  onCoverLockChange: (locked: boolean) => void;

  /** Called when the user confirms an upload */
  onUpload: (file: File) => void;
  /** Whether an upload is in progress */
  isUploading?: boolean;

  /** Called when a cover is selected */
  onSelect: (coverId: string) => void;
  /** Whether a selection is in progress */
  isSelecting?: boolean;

  /** Called when the user resets to default cover */
  onReset: () => void;
  /** Whether a reset is in progress */
  isResetting?: boolean;

  /** Called when a cover is deleted */
  onDelete: (coverId: string) => void;
  /** Whether a delete is in progress */
  isDeleting?: boolean;

  /** Returns a URL for a cover's image given its ID */
  getCoverImageUrl: (coverId: string) => string;

  /** Returns a human-readable label for a cover source string */
  getCoverSourceLabel: (source: string) => string;

  /** Label for the reset button (e.g., "Reset to Default Cover") */
  resetButtonLabel?: string;

  /** Message shown when using default and no cover is selected */
  defaultCoverMessage?: string;
}

export function CoverEditor({
  covers,
  coverLocked,
  onCoverLockChange,
  onUpload,
  isUploading = false,
  onSelect,
  isSelecting = false,
  onReset,
  isResetting = false,
  onDelete,
  isDeleting = false,
  getCoverImageUrl,
  getCoverSourceLabel,
  resetButtonLabel = "Reset to Default Cover",
  defaultCoverMessage = "Using default cover",
}: CoverEditorProps) {
  const [pendingUpload, setPendingUpload] = useState<File | null>(null);
  const [uploadPreviewUrl, setUploadPreviewUrl] = useState<string | null>(null);

  const handleFileDrop = (files: File[]) => {
    const file = files[0];
    if (file) {
      if (uploadPreviewUrl) {
        URL.revokeObjectURL(uploadPreviewUrl);
      }
      setPendingUpload(file);
      setUploadPreviewUrl(URL.createObjectURL(file));
    }
  };

  const handleUploadConfirm = () => {
    if (pendingUpload) {
      onUpload(pendingUpload);
      // Clean up preview after initiating upload
      if (uploadPreviewUrl) {
        URL.revokeObjectURL(uploadPreviewUrl);
        setUploadPreviewUrl(null);
      }
      setPendingUpload(null);
    }
  };

  const handleUploadCancel = () => {
    if (uploadPreviewUrl) {
      URL.revokeObjectURL(uploadPreviewUrl);
      setUploadPreviewUrl(null);
    }
    setPendingUpload(null);
  };

  const hasSelectedCover = covers.some((c) => c.isSelected);

  return (
    <Stack gap="md">
      <Text size="sm" c="dimmed">
        Upload custom cover images or select from existing covers.
      </Text>

      {/* Cover lock toggle */}
      <Group gap="xs">
        <Tooltip
          label={
            coverLocked
              ? "Locked: Cover selection protected from automatic updates"
              : "Unlocked: Cover can be changed by plugins"
          }
          position="right"
          zIndex={1100}
        >
          <ActionIcon
            variant="subtle"
            color={coverLocked ? "orange" : "gray"}
            onClick={() => onCoverLockChange(!coverLocked)}
            aria-label={coverLocked ? "Unlock cover" : "Lock cover"}
          >
            {coverLocked ? <IconLock size={18} /> : <IconLockOpen size={18} />}
          </ActionIcon>
        </Tooltip>
        <Text size="sm" c={coverLocked ? "orange" : "dimmed"}>
          {coverLocked ? "Cover selection locked" : "Cover selection unlocked"}
        </Text>
      </Group>

      {/* Upload dropzone */}
      <Dropzone
        onDrop={handleFileDrop}
        onReject={() =>
          notifications.show({
            title: "Error",
            message: "Invalid file type. Please upload an image.",
            color: "red",
          })
        }
        maxSize={10 * 1024 * 1024}
        accept={IMAGE_MIME_TYPE}
        multiple={false}
        disabled={isUploading}
      >
        <Group
          justify="center"
          gap="xl"
          mih={100}
          style={{ pointerEvents: "none" }}
        >
          <Dropzone.Accept>
            <IconUpload size={40} stroke={1.5} />
          </Dropzone.Accept>
          <Dropzone.Reject>
            <IconX size={40} stroke={1.5} />
          </Dropzone.Reject>
          <Dropzone.Idle>
            <IconPhoto size={40} stroke={1.5} />
          </Dropzone.Idle>

          <Box>
            <Text size="md" inline>
              Drop image here or click to upload
            </Text>
            <Text size="sm" c="dimmed" inline mt={7}>
              Max file size: 10MB
            </Text>
          </Box>
        </Group>
      </Dropzone>

      {/* Pending upload preview */}
      {pendingUpload && uploadPreviewUrl && (
        <Card withBorder p="md">
          <Group wrap="nowrap" align="flex-start">
            <Image
              src={uploadPreviewUrl}
              alt="Upload preview"
              w={80}
              h={120}
              fit="contain"
              radius="sm"
            />
            <Stack gap="xs" style={{ flex: 1 }}>
              <Text size="sm" fw={500}>
                Ready to upload
              </Text>
              <Text size="sm" c="dimmed">
                {pendingUpload.name}
              </Text>
            </Stack>
            <Group gap="xs">
              <Tooltip label="Upload" zIndex={1100}>
                <ActionIcon
                  variant="filled"
                  color="green"
                  onClick={handleUploadConfirm}
                  loading={isUploading}
                  aria-label="Confirm upload"
                >
                  <IconCheck size={18} />
                </ActionIcon>
              </Tooltip>
              <Tooltip label="Cancel" zIndex={1100}>
                <ActionIcon
                  variant="subtle"
                  color="red"
                  onClick={handleUploadCancel}
                  aria-label="Cancel upload"
                >
                  <IconX size={18} />
                </ActionIcon>
              </Tooltip>
            </Group>
          </Group>
        </Card>
      )}

      {/* Reset to default button */}
      {hasSelectedCover && (
        <Button
          variant="light"
          color="gray"
          leftSection={<IconRefresh size={16} />}
          onClick={onReset}
          loading={isResetting}
        >
          {resetButtonLabel}
        </Button>
      )}

      {/* Existing covers grid */}
      {covers.length > 0 && (
        <>
          <Group justify="space-between" mt="md">
            <Text size="sm" fw={500}>
              Available Covers
            </Text>
            {!hasSelectedCover && (
              <Text size="xs" c="dimmed">
                {defaultCoverMessage}
              </Text>
            )}
          </Group>
          <SimpleGrid cols={4} spacing="md">
            {covers.map((cover) => (
              <Card
                key={cover.id}
                withBorder
                p="xs"
                style={{
                  cursor: "pointer",
                  borderColor: cover.isSelected
                    ? "var(--mantine-color-blue-6)"
                    : undefined,
                  borderWidth: cover.isSelected ? 2 : 1,
                }}
                onClick={() => {
                  if (!cover.isSelected && !isSelecting) {
                    onSelect(cover.id);
                  }
                }}
              >
                <Card.Section>
                  <Image
                    src={getCoverImageUrl(cover.id)}
                    alt="Cover"
                    h={150}
                    fit="contain"
                  />
                </Card.Section>
                <Group justify="space-between" mt="xs" wrap="nowrap">
                  <Stack gap={2}>
                    <Text size="xs" c="dimmed" truncate>
                      {getCoverSourceLabel(cover.source)}
                    </Text>
                    {cover.isSelected && (
                      <Text size="xs" c="blue" fw={500}>
                        Selected
                      </Text>
                    )}
                  </Stack>
                  <Tooltip label="Delete cover" zIndex={1100}>
                    <ActionIcon
                      variant="subtle"
                      color="red"
                      size="sm"
                      onClick={(e: React.MouseEvent) => {
                        e.stopPropagation();
                        onDelete(cover.id);
                      }}
                      loading={isDeleting}
                      aria-label="Delete cover"
                    >
                      <IconTrash size={14} />
                    </ActionIcon>
                  </Tooltip>
                </Group>
              </Card>
            ))}
          </SimpleGrid>
        </>
      )}

      {covers.length === 0 && (
        <Text size="sm" c="dimmed" ta="center" py="xl">
          No covers uploaded yet. Upload an image above.
        </Text>
      )}
    </Stack>
  );
}
