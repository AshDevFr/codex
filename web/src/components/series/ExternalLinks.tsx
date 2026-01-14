import { ActionIcon, Group, Tooltip } from "@mantine/core";
import { IconExternalLink } from "@tabler/icons-react";
import type { ExternalLink } from "@/api/seriesMetadata";

interface ExternalLinksProps {
	links: ExternalLink[];
}

// Map source names to display names and colors
const SOURCE_CONFIG: Record<
	string,
	{ name: string; color: string; icon?: string }
> = {
	myanimelist: { name: "MyAnimeList", color: "#2e51a2" },
	anilist: { name: "AniList", color: "#02a9ff" },
	mangabaka: { name: "MangaBaka", color: "#ff6b35" },
	mangadex: { name: "MangaDex", color: "#ff6740" },
	kitsu: { name: "Kitsu", color: "#f75239" },
	mangaupdates: { name: "MangaUpdates", color: "#2a4a6d" },
	comicvine: { name: "Comic Vine", color: "#e41d25" },
	goodreads: { name: "Goodreads", color: "#553b08" },
	amazon: { name: "Amazon", color: "#ff9900" },
};

export function ExternalLinks({ links }: ExternalLinksProps) {
	if (links.length === 0) {
		return null;
	}

	return (
		<Group gap="xs">
			{links.map((link) => {
				const config = SOURCE_CONFIG[link.sourceName.toLowerCase()] || {
					name: link.sourceName,
					color: "gray",
				};

				return (
					<Tooltip key={link.id} label={config.name} position="top">
						<ActionIcon
							component="a"
							href={link.url}
							target="_blank"
							rel="noopener noreferrer"
							variant="light"
							color="gray"
							size="md"
						>
							<IconExternalLink size={16} />
						</ActionIcon>
					</Tooltip>
				);
			})}
		</Group>
	);
}
