import { List, Text } from "@mantine/core";
import type { AlternateTitle } from "@/api/seriesMetadata";

interface AlternateTitlesProps {
	titles: AlternateTitle[];
}

export function AlternateTitles({ titles }: AlternateTitlesProps) {
	if (titles.length === 0) {
		return null;
	}

	return (
		<List size="sm" spacing="xs">
			{titles.map((title) => (
				<List.Item key={title.id}>
					<Text component="span" size="sm" c="dimmed">
						{title.label}:{" "}
					</Text>
					<Text component="span" size="sm">
						{title.title}
					</Text>
				</List.Item>
			))}
		</List>
	);
}
