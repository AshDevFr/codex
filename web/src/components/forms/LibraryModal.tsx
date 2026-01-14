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
	Tabs,
	Text,
	Textarea,
	TextInput,
	UnstyledButton,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconChevronRight,
	IconFilter,
	IconFolder,
	IconFolderOpen,
	IconHome,
	IconInfoCircle,
	IconRefresh,
	IconSettings,
	IconWand,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capitalize } from "es-toolkit/string";
import { useEffect, useState } from "react";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import type {
	BookStrategy,
	CreateLibraryRequest,
	FileSystemEntry,
	Library,
	NumberStrategy,
	ScanningConfig,
	SeriesStrategy,
} from "@/types";
import { CronInput } from "./CronInput";
import { PreviewScanPanel } from "./PreviewScanPanel";
import {
	BookStrategySelector,
	NumberStrategySelector,
	SeriesStrategySelector,
} from "./StrategySelector";

interface LibraryModalProps {
	opened: boolean;
	onClose: (createdLibrary?: Library) => void;
	library?: Library | null; // If provided, we're in edit mode; otherwise, add mode
}

type ScanStrategy = "manual" | "auto";

const ALL_FORMATS = ["CBZ", "CBR", "EPUB", "PDF"];

const READING_DIRECTIONS = [
	{ value: "ltr", label: "Left to Right (Books & Comics)" },
	{ value: "rtl", label: "Right to Left (Manga)" },
	{ value: "ttb", label: "Vertical" },
	{ value: "webtoon", label: "Webtoon" },
];

export function LibraryModal({ opened, onClose, library }: LibraryModalProps) {
	const isEditMode = !!library;
	const queryClient = useQueryClient();
	const [currentPath, setCurrentPath] = useState<string | null>(null);
	const [libraryName, setLibraryName] = useState("");
	const [libraryPath, setLibraryPath] = useState("");
	const [selectedPath, setSelectedPath] = useState("");
	const [showPathBrowser, setShowPathBrowser] = useState(false);
	const [activeTab, setActiveTab] = useState<string | null>("general");

	// Reading direction state
	const [readingDirection, setReadingDirection] = useState("ltr");

	// Scanning configuration state
	const [scanStrategy, setScanStrategy] = useState<ScanStrategy>("manual");
	const [cronSchedule, setCronSchedule] = useState("0 0 * * *");
	const [autoScanOnCreate, setAutoScanOnCreate] = useState(false);
	const [scanOnStart, setScanOnStart] = useState(false);
	const [purgeDeletedOnScan, setPurgeDeletedOnScan] = useState(false);

	// Format filtering state
	const [allowedFormats, setAllowedFormats] = useState<string[]>(ALL_FORMATS);
	const [excludedPatterns, setExcludedPatterns] = useState("");

	// Strategy state (only used in add mode - series strategy is immutable after creation)
	const [seriesStrategy, setSeriesStrategy] =
		useState<SeriesStrategy>("series_volume");
	const [seriesConfig, setSeriesConfig] = useState<Record<string, unknown>>({});
	const [bookStrategy, setBookStrategy] = useState<BookStrategy>("filename");
	const [numberStrategy, setNumberStrategy] =
		useState<NumberStrategy>("file_order");

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
	// or reset form when modal opens in add mode
	useEffect(() => {
		if (!opened) return;

		if (isEditMode && library) {
			setLibraryName(library.name);
			setLibraryPath(library.path);
			setReadingDirection(library.defaultReadingDirection || "ltr");

			if (!library.scanningConfig || !library.scanningConfig.enabled) {
				setScanStrategy("manual");
			} else {
				setScanStrategy("auto");
			}

			if (library.scanningConfig) {
				setCronSchedule(library.scanningConfig.cronSchedule || "0 0 * * *");
				setScanOnStart(library.scanningConfig.scanOnStart ?? false);
				setPurgeDeletedOnScan(
					library.scanningConfig.purgeDeletedOnScan ?? false,
				);
			}

			setAllowedFormats(
				library.allowedFormats && library.allowedFormats.length > 0
					? library.allowedFormats
					: ALL_FORMATS,
			);
			setExcludedPatterns(library.excludedPatterns || "");

			// Initialize strategy state from library (series strategy is read-only in edit mode)
			setSeriesStrategy(library.seriesStrategy || "series_volume");
			setSeriesConfig((library.seriesConfig as Record<string, unknown>) || {});
			setBookStrategy(library.bookStrategy || "filename");
			setNumberStrategy(library.numberStrategy || "file_order");
		} else if (!isEditMode) {
			// Reset form for add mode
			setLibraryName("");
			setSelectedPath("");
			setLibraryPath("");
			setCurrentPath(null);
			setShowPathBrowser(false);
			setActiveTab("general");
			setReadingDirection("ltr");
			setScanStrategy("manual");
			setCronSchedule("0 0 * * *");
			setAutoScanOnCreate(false);
			setScanOnStart(false);
			setPurgeDeletedOnScan(false);
			setAllowedFormats(ALL_FORMATS);
			setExcludedPatterns("");
			// Reset strategy state
			setSeriesStrategy("series_volume");
			setSeriesConfig({});
			setBookStrategy("filename");
			setNumberStrategy("file_order");
		}
	}, [opened, library, isEditMode]);

	// Create library mutation
	const createMutation = useMutation({
		mutationFn: (request: CreateLibraryRequest) => librariesApi.create(request),
		onSuccess: (createdLibrary) => {
			notifications.show({
				title: "Success",
				message: "Library created successfully",
				color: "green",
			});
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
					setLibraryName(capitalize(folderName));
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
			setActiveTab("general");
			return;
		}

		if (isEditMode) {
			// Edit mode validation
			if (scanStrategy === "auto") {
				if (!cronSchedule.trim()) {
					notifications.show({
						title: "Validation Error",
						message: "Please enter a cron schedule for automatic scanning",
						color: "red",
					});
					setActiveTab("scanning");
					return;
				}
			}

			// Build scanning config based on strategy
			const scanningConfig: ScanningConfig = {
				cronSchedule: scanStrategy === "auto" ? cronSchedule : undefined,
				scanMode: "normal",
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
						defaultReadingDirection: readingDirection,
						// Book naming and number strategies can be changed in edit mode
						bookStrategy,
						numberStrategy,
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
				setActiveTab("general");
				return;
			}

			if (scanStrategy === "auto") {
				if (!cronSchedule.trim()) {
					notifications.show({
						title: "Validation Error",
						message: "Please enter a cron schedule for automatic scanning",
						color: "red",
					});
					setActiveTab("scanning");
					return;
				}
			}

			// Build scanning config based on strategy
			const scanningConfig: ScanningConfig | undefined = {
				cronSchedule: scanStrategy === "auto" ? cronSchedule : undefined,
				scanMode: "normal",
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
				defaultReadingDirection: readingDirection,
				// Include strategy configuration
				seriesStrategy,
				seriesConfig:
					Object.keys(seriesConfig).length > 0 ? seriesConfig : undefined,
				bookStrategy,
				numberStrategy,
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
	const currentPathValue = isEditMode ? libraryPath : selectedPath;

	// Path browser view (only for add mode)
	const renderPathBrowser = () => (
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
	);

	// General tab content
	const renderGeneralTab = () => (
		<Stack gap="md">
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
					onChange={(e) => setSelectedPath(e.currentTarget.value)}
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

			<Select
				label="Default Reading Direction"
				description="The default reading direction for books in this library"
				data={READING_DIRECTIONS}
				value={readingDirection}
				onChange={(value) => setReadingDirection(value || "ltr")}
				comboboxProps={{ zIndex: 1001 }}
			/>
		</Stack>
	);

	// Formats tab content
	const renderFormatsTab = () => (
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
	);

	// Scanning tab content
	const renderScanningTab = () => (
		<Stack gap="md">
			<Select
				label="Scan Strategy"
				description="How this library should be scanned"
				data={[
					{ value: "manual", label: "Manual - Trigger scans on demand" },
					{ value: "auto", label: "Automatic - Scheduled scanning" },
				]}
				value={scanStrategy}
				onChange={(value) => setScanStrategy(value as ScanStrategy)}
				required
				comboboxProps={{ zIndex: 1001 }}
			/>

			<Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
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

			<Paper p="md" withBorder>
				<Stack gap="xs">
					<Text size="sm" fw={500}>
						Additional Options
					</Text>

					{!isEditMode && (
						<Checkbox
							label="Scan immediately after creation"
							description="Start scanning this library as soon as it's created (normal scan)"
							checked={autoScanOnCreate}
							onChange={(e) => setAutoScanOnCreate(e.currentTarget.checked)}
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
						onChange={(e) => setPurgeDeletedOnScan(e.currentTarget.checked)}
					/>
				</Stack>
			</Paper>
		</Stack>
	);

	// Strategy tab content
	const renderStrategyTab = () => {
		return (
			<Stack gap="md">
				{isEditMode ? (
					<Alert
						icon={<IconInfoCircle size={16} />}
						color="blue"
						variant="light"
					>
						<Text size="sm">
							The <strong>series detection strategy</strong> cannot be changed
							after library creation. You can modify book naming and numbering
							strategies - changes will apply on the next scan.
						</Text>
					</Alert>
				) : (
					<Alert
						icon={<IconInfoCircle size={16} />}
						color="yellow"
						variant="light"
					>
						<Text size="sm">
							The <strong>series detection strategy</strong> is permanent and
							cannot be changed after library creation. Choose carefully based
							on your folder structure.
						</Text>
					</Alert>
				)}

				<SeriesStrategySelector
					value={seriesStrategy}
					onChange={setSeriesStrategy}
					config={seriesConfig}
					onConfigChange={setSeriesConfig}
					disabled={isEditMode}
				/>

				<Divider my="sm" />

				<BookStrategySelector value={bookStrategy} onChange={setBookStrategy} />

				<Divider my="sm" />

				<NumberStrategySelector
					value={numberStrategy}
					onChange={setNumberStrategy}
				/>

				{!isEditMode && (
					<>
						<Divider my="sm" />

						<PreviewScanPanel
							path={selectedPath || libraryPath}
							seriesStrategy={seriesStrategy}
							seriesConfig={seriesConfig}
						/>
					</>
				)}
			</Stack>
		);
	};

	return (
		<Modal
			opened={opened}
			onClose={handleClose}
			title={modalTitle}
			size="lg"
			centered
			zIndex={1000}
			overlayProps={{
				backgroundOpacity: 0.55,
				blur: 3,
			}}
		>
			<Stack gap="md">
				{!isEditMode && showPathBrowser ? (
					renderPathBrowser()
				) : (
					<>
						<Tabs value={activeTab} onChange={setActiveTab}>
							<Tabs.List>
								<Tabs.Tab
									value="general"
									leftSection={<IconSettings size={16} />}
								>
									General
								</Tabs.Tab>
								<Tabs.Tab value="strategy" leftSection={<IconWand size={16} />}>
									Strategy
								</Tabs.Tab>
								<Tabs.Tab
									value="formats"
									leftSection={<IconFilter size={16} />}
								>
									Formats
								</Tabs.Tab>
								<Tabs.Tab
									value="scanning"
									leftSection={<IconRefresh size={16} />}
								>
									Scanning
								</Tabs.Tab>
							</Tabs.List>

							<Tabs.Panel value="general" pt="md">
								{renderGeneralTab()}
							</Tabs.Panel>

							<Tabs.Panel value="strategy" pt="md">
								{renderStrategyTab()}
							</Tabs.Panel>

							<Tabs.Panel value="formats" pt="md">
								{renderFormatsTab()}
							</Tabs.Panel>

							<Tabs.Panel value="scanning" pt="md">
								{renderScanningTab()}
							</Tabs.Panel>
						</Tabs>

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
