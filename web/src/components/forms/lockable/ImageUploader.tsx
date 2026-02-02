import {
  ActionIcon,
  Box,
  Card,
  Group,
  Image,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { Dropzone, IMAGE_MIME_TYPE } from "@mantine/dropzone";
import {
  IconPhoto,
  IconRefresh,
  IconTrash,
  IconUpload,
  IconX,
} from "@tabler/icons-react";
import { useCallback, useState } from "react";

export interface ImageInfo {
  url: string;
  file?: File;
  size?: number;
  width?: number;
  height?: number;
  mimeType?: string;
}

export interface ImageUploaderProps {
  /** Current image info */
  value: ImageInfo | null;
  /** Callback when image changes */
  onChange: (value: ImageInfo | null) => void;
  /** Callback to refresh/reset to original image */
  onRefresh?: () => void;
  /** Label for the dropzone */
  label?: string;
  /** Maximum file size in bytes (default: 5MB) */
  maxSize?: number;
  /** Accepted MIME types */
  accept?: string[];
  /** Whether the uploader is disabled */
  disabled?: boolean;
}

/**
 * A drag-and-drop image uploader with preview and metadata display.
 *
 * Features:
 * - Drag and drop or click to upload
 * - Image preview with file info
 * - Refresh (reset to original), confirm, and delete actions
 */
export function ImageUploader({
  value,
  onChange,
  onRefresh,
  label = "Choose an image - drag and drop",
  maxSize = 5 * 1024 * 1024, // 5MB
  accept = IMAGE_MIME_TYPE,
  disabled = false,
}: ImageUploaderProps) {
  const [error, setError] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);

  const formatBytes = useCallback((bytes: number): string => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "kB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${Number.parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }, []);

  const handleDrop = useCallback(
    (files: File[]) => {
      setError(null);

      if (files.length === 0) return;

      const file = files[0];

      // Validate file size
      if (file.size > maxSize) {
        setError(`File size must be less than ${formatBytes(maxSize)}`);
        return;
      }

      // Create preview URL
      const url = URL.createObjectURL(file);
      setPreviewUrl(url);

      // Get image dimensions
      const img = new window.Image();
      img.onload = () => {
        onChange({
          url,
          file,
          size: file.size,
          width: img.naturalWidth,
          height: img.naturalHeight,
          mimeType: file.type,
        });
      };
      img.onerror = () => {
        setError("Failed to load image");
        URL.revokeObjectURL(url);
      };
      img.src = url;
    },
    [maxSize, onChange, formatBytes],
  );

  const handleReject = useCallback(() => {
    setError("Invalid file type. Please upload an image.");
  }, []);

  const handleDelete = useCallback(() => {
    if (previewUrl) {
      URL.revokeObjectURL(previewUrl);
      setPreviewUrl(null);
    }
    onChange(null);
  }, [previewUrl, onChange]);

  const handleRefresh = useCallback(() => {
    if (previewUrl) {
      URL.revokeObjectURL(previewUrl);
      setPreviewUrl(null);
    }
    onRefresh?.();
  }, [previewUrl, onRefresh]);

  return (
    <Stack gap="md">
      <Dropzone
        onDrop={handleDrop}
        onReject={handleReject}
        maxSize={maxSize}
        accept={accept}
        disabled={disabled}
        multiple={false}
      >
        <Group
          justify="center"
          gap="xl"
          mih={120}
          style={{ pointerEvents: "none" }}
        >
          <Dropzone.Accept>
            <IconUpload size={50} stroke={1.5} />
          </Dropzone.Accept>
          <Dropzone.Reject>
            <IconX size={50} stroke={1.5} />
          </Dropzone.Reject>
          <Dropzone.Idle>
            <IconPhoto size={50} stroke={1.5} />
          </Dropzone.Idle>

          <Box>
            <Text size="lg" inline>
              {label}
            </Text>
            <Text size="sm" c="dimmed" inline mt={7}>
              File should not exceed {formatBytes(maxSize)}
            </Text>
          </Box>
        </Group>
      </Dropzone>

      {error && (
        <Text c="red" size="sm">
          {error}
        </Text>
      )}

      {value && (
        <Card withBorder p="md">
          <Group wrap="nowrap" align="flex-start">
            <Image
              src={value.url}
              alt="Preview"
              w={100}
              h={140}
              fit="contain"
              radius="sm"
            />

            <Stack gap="xs" style={{ flex: 1 }}>
              {value.size !== undefined && (
                <Text size="sm" c="dimmed">
                  Size: {formatBytes(value.size)}
                </Text>
              )}
              {value.width !== undefined && value.height !== undefined && (
                <Text size="sm" c="dimmed">
                  Dimensions: {value.width} × {value.height}
                </Text>
              )}
              {value.mimeType && (
                <Text size="sm" c="dimmed">
                  Type: {value.mimeType}
                </Text>
              )}
            </Stack>

            <Group gap="xs">
              {onRefresh && (
                <Tooltip label="Reset to original">
                  <ActionIcon
                    variant="subtle"
                    color="blue"
                    onClick={handleRefresh}
                    aria-label="Reset to original"
                  >
                    <IconRefresh size={18} />
                  </ActionIcon>
                </Tooltip>
              )}
              <Tooltip label="Delete image">
                <ActionIcon
                  variant="subtle"
                  color="red"
                  onClick={handleDelete}
                  aria-label="Delete image"
                >
                  <IconTrash size={18} />
                </ActionIcon>
              </Tooltip>
            </Group>
          </Group>
        </Card>
      )}
    </Stack>
  );
}
