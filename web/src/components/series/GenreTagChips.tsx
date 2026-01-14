import { Badge, Group, Text } from "@mantine/core";
import { useNavigate } from "react-router-dom";
import type { Genre } from "@/api/genres";
import type { Tag } from "@/api/tags";

interface GenreTagChipsProps {
	genres?: Genre[];
	tags?: Tag[];
	libraryId?: string;
	clickable?: boolean;
	maxDisplay?: number;
}

export function GenreTagChips({
	genres = [],
	tags = [],
	libraryId,
	clickable = true,
	maxDisplay,
}: GenreTagChipsProps) {
	const navigate = useNavigate();

	const handleGenreClick = (genre: Genre) => {
		if (!clickable) return;
		const basePath = libraryId ? `/libraries/${libraryId}` : "/libraries/all";
		navigate(`${basePath}/series?genres=${encodeURIComponent(genre.name)}`);
	};

	const handleTagClick = (tag: Tag) => {
		if (!clickable) return;
		const basePath = libraryId ? `/libraries/${libraryId}` : "/libraries/all";
		navigate(`${basePath}/series?tags=${encodeURIComponent(tag.name)}`);
	};

	const displayGenres = maxDisplay ? genres.slice(0, maxDisplay) : genres;
	const displayTags = maxDisplay
		? tags.slice(0, Math.max(0, maxDisplay - genres.length))
		: tags;
	const hiddenCount =
		genres.length + tags.length - displayGenres.length - displayTags.length;

	if (genres.length === 0 && tags.length === 0) {
		return null;
	}

	return (
		<Group gap="xs">
			{displayGenres.map((genre) => (
				<Badge
					key={`genre-${genre.id}`}
					variant="light"
					color="blue"
					size="sm"
					style={clickable ? { cursor: "pointer" } : undefined}
					onClick={() => handleGenreClick(genre)}
				>
					{genre.name}
				</Badge>
			))}
			{displayTags.map((tag) => (
				<Badge
					key={`tag-${tag.id}`}
					variant="light"
					color="gray"
					size="sm"
					style={clickable ? { cursor: "pointer" } : undefined}
					onClick={() => handleTagClick(tag)}
				>
					{tag.name}
				</Badge>
			))}
			{hiddenCount > 0 && (
				<Text size="xs" c="dimmed">
					+{hiddenCount} more
				</Text>
			)}
		</Group>
	);
}
