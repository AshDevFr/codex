import { Group, Progress, Text } from "@mantine/core";
import type { ReadProgress } from "@/types";

interface BookProgressProps {
	progress: ReadProgress | null | undefined;
	pageCount: number;
	fileFormat?: string;
}

export function BookProgress({ progress, pageCount, fileFormat }: BookProgressProps) {
	if (!progress) {
		return (
			<Text size="sm" c="dimmed">
				Not started
			</Text>
		);
	}

	const currentPage = progress.current_page; // Already 1-indexed
	const percentage = pageCount > 0 ? (currentPage / pageCount) * 100 : 0;

	if (progress.completed) {
		return (
			<Group gap="xs">
				<Text size="sm" c="green" fw={500}>
					Completed
				</Text>
				{progress.completed_at && (
					<Text size="xs" c="dimmed">
						on {new Date(progress.completed_at).toLocaleDateString()}
					</Text>
				)}
			</Group>
		);
	}

	return (
		<Group gap="md" style={{ flex: 1 }}>
			{fileFormat !== "epub" ? (
				<Text size="sm">
					Page {currentPage} of {pageCount} ({Math.round(percentage)}%)
				</Text>
			) : (
				<Text size="sm">{Math.round(percentage)}%</Text>
			)}
			<Progress
				value={percentage}
				size="sm"
				style={{ flex: 1, maxWidth: 300 }}
			/>
		</Group>
	);
}
