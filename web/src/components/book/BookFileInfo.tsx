import { Grid, Paper, Stack, Text, Title, Tooltip } from "@mantine/core";
import type { Book } from "@/types";

interface BookFileInfoProps {
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

function formatDate(dateString: string): string {
	return new Date(dateString).toLocaleDateString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
	});
}

interface InfoItemProps {
	label: string;
	value: string | number | null | undefined;
	tooltip?: string;
}

function InfoItem({ label, value, tooltip }: InfoItemProps) {
	if (!value && value !== 0) return null;

	const content = (
		<Paper p="sm" radius="sm" withBorder>
			<Stack gap={2}>
				<Text size="xs" c="dimmed" tt="uppercase" fw={500}>
					{label}
				</Text>
				<Text
					size="sm"
					fw={500}
					style={{
						overflow: "hidden",
						textOverflow: "ellipsis",
						whiteSpace: "nowrap",
					}}
				>
					{value}
				</Text>
			</Stack>
		</Paper>
	);

	if (tooltip) {
		return (
			<Tooltip label={tooltip} position="top" multiline w={300}>
				{content}
			</Tooltip>
		);
	}

	return content;
}

export function BookFileInfo({ book }: BookFileInfoProps) {
	const items = [
		{ label: "Format", value: book.fileFormat.toUpperCase() },
		{ label: "Size", value: formatFileSize(book.fileSize) },
		{ label: "Pages", value: book.pageCount },
		{
			label: "Hash",
			value: book.fileHash.substring(0, 12) + "...",
			tooltip: book.fileHash,
		},
		{ label: "Added", value: formatDate(book.createdAt) },
		{
			label: "File Path",
			value: book.filePath.split("/").pop() || book.filePath,
			tooltip: book.filePath,
		},
	];

	return (
		<Stack gap="sm">
			<Title order={4}>File Information</Title>
			<Grid gutter="sm">
				{items.map((item) => (
					<Grid.Col key={item.label} span={{ base: 6, sm: 4, md: 3, lg: 2 }}>
						<InfoItem
							label={item.label}
							value={item.value}
							tooltip={item.tooltip}
						/>
					</Grid.Col>
				))}
			</Grid>
		</Stack>
	);
}
