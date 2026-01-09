import {
	Alert,
	Button,
	Checkbox,
	Divider,
	Group,
	Modal,
	MultiSelect,
	Paper,
	Select,
	Stack,
	Text,
	Textarea,
	TextInput,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconInfoCircle } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { librariesApi } from "@/api/libraries";
import type { Library, ScanningConfig } from "@/types/api";
import { CronInput } from "./CronInput";

interface EditLibraryModalProps {
	opened: boolean;
	onClose: () => void;
	library: Library | null;
}

type ScanStrategy = "manual" | "auto";

const ALL_FORMATS = ["CBZ", "CBR", "EPUB", "PDF"];

export function EditLibraryModal({
	opened,
	onClose,
	library,
}: EditLibraryModalProps) {
	const queryClient = useQueryClient();
	const [libraryName, setLibraryName] = useState("");
	const [libraryPath, setLibraryPath] = useState("");

	// Scanning configuration state
	const [scanStrategy, setScanStrategy] = useState<ScanStrategy>("manual");
	const [cronSchedule, setCronSchedule] = useState("0 0 * * *");
	const [autoScanOnCreate, setAutoScanOnCreate] = useState(false);
	const [scanOnStart, setScanOnStart] = useState(false);
	const [purgeDeletedOnScan, setPurgeDeletedOnScan] = useState(false);

	// Format filtering state
	const [allowedFormats, setAllowedFormats] = useState<string[]>(ALL_FORMATS);
	const [excludedPatterns, setExcludedPatterns] = useState("");

	// Initialize form with library data when library changes
	useEffect(() => {
		if (library) {
			setLibraryName(library.name);
			setLibraryPath(library.path);

			if (!library.scanningConfig || !library.scanningConfig.enabled) {
				setScanStrategy("manual");
			} else {
				setScanStrategy("auto");
			}

			if (library.scanningConfig) {
				setCronSchedule(library.scanningConfig.cronSchedule || "0 0 * * *");
				setAutoScanOnCreate(library.scanningConfig.autoScanOnCreate);
				setScanOnStart(library.scanningConfig.scanOnStart);
				setPurgeDeletedOnScan(library.scanningConfig.purgeDeletedOnScan);
			}

			setAllowedFormats(
				library.allowedFormats && library.allowedFormats.length > 0
					? library.allowedFormats
					: ALL_FORMATS,
			);
			setExcludedPatterns(library.excludedPatterns || "");
		}
	}, [library]);

	// Update library mutation
	const updateMutation = useMutation({
		mutationFn: ({ id, data }: { id: string; data: Partial<Library> }) =>
			librariesApi.update(id, data),
		onSuccess: () => {
			notifications.show({
				title: "Success",
				message: "Library updated successfully",
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["libraries"] });
			handleClose();
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update library",
				color: "red",
			});
		},
	});

	const handleClose = () => {
		onClose();
	};

	const handleSubmit = () => {
		if (!libraryName.trim()) {
			notifications.show({
				title: "Validation Error",
				message: "Please enter a library name",
				color: "red",
			});
			return;
		}

		// Validate cron schedule if auto scan is enabled
		if (scanStrategy === "auto") {
			if (!cronSchedule.trim()) {
				notifications.show({
					title: "Validation Error",
					message: "Please enter a cron schedule for automatic scanning",
					color: "red",
				});
				return;
			}
		}

		// Build scanning config based on strategy
		const scanningConfig: ScanningConfig = {
			cronSchedule: scanStrategy === "auto" ? cronSchedule : undefined,
			scanMode: "normal", // Always use normal mode, deep scans are triggered manually
			autoScanOnCreate,
			enabled: scanStrategy === "auto",
			scanOnStart,
			purgeDeletedOnScan,
		};

		if (library) {
			updateMutation.mutate({
				id: library.id,
				data: {
					name: libraryName,
					scanningConfig,
					allowedFormats:
						allowedFormats.length > 0 ? allowedFormats : undefined,
					excludedPatterns: excludedPatterns.trim() || undefined,
				},
			});
		}
	};

	// Don't render if library is null
	if (!library) {
		return null;
	}

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

				<Divider label="Format Filtering" labelPosition="left" mt="md" />

				<Paper p="md" withBorder>
					<Stack gap="md">
						<MultiSelect
							label="Allowed Formats"
							description="Select file formats to include in this library. Leave empty to allow all formats."
							placeholder="Select formats (leave empty for all)"
							data={[
								{ value: "CBZ", label: "CBZ (Comic Book ZIP)" },
								{ value: "CBR", label: "CBR (Comic Book RAR)" },
								{ value: "EPUB", label: "EPUB (Ebook)" },
								{ value: "PDF", label: "PDF (Portable Document Format)" },
							]}
							value={allowedFormats}
							onChange={setAllowedFormats}
							clearable
							comboboxProps={{ zIndex: 1001 }}
						/>

						<Textarea
							label="Excluded Patterns"
							description="File or directory patterns to exclude (one per line). Examples: .DS_Store, Thumbs.db, @eaDir/*"
							placeholder=".DS_Store&#10;Thumbs.db&#10;@eaDir/*"
							value={excludedPatterns}
							onChange={(e) => setExcludedPatterns(e.currentTarget.value)}
							minRows={3}
							autosize
						/>
					</Stack>
				</Paper>

				<Divider label="Scanning Configuration" labelPosition="left" mt="md" />

				<Paper p="md" withBorder>
					<Stack gap="md">
						<Select
							label="Scan Strategy"
							description="How this library should be scanned"
							data={[
								{
									value: "manual",
									label: "Manual - Trigger scans on demand",
								},
								{
									value: "auto",
									label: "Automatic - Scheduled scanning",
								},
							]}
							value={scanStrategy}
							onChange={(value) => setScanStrategy(value as ScanStrategy)}
							required
							comboboxProps={{ zIndex: 1001 }}
						/>

						<Alert
							icon={<IconInfoCircle size={16} />}
							color="blue"
							variant="light"
						>
							{scanStrategy === "manual" &&
								"Trigger normal or deep scans manually from the library dashboard. No automatic scanning will occur."}
							{scanStrategy === "auto" &&
								"Library will be scanned automatically (normal mode) according to the cron schedule below. You can still trigger manual deep scans."}
						</Alert>

						{scanStrategy === "auto" && (
							<CronInput
								label="Cron Schedule"
								description="Cron expression for automatic scanning (e.g., '0 0 * * *' for daily at midnight)"
								placeholder="0 0 * * *"
								value={cronSchedule}
								onChange={setCronSchedule}
								required
							/>
						)}

						<Stack gap="xs">
							<Text size="sm" fw={500}>
								Additional Options
							</Text>

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
