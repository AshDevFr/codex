import {
	ActionIcon,
	Badge,
	Code,
	CopyButton,
	Group,
	Modal,
	Paper,
	Stack,
	Text,
	Tooltip,
} from "@mantine/core";
import {
	IconCheck,
	IconCopy,
	IconExternalLink,
	IconInfoCircle,
} from "@tabler/icons-react";
import type { FullSeries } from "@/types";

export interface SeriesInfoModalProps {
	opened: boolean;
	onClose: () => void;
	series: FullSeries;
}

function formatDateTime(dateString: string): string {
	return new Date(dateString).toLocaleString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function formatSourceName(source: string): string {
	// Handle plugin sources: "plugin:mangabaka" -> "Mangabaka"
	if (source.startsWith("plugin:")) {
		const pluginName = source.slice(7);
		return pluginName.charAt(0).toUpperCase() + pluginName.slice(1);
	}
	// Handle known sources
	switch (source) {
		case "comicinfo":
			return "ComicInfo";
		case "epub":
			return "EPUB";
		case "manual":
			return "Manual";
		default:
			return source.charAt(0).toUpperCase() + source.slice(1);
	}
}

interface InfoRowProps {
	label: string;
	value: string | number | null | undefined;
	copyable?: boolean;
	monospace?: boolean;
}

function InfoRow({ label, value, copyable, monospace }: InfoRowProps) {
	if (value === null || value === undefined || value === "") return null;

	const displayValue = String(value);

	// For copyable monospace values (path, IDs), show inline with copy button
	if (copyable && monospace) {
		return (
			<Group justify="space-between" wrap="nowrap" gap="md">
				<Text size="sm" c="dimmed" style={{ flexShrink: 0 }}>
					{label}
				</Text>
				<Group gap="xs" wrap="nowrap" style={{ minWidth: 0 }}>
					<Code
						style={{
							wordBreak: "break-all",
							whiteSpace: "normal",
						}}
					>
						{displayValue}
					</Code>
					<CopyButton value={displayValue}>
						{({ copied, copy }) => (
							<Tooltip
								label={copied ? "Copied" : "Copy"}
								withArrow
								zIndex={1100}
							>
								<ActionIcon
									size="xs"
									variant="subtle"
									color={copied ? "teal" : "gray"}
									onClick={copy}
									style={{ flexShrink: 0 }}
								>
									{copied ? <IconCheck size={14} /> : <IconCopy size={14} />}
								</ActionIcon>
							</Tooltip>
						)}
					</CopyButton>
				</Group>
			</Group>
		);
	}

	return (
		<Group justify="space-between" wrap="nowrap" gap="md">
			<Text size="sm" c="dimmed" style={{ flexShrink: 0 }}>
				{label}
			</Text>
			<Text size="sm" fw={500} style={{ textAlign: "right" }}>
				{displayValue}
			</Text>
		</Group>
	);
}

export function SeriesInfoModal({
	opened,
	onClose,
	series,
}: SeriesInfoModalProps) {
	const metadata = series.metadata;

	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title={
				<Group gap="xs">
					<IconInfoCircle size={20} />
					<Text fw={500}>Series Information</Text>
				</Group>
			}
			size="lg"
			centered
			zIndex={1000}
			overlayProps={{
				backgroundOpacity: 0.55,
				blur: 3,
			}}
		>
			<Stack gap="md">
				{/* Basic Info */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Basic Information
						</Text>
						<InfoRow label="Title" value={metadata?.title} />
						{metadata?.titleSort && (
							<InfoRow label="Sort Title" value={metadata.titleSort} />
						)}
						<InfoRow label="Library" value={series.libraryName} />
						<InfoRow label="Book Count" value={series.bookCount} />
						{series.unreadCount !== null &&
							series.unreadCount !== undefined && (
								<InfoRow label="Unread" value={series.unreadCount} />
							)}
						{metadata?.year && <InfoRow label="Year" value={metadata.year} />}
						{metadata?.status && (
							<InfoRow
								label="Status"
								value={
									metadata.status.charAt(0).toUpperCase() +
									metadata.status.slice(1)
								}
							/>
						)}
						{metadata?.readingDirection && (
							<InfoRow
								label="Reading Direction"
								value={
									metadata.readingDirection === "ltr"
										? "Left to Right"
										: metadata.readingDirection === "rtl"
											? "Right to Left"
											: metadata.readingDirection === "ttb"
												? "Top to Bottom"
												: metadata.readingDirection === "webtoon"
													? "Webtoon"
													: metadata.readingDirection
								}
							/>
						)}
					</Stack>
				</Paper>

				{/* Publishing Info */}
				{(metadata?.publisher ||
					metadata?.imprint ||
					metadata?.language ||
					metadata?.ageRating) && (
					<Paper p="sm" radius="sm" withBorder>
						<Stack gap="xs">
							<Text size="sm" fw={600} c="dimmed" tt="uppercase">
								Publishing
							</Text>
							{metadata?.publisher && (
								<InfoRow label="Publisher" value={metadata.publisher} />
							)}
							{metadata?.imprint && (
								<InfoRow label="Imprint" value={metadata.imprint} />
							)}
							{metadata?.language && (
								<InfoRow label="Language" value={metadata.language} />
							)}
							{metadata?.ageRating != null && (
								<InfoRow label="Age Rating" value={`${metadata.ageRating}+`} />
							)}
						</Stack>
					</Paper>
				)}

				{/* File System Info */}
				{series.path && (
					<Paper p="sm" radius="sm" withBorder>
						<Stack gap="xs">
							<Text size="sm" fw={600} c="dimmed" tt="uppercase">
								File System
							</Text>
							<InfoRow label="Path" value={series.path} copyable monospace />
						</Stack>
					</Paper>
				)}

				{/* Timestamps */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Timestamps
						</Text>
						<InfoRow label="Added" value={formatDateTime(series.createdAt)} />
						<InfoRow label="Updated" value={formatDateTime(series.updatedAt)} />
						{metadata?.createdAt && (
							<InfoRow
								label="Metadata Created"
								value={formatDateTime(metadata.createdAt)}
							/>
						)}
						{metadata?.updatedAt && (
							<InfoRow
								label="Metadata Updated"
								value={formatDateTime(metadata.updatedAt)}
							/>
						)}
					</Stack>
				</Paper>

				{/* Cover Info */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Cover
						</Text>
						<Group justify="space-between" wrap="nowrap" gap="md">
							<Text size="sm" c="dimmed">
								Custom Cover
							</Text>
							<Badge
								color={series.hasCustomCover ? "green" : "gray"}
								variant="light"
								size="sm"
							>
								{series.hasCustomCover ? "Yes" : "No"}
							</Badge>
						</Group>
						{series.selectedCoverSource && (
							<InfoRow
								label="Cover Source"
								value={series.selectedCoverSource}
							/>
						)}
					</Stack>
				</Paper>

				{/* External Sources */}
				{series.externalIds && series.externalIds.length > 0 && (
					<Paper p="sm" radius="sm" withBorder>
						<Stack gap="xs">
							<Text size="sm" fw={600} c="dimmed" tt="uppercase">
								External Sources
							</Text>
							{series.externalIds.map((extId) => (
								<Group
									key={extId.id}
									justify="space-between"
									wrap="nowrap"
									gap="md"
								>
									<Text size="sm" c="dimmed" style={{ flexShrink: 0 }}>
										{formatSourceName(extId.source)}
									</Text>
									<Group gap="xs" wrap="nowrap" style={{ minWidth: 0 }}>
										<Code
											style={{
												wordBreak: "break-all",
												whiteSpace: "normal",
											}}
										>
											{extId.externalId}
										</Code>
										<CopyButton value={extId.externalId}>
											{({ copied, copy }) => (
												<Tooltip
													label={copied ? "Copied" : "Copy"}
													withArrow
													zIndex={1100}
												>
													<ActionIcon
														size="xs"
														variant="subtle"
														color={copied ? "teal" : "gray"}
														onClick={copy}
														style={{ flexShrink: 0 }}
													>
														{copied ? (
															<IconCheck size={14} />
														) : (
															<IconCopy size={14} />
														)}
													</ActionIcon>
												</Tooltip>
											)}
										</CopyButton>
										{extId.externalUrl && (
											<Tooltip label="Open in new tab" withArrow zIndex={1100}>
												<ActionIcon
													size="xs"
													variant="subtle"
													color="blue"
													component="a"
													href={extId.externalUrl}
													target="_blank"
													rel="noopener noreferrer"
													style={{ flexShrink: 0 }}
												>
													<IconExternalLink size={14} />
												</ActionIcon>
											</Tooltip>
										)}
									</Group>
								</Group>
							))}
						</Stack>
					</Paper>
				)}

				{/* Identifiers */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Identifiers
						</Text>
						<InfoRow label="Series ID" value={series.id} copyable monospace />
						<InfoRow
							label="Library ID"
							value={series.libraryId}
							copyable
							monospace
						/>
					</Stack>
				</Paper>
			</Stack>
		</Modal>
	);
}
