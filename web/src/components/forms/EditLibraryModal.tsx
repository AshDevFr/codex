import { useState, useEffect } from 'react';
import {
  Modal,
  Button,
  TextInput,
  Stack,
  Group,
  Text,
  Select,
  Alert,
  Checkbox,
  Divider,
  Paper,
  Tooltip,
} from '@mantine/core';
import { IconInfoCircle } from '@tabler/icons-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { librariesApi } from '@/api/libraries';
import { notifications } from '@mantine/notifications';
import type { Library, ScanningConfig } from '@/types/api';

interface EditLibraryModalProps {
  opened: boolean;
  onClose: () => void;
  library: Library | null;
}

type ScanStrategy = 'manual' | 'auto';

// Simple cron validation - checks basic format
function isValidCron(expression: string): boolean {
  if (!expression.trim()) return false;

  const parts = expression.trim().split(/\s+/);
  // Standard cron has 5 fields (minute, hour, day, month, weekday)
  // Some systems support 6 fields (with seconds) or 7 (with year)
  if (parts.length < 5 || parts.length > 7) return false;

  // Basic validation: each part should be either a number, *, */n, n-m, or comma-separated values
  const cronPartRegex = /^(\*|(\d+|\*\/\d+|\d+-\d+)(,(\d+|\*\/\d+|\d+-\d+))*)$/;
  return parts.every(part => cronPartRegex.test(part));
}

export function EditLibraryModal({ opened, onClose, library }: EditLibraryModalProps) {
  const queryClient = useQueryClient();
  const [libraryName, setLibraryName] = useState('');
  const [libraryPath, setLibraryPath] = useState('');

  // Scanning configuration state
  const [scanStrategy, setScanStrategy] = useState<ScanStrategy>('manual');
  const [cronSchedule, setCronSchedule] = useState('0 0 * * *');
  const [autoScanOnCreate, setAutoScanOnCreate] = useState(false);
  const [scanOnStart, setScanOnStart] = useState(false);
  const [purgeDeletedOnScan, setPurgeDeletedOnScan] = useState(false);

  // Initialize form with library data when library changes
  useEffect(() => {
    if (library) {
      setLibraryName(library.name);
      setLibraryPath(library.path);

      if (!library.scanningConfig || !library.scanningConfig.enabled) {
        setScanStrategy('manual');
      } else {
        setScanStrategy('auto');
      }

      if (library.scanningConfig) {
        setCronSchedule(library.scanningConfig.cronSchedule || '0 0 * * *');
        setAutoScanOnCreate(library.scanningConfig.autoScanOnCreate);
        setScanOnStart(library.scanningConfig.scanOnStart);
        setPurgeDeletedOnScan(library.scanningConfig.purgeDeletedOnScan);
      }
    }
  }, [library]);

  // Update library mutation
  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<Library> }) =>
      librariesApi.update(id, data),
    onSuccess: () => {
      notifications.show({
        title: 'Success',
        message: 'Library updated successfully',
        color: 'green',
      });
      queryClient.invalidateQueries({ queryKey: ['libraries'] });
      handleClose();
    },
    onError: (error: Error) => {
      notifications.show({
        title: 'Error',
        message: error.message || 'Failed to update library',
        color: 'red',
      });
    },
  });

  const handleClose = () => {
    onClose();
  };

  const handleSubmit = () => {
    if (!libraryName.trim()) {
      notifications.show({
        title: 'Validation Error',
        message: 'Please enter a library name',
        color: 'red',
      });
      return;
    }

    // Validate cron schedule if auto scan is enabled
    if (scanStrategy === 'auto') {
      if (!cronSchedule.trim()) {
        notifications.show({
          title: 'Validation Error',
          message: 'Please enter a cron schedule for automatic scanning',
          color: 'red',
        });
        return;
      }

      if (!isValidCron(cronSchedule)) {
        notifications.show({
          title: 'Validation Error',
          message: 'Invalid cron expression. Please use the format: "minute hour day month weekday" (e.g., "0 0 * * *")',
          color: 'red',
        });
        return;
      }
    }

    // Build scanning config based on strategy
    const scanningConfig: ScanningConfig = {
      cronSchedule: scanStrategy === 'auto' ? cronSchedule : undefined,
      scanMode: 'normal', // Always use normal mode, deep scans are triggered manually
      autoScanOnCreate,
      enabled: scanStrategy === 'auto',
      scanOnStart,
      purgeDeletedOnScan,
    };

    if (library) {
      updateMutation.mutate({
        id: library.id,
        data: {
          name: libraryName,
          scanningConfig,
        },
      });
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={handleClose}
      title="Edit Library"
      size="lg"
      centered
      zIndex={1000}
      overlayProps={{
        backgroundOpacity: 0.55,
        blur: 3,
      }}
    >
      <Stack gap="md">
        <TextInput
          label="Library Name"
          placeholder="Enter library name"
          required
          value={libraryName}
          onChange={(e) => setLibraryName(e.currentTarget.value)}
        />

        <TextInput
          label="Library Path"
          placeholder="Path to library"
          value={libraryPath}
          readOnly
          disabled
          description="Library path cannot be changed after creation"
        />

        <Divider label="Scanning Configuration" labelPosition="left" mt="md" />

        <Paper p="md" withBorder>
          <Stack gap="md">
            <Select
              label="Scan Strategy"
              description="How this library should be scanned"
              data={[
                {
                  value: 'manual',
                  label: 'Manual - Trigger scans on demand',
                },
                {
                  value: 'auto',
                  label: 'Automatic - Scheduled scanning',
                },
              ]}
              value={scanStrategy}
              onChange={(value) => setScanStrategy(value as ScanStrategy)}
              required
              comboboxProps={{ zIndex: 1001 }}
            />

            <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
              {scanStrategy === 'manual' && 'Trigger normal or deep scans manually from the library dashboard. No automatic scanning will occur.'}
              {scanStrategy === 'auto' && 'Library will be scanned automatically (normal mode) according to the cron schedule below. You can still trigger manual deep scans.'}
            </Alert>

            {scanStrategy === 'auto' && (
              <>
                <TextInput
                  label="Cron Schedule"
                  description="Cron expression for automatic scanning (e.g., '0 0 * * *' for daily at midnight)"
                  placeholder="0 0 * * *"
                  value={cronSchedule}
                  onChange={(e) => setCronSchedule(e.currentTarget.value)}
                  required
                  error={cronSchedule && !isValidCron(cronSchedule) ? 'Invalid cron expression' : undefined}
                  rightSection={
                    <Tooltip label="Common: '0 */6 * * *' (every 6 hours), '0 0 * * 0' (weekly on Sunday)">
                      <IconInfoCircle size={16} style={{ cursor: 'help' }} />
                    </Tooltip>
                  }
                />
                <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
                  <Text size="xs">
                    <strong>Cron format:</strong> minute hour day month weekday<br />
                    Examples:<br />
                    • <code>0 0 * * *</code> - Daily at midnight<br />
                    • <code>0 */6 * * *</code> - Every 6 hours<br />
                    • <code>0 0 * * 0</code> - Weekly on Sunday<br />
                    • <code>0 2 * * 1-5</code> - Weekdays at 2 AM
                  </Text>
                </Alert>
              </>
            )}

            <Stack gap="xs">
              <Text size="sm" fw={500}>Additional Options</Text>

              <Checkbox
                label="Scan immediately after creation"
                description="Start scanning this library as soon as it's created (normal scan)"
                checked={autoScanOnCreate}
                onChange={(e) => setAutoScanOnCreate(e.currentTarget.checked)}
              />

              <Checkbox
                label="Scan on application start"
                description="Automatically scan this library when the server starts (normal scan)"
                checked={scanOnStart}
                onChange={(e) => setScanOnStart(e.currentTarget.checked)}
              />

              <Checkbox
                label="Purge deleted items after scan"
                description="Remove database entries for files that no longer exist on disk"
                checked={purgeDeletedOnScan}
                onChange={(e) => setPurgeDeletedOnScan(e.currentTarget.checked)}
              />
            </Stack>
          </Stack>
        </Paper>

        <Group justify="flex-end" mt="md">
          <Button variant="subtle" onClick={handleClose}>
            Cancel
          </Button>
          <Button
            onClick={handleSubmit}
            loading={updateMutation.isPending}
            disabled={!libraryName}
          >
            Save Changes
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

