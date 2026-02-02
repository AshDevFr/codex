import { Badge, Group, Text } from "@mantine/core";
import { Link } from "react-router-dom";
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
  const basePath = libraryId ? `/libraries/${libraryId}` : "/libraries/all";

  const getGenreUrl = (genre: Genre) =>
    `${basePath}/series?gf=any:${encodeURIComponent(genre.name)}`;

  const getTagUrl = (tag: Tag) =>
    `${basePath}/series?tf=any:${encodeURIComponent(tag.name)}`;

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
      {displayGenres.map((genre) =>
        clickable ? (
          <Badge
            key={`genre-${genre.id}`}
            component={Link}
            to={getGenreUrl(genre)}
            variant="light"
            color="blue"
            size="sm"
            style={{ cursor: "pointer", textDecoration: "none" }}
          >
            {genre.name}
          </Badge>
        ) : (
          <Badge
            key={`genre-${genre.id}`}
            variant="light"
            color="blue"
            size="sm"
          >
            {genre.name}
          </Badge>
        ),
      )}
      {displayTags.map((tag) =>
        clickable ? (
          <Badge
            key={`tag-${tag.id}`}
            component={Link}
            to={getTagUrl(tag)}
            variant="light"
            color="gray"
            size="sm"
            style={{ cursor: "pointer", textDecoration: "none" }}
          >
            {tag.name}
          </Badge>
        ) : (
          <Badge key={`tag-${tag.id}`} variant="light" color="gray" size="sm">
            {tag.name}
          </Badge>
        ),
      )}
      {hiddenCount > 0 && (
        <Text size="xs" c="dimmed">
          +{hiddenCount} more
        </Text>
      )}
    </Group>
  );
}
