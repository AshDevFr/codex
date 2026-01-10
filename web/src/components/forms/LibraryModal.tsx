import {
	Alert,
	Anchor,
	Breadcrumbs,
	Button,
	Center,
	Checkbox,
	Divider,
	Group,
	Loader,
	Modal,
	MultiSelect,
	Paper,
	ScrollArea,
	Select,
	Stack,
	Text,
	Textarea,
	TextInput,
	UnstyledButton,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconChevronRight,
	IconFolder,
	IconFolderOpen,
	IconHome,
	IconInfoCircle,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import type {
	CreateLibraryRequest,
	FileSystemEntry,
	Library,
	ScanningConfig,
} from "@/types/api";
import { CronInput } from "./CronInput";

interface LibraryModalProps {
	opened: boolean;
	onClose: (createdLibrary?: Library) => void;
	library?: Library | null; // If provided, we're in edit mode; otherwise, add mode
}

type ScanStrategy = "manual" | "auto";

const ALL_FORMATS = ["CBZ", "CBR", "EPUB", "PDF"];

export function LibraryModal({ opened, onClose, library }: LibraryModalProps) {
	const isEditMode = !!library;
	const queryClient = useQueryClient();
	const [currentPath, setCurrentPath] = useState<string | null>(null);
	const [libraryName, setLibraryName] = useState("");
	const [libraryPath, setLibraryPath] = useState("");
	const [selectedPath, setSelectedPath] = useState("");
	const [showPathBrowser, setShowPathBrowser] = useState(false);

	// Scanning configuration state
	const [scanStrategy, setScanStrategy] = useState<ScanStrategy>("manual");
	const [cronSchedule, setCronSchedule] = useState("0 0 * * *");
	const [autoScanOnCreate, setAutoScanOnCreate] = useState(false);
	const [scanOnStart, setScanOnStart] = useState(false);
	const [purgeDeletedOnScan, setPurgeDeletedOnScan] = useState(false);

	// Format filtering state
	const [allowedFormats, setAllowedFormats] = useState<string[]>(ALL_FORMATS);
	const [excludedPatterns, setExcludedPatterns] = useState("");

	// Load drives when modal opens (only for add mode)
	const { data: drives, isLoading: drivesLoading } = useQuery({
		queryKey: ["drives"],
		queryFn: filesystemApi.getDrives,
		enabled: opened && !isEditMode,
	});

	// Browse current directory (only for add mode)
	const {
		data: browseData,
		isLoading: browseLoading,
		error: browseError,
	} = useQuery({
		queryKey: ["browse", currentPath],
		queryFn: () => filesystemApi.browse(currentPath || undefined),
		enabled: currentPath !== null && !isEditMode,
	});

	// Initialize form with library data when library changes (edit mode)
	useEffect(() => {
		if (isEditMode && library) {
			setLibraryName(library.name);
			setLibraryPath(library.path);

			if (!library.scanningConfig || !library.scanningConfig.enabled) {
				setScanStrategy("manual");
			} else {
				setScanStrategy("auto");
			}

			if (library.scanningConfig) {
				setCronSchedule(library.scanningConfig.cronSchedule || "0 0 * * *");
				setScanOnStart(library.scanningConfig.scanOnStart);
				setPurgeDeletedOnScan(library.scanningConfig.purgeDeletedOnScan);
			}

			setAllowedFormats(
				library.allowedFormats && library.allowedFormats.length > 0
					? library.allowedFormats
					: ALL_FORMATS,
			);
			setExcludedPatterns(library.excludedPatterns || "");
		} else if (!isEditMode) {
			// Reset form for add mode
			setLibraryName("");
			setSelectedPath("");
			setLibraryPath("");
			setCurrentPath(null);
			setShowPathBrowser(false);
			setScanStrategy("manual");
			setCronSchedule("0 0 * * *");
			setAutoScanOnCreate(false); // Used for scanImmediately parameter, not in scanningConfig
			setScanOnStart(false);
			setPurgeDeletedOnScan(false);
			setAllowedFormats(ALL_FORMATS);
			setExcludedPatterns("");
		}
	}, [library, isEditMode]);

	// Create library mutation
	const createMutation = useMutation({
		mutationFn: (request: CreateLibraryRequest) => librariesApi.create(request),
		onSuccess: (createdLibrary) => {
			notifications.show({
				title: "Success",
				message: "Library created successfully",
				color: "green",
			});
			// Use refetchQueries to force immediate refetch, bypassing staleTime
			queryClient.refetchQueries({ queryKey: ["libraries"] });
			handleClose(createdLibrary);
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to create library",
				color: "red",
			});
		},
	});

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
			// Use refetchQueries to force immediate refetch, bypassing staleTime
			queryClient.refetchQueries({ queryKey: ["libraries"] });
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

	const handleClose = (createdLibrary?: Library) => {
		onClose(createdLibrary);
	};

	const handleDriveSelect = (entry: FileSystemEntry) => {
		setCurrentPath(entry.path);
		setShowPathBrowser(true);
	};

	const handleDirectoryClick = (entry: FileSystemEntry) => {
		if (entry.is_directory) {
			setCurrentPath(entry.path);
		}
	};

	const handleSelectCurrentPath = () => {
		if (browseData) {
			setSelectedPath(browseData.current_path);
			setLibraryPath(browseData.current_path);
			setShowPathBrowser(false);

			// Auto-generate library name from path if empty
			if (!libraryName) {
				const pathParts = browseData.current_path.split(/[/\\]/);
				const folderName =
					pathParts[pathParts.length - 1] || pathParts[pathParts.length - 2];
				if (folderName) {
					setLibraryName(folderName);
				}
			}
		}
	};

	const handleNavigateToParent = () => {
		if (browseData?.parent_path) {
			setCurrentPath(browseData.parent_path);
		}
	};

	const handleBreadcrumbClick = (path: string) => {
		setCurrentPath(path);
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

		if (isEditMode) {
			// Edit mode validation
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
		} else {
			// Add mode validation
			const pathToUse = selectedPath || libraryPath;
			if (!pathToUse.trim()) {
				notifications.show({
					title: "Validation Error",
					message: "Please select a library path",
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
			const scanningConfig: ScanningConfig | undefined = {
				cronSchedule: scanStrategy === "auto" ? cronSchedule : undefined,
				scanMode: "normal", // Always use normal mode, deep scans are triggered manually
				enabled: scanStrategy === "auto",
				scanOnStart,
				purgeDeletedOnScan,
			};

			createMutation.mutate({
				name: libraryName,
				path: pathToUse,
				scanningConfig,
				scanImmediately: autoScanOnCreate,
				allowedFormats: allowedFormats.length > 0 ? allowedFormats : undefined,
				excludedPatterns: excludedPatterns.trim() || undefined,
			});
		}
	};

	// Generate breadcrumbs from current path
	const breadcrumbs = currentPath
		? currentPath
				.split(/[/\\]/)
				.filter(Boolean)
				.map((part, index, arr) => {
					const path = arr.slice(0, index + 1).join("/");
					return { label: part, path: `/${path}` };
				})
		: [];

	// Don't render if in edit mode and library is null
	if (isEditMode && !library) {
		return null;
	}

	const isLoading = isEditMode
		? updateMutation.isPending
		: createMutation.isPending;
	const submitButtonText = isEditMode ? "Save Changes" : "Create Library";
	const modalTitle = isEditMode ? "Edit Library" : "Add New Library";
	const modalSize = isEditMode ? "lg" : "xl";
	const currentPathValue = isEditMode ? libraryPath : selectedPath;

	return (
		<Modal
			opened={opened}
			onClose={handleClose}
			title={modalTitle}
			size={modalSize}
			centered
			zIndex={1000}
			overlayProps={{
				backgroundOpacity: 0.55,
				blur: 3,
			}}
		>
			<Stack gap="md">
				{!isEditMode && showPathBrowser ? (
					<>
						{/* Path Browser - only for add mode */}
						<Stack gap="xs">
							<Group justify="space-between">
								<Text fw={500}>Select Library Path</Text>
								<Button
									size="xs"
									variant="subtle"
									onClick={() => setShowPathBrowser(false)}
								>
									Back to Form
								</Button>
							</Group>

							{currentPath === null ? (
								// Show drives
								<>
									<Text size="sm" c="dimmed">
										Select a drive or location to browse:
									</Text>
									{drivesLoading ? (
										<Center h={200}>
											<Loader />
										</Center>
									) : (
										<ScrollArea h={400} type="auto">
											<Stack gap={6}>
												{drives?.map((drive) => (
													<UnstyledButton
														key={drive.path}
														onClick={() => handleDriveSelect(drive)}
														p="xs"
														style={{
															borderRadius: "4px",
															border: "1px solid var(--mantine-color-gray-3)",
															"&:hover": {
																backgroundColor: "var(--mantine-color-gray-1)",
															},
														}}
													>
														<Group gap="xs">
															<IconFolder size={18} />
															<div>
																<Text size="sm" fw={500}>
																	{drive.name}
																</Text>
																<Text size="xs" c="dimmed">
																	{drive.path}
																</Text>
															</div>
														</Group>
													</UnstyledButton>
												))}
											</Stack>
										</ScrollArea>
									)}
								</>
							) : (
								// Show directory contents
								<>
									{/* Breadcrumbs */}
									<Breadcrumbs separator={<IconChevronRight size={14} />}>
										<Anchor
											size="sm"
											onClick={() => setCurrentPath(null)}
											style={{ cursor: "pointer" }}
										>
											<Group gap={4}>
												<IconHome size={16} />
												<span>Drives</span>
											</Group>
										</Anchor>
										{breadcrumbs.map((crumb) => (
											<Anchor
												key={crumb.path}
												size="sm"
												onClick={() => handleBreadcrumbClick(crumb.path)}
												style={{ cursor: "pointer" }}
											>
												{crumb.label}
											</Anchor>
										))}
									</Breadcrumbs>

									<Group justify="space-between">
										<Button
											size="xs"
											variant="light"
											leftSection={<IconFolder size={16} />}
											onClick={handleSelectCurrentPath}
										>
											Select This Folder
										</Button>
										<Button
											size="xs"
											variant="subtle"
											onClick={handleNavigateToParent}
											disabled={!browseData?.parent_path}
										>
											Up One Level
										</Button>
									</Group>

									{browseError && (
										<Alert icon={<IconAlertCircle size={16} />} color="red">
											Failed to browse directory. Please check permissions.
										</Alert>
									)}

									{browseLoading ? (
										<Center h={200}>
											<Loader />
										</Center>
									) : (
										<ScrollArea h={400} type="auto">
											<Stack gap={6}>
												{browseData?.entries
													.filter((entry) => entry.is_directory)
													.map((entry) => (
														<UnstyledButton
															key={entry.path}
															onClick={() => handleDirectoryClick(entry)}
															p="xs"
															style={{
																borderRadius: "4px",
																border: "1px solid var(--mantine-color-gray-3)",
															}}
														>
															<Group gap="xs">
																<IconFolderOpen size={18} />
																<Text size="sm">{entry.name}</Text>
															</Group>
														</UnstyledButton>
													))}
											</Stack>
										</ScrollArea>
									)}

									<Text size="xs" c="dimmed">
										Current: {browseData?.current_path}
									</Text>
								</>
							)}
						</Stack>
					</>
				) : (
					<>
						{/* Main Form */}
						<TextInput
							label="Library Name"
							placeholder="Enter library name"
							required
							value={libraryName}
							onChange={(e) => setLibraryName(e.currentTarget.value)}
						/>

						{isEditMode ? (
							<TextInput
								label="Library Path"
								placeholder="Path to library"
								value={libraryPath}
								readOnly
								disabled
								description="Library path cannot be changed after creation"
							/>
						) : (
							<TextInput
								label="Library Path"
								placeholder="Select a path..."
								required
								value={selectedPath}
								readOnly
								rightSection={
									<Button
										size="xs"
										variant="subtle"
										onClick={() => setShowPathBrowser(true)}
									>
										Browse
									</Button>
								}
								styles={{ input: { paddingRight: 80 } }}
							/>
						)}

						<Divider label="Format Filtering" labelPosition="left" mt="md" />

						<Paper p="md" withBorder>
							<Stack gap="md">
								<MultiSelect
									label="Allowed Formats"
									description="Select file formats to include in this library. Leave empty to allow all formats."
									placeholder="Select formats (leave empty for all)"
									data={[
										{
											value: "CBZ",
											label: "CBZ (Comic Book ZIP)",
										},
										{
											value: "CBR",
											label: "CBR (Comic Book RAR)",
										},
										{
											value: "EPUB",
											label: "EPUB (Ebook)",
										},
										{
											value: "PDF",
											label: "PDF (Portable Document Format)",
										},
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

						<Divider
							label="Scanning Configuration"
							labelPosition="left"
							mt="md"
						/>

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

									{!isEditMode && (
										<Checkbox
											label="Scan immediately after creation"
											description="Start scanning this library as soon as it's created (normal scan)"
											checked={autoScanOnCreate}
											onChange={(e) =>
												setAutoScanOnCreate(e.currentTarget.checked)
											}
										/>
									)}

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
										onChange={(e) =>
											setPurgeDeletedOnScan(e.currentTarget.checked)
										}
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
								loading={isLoading}
								disabled={!libraryName || (!isEditMode && !currentPathValue)}
							>
								{submitButtonText}
							</Button>
						</Group>
					</>
				)}
			</Stack>
		</Modal>
	);
}
