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
import { IconCheck, IconCopy, IconInfoCircle } from "@tabler/icons-react";
import type { Book } from "@/types";

export interface BookInfoModalProps {
	opened: boolean;
	onClose: () => void;
	book: Book;
}

function formatFileSize(bytes: number): string {
	if (bytes >= 1073741824) {
		return `${(bytes / 1073741824).toFixed(2)} GB`;
	}
	if (bytes >= 1048576) {
		return `${(bytes / 1048576).toFixed(2)} MB`;
	}
	if (bytes >= 1024) {
		return `${(bytes / 1024).toFixed(2)} KB`;
	}
	return `${bytes} B`;
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

interface InfoRowProps {
	label: string;
	value: string | number | null | undefined;
	copyable?: boolean;
	monospace?: boolean;
}

function InfoRow({ label, value, copyable, monospace }: InfoRowProps) {
	if (value === null || value === undefined || value === "") return null;

	const displayValue = String(value);

	// For copyable monospace values (path, hash, IDs), show inline with copy button
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
							<Tooltip label={copied ? "Copied" : "Copy"} withArrow>
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

export function BookInfoModal({ opened, onClose, book }: BookInfoModalProps) {
	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title={
				<Group gap="xs">
					<IconInfoCircle size={20} />
					<Text fw={500}>Book Information</Text>
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
						<InfoRow label="Title" value={book.title} />
						<InfoRow label="Number" value={book.number} />
						<InfoRow label="Series" value={book.seriesName} />
						<InfoRow label="Library" value={book.libraryName} />
						{book.titleSort && (
							<InfoRow label="Sort Title" value={book.titleSort} />
						)}
						{book.readingDirection && (
							<InfoRow
								label="Reading Direction"
								value={
									book.readingDirection === "ltr"
										? "Left to Right"
										: book.readingDirection === "rtl"
											? "Right to Left"
											: book.readingDirection === "ttb"
												? "Top to Bottom"
												: book.readingDirection === "webtoon"
													? "Webtoon"
													: book.readingDirection
								}
							/>
						)}
					</Stack>
				</Paper>

				{/* File Info */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							File Information
						</Text>
						<InfoRow label="Format" value={book.fileFormat.toUpperCase()} />
						<InfoRow label="Size" value={formatFileSize(book.fileSize)} />
						<InfoRow label="Pages" value={book.pageCount} />
						<InfoRow label="Path" value={book.filePath} copyable monospace />
						<InfoRow label="Hash" value={book.fileHash} copyable monospace />
					</Stack>
				</Paper>

				{/* Read Progress */}
				{book.readProgress && (
					<Paper p="sm" radius="sm" withBorder>
						<Stack gap="xs">
							<Text size="sm" fw={600} c="dimmed" tt="uppercase">
								Reading Progress
							</Text>
							<InfoRow
								label="Current Page"
								value={`${book.readProgress.current_page} / ${book.pageCount}`}
							/>
							{book.readProgress.progress_percentage !== null &&
								book.readProgress.progress_percentage !== undefined && (
									<InfoRow
										label="Progress"
										value={`${Math.round(book.readProgress.progress_percentage * 100)}%`}
									/>
								)}
							<Group justify="space-between" wrap="nowrap" gap="md">
								<Text size="sm" c="dimmed">
									Status
								</Text>
								<Badge
									color={book.readProgress.completed ? "green" : "blue"}
									variant="light"
									size="sm"
								>
									{book.readProgress.completed ? "Completed" : "In Progress"}
								</Badge>
							</Group>
							<InfoRow
								label="Started"
								value={formatDateTime(book.readProgress.started_at)}
							/>
							{book.readProgress.completed &&
								book.readProgress.completed_at && (
									<InfoRow
										label="Completed"
										value={formatDateTime(book.readProgress.completed_at)}
									/>
								)}
							<InfoRow
								label="Last Read"
								value={formatDateTime(book.readProgress.updated_at)}
							/>
						</Stack>
					</Paper>
				)}

				{/* Timestamps & Status */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Timestamps & Status
						</Text>
						<InfoRow label="Added" value={formatDateTime(book.createdAt)} />
						<InfoRow label="Updated" value={formatDateTime(book.updatedAt)} />
						<Group justify="space-between" wrap="nowrap" gap="md">
							<Text size="sm" c="dimmed">
								Status
							</Text>
							<Badge
								color={book.deleted ? "red" : "green"}
								variant="light"
								size="sm"
							>
								{book.deleted ? "Deleted" : "Active"}
							</Badge>
						</Group>
					</Stack>
				</Paper>

				{/* IDs */}
				<Paper p="sm" radius="sm" withBorder>
					<Stack gap="xs">
						<Text size="sm" fw={600} c="dimmed" tt="uppercase">
							Identifiers
						</Text>
						<InfoRow label="Book ID" value={book.id} copyable monospace />
						<InfoRow
							label="Series ID"
							value={book.seriesId}
							copyable
							monospace
						/>
						<InfoRow
							label="Library ID"
							value={book.libraryId}
							copyable
							monospace
						/>
					</Stack>
				</Paper>

				{/* Analysis Error */}
				{book.analysisError && (
					<Paper
						p="sm"
						radius="sm"
						withBorder
						style={{ borderColor: "var(--mantine-color-red-6)" }}
					>
						<Stack gap="xs">
							<Text size="sm" fw={600} c="red" tt="uppercase">
								Analysis Error
							</Text>
							<Code color="red" block style={{ whiteSpace: "pre-wrap" }}>
								{book.analysisError}
							</Code>
						</Stack>
					</Paper>
				)}
			</Stack>
		</Modal>
	);
}
