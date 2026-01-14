import { Group, Text, Tooltip } from "@mantine/core";
import { IconStar } from "@tabler/icons-react";
import type { ExternalRating } from "@/api/seriesMetadata";

interface ExternalRatingsProps {
	ratings: ExternalRating[];
}

// Map source names to display names
const SOURCE_DISPLAY_NAMES: Record<string, string> = {
	myanimelist: "MAL",
	anilist: "AniList",
	mangabaka: "MangaBaka",
	mangadex: "MangaDex",
	kitsu: "Kitsu",
	mangaupdates: "MangaUpdates",
};

function formatRating(rating: number): string {
	// Rating is stored as 0-100, display as 0-10
	return (rating / 10).toFixed(1);
}

function formatVoteCount(count: number): string {
	if (count >= 1000000) {
		return `${(count / 1000000).toFixed(1)}M`;
	}
	if (count >= 1000) {
		return `${(count / 1000).toFixed(1)}K`;
	}
	return count.toString();
}

export function ExternalRatings({ ratings }: ExternalRatingsProps) {
	if (ratings.length === 0) {
		return null;
	}

	return (
		<Group gap="md">
			{ratings.map((rating) => {
				const displayName =
					SOURCE_DISPLAY_NAMES[rating.sourceName.toLowerCase()] ||
					rating.sourceName;
				const displayRating = formatRating(rating.rating);
				const voteText = rating.voteCount
					? `(${formatVoteCount(rating.voteCount)} votes)`
					: "";

				return (
					<Tooltip
						key={rating.id}
						label={`${rating.sourceName}: ${displayRating}/10 ${voteText}`}
						position="top"
					>
						<Group gap={4}>
							<IconStar
								size={14}
								style={{ color: "var(--mantine-color-yellow-5)" }}
							/>
							<Text size="sm" fw={500}>
								{displayName}: {displayRating}
							</Text>
							{rating.voteCount && (
								<Text size="xs" c="dimmed">
									({formatVoteCount(rating.voteCount)})
								</Text>
							)}
						</Group>
					</Tooltip>
				);
			})}
		</Group>
	);
}
