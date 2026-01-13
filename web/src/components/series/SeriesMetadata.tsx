import { Grid, Paper, Stack, Text, Title } from "@mantine/core";
import type { FullSeriesMetadata } from "@/api/seriesMetadata";

interface SeriesMetadataProps {
	metadata: FullSeriesMetadata;
}

// Map status values to display text
const STATUS_DISPLAY: Record<string, string> = {
	ongoing: "Ongoing",
	ended: "Ended",
	hiatus: "Hiatus",
	abandoned: "Abandoned",
	unknown: "Unknown",
};

// Map reading direction values to display text
const READING_DIRECTION_DISPLAY: Record<string, string> = {
	ltr: "Left to Right",
	rtl: "Right to Left",
	ttb: "Vertical",
	webtoon: "Webtoon",
};

// Map language codes to display names
const LANGUAGE_DISPLAY: Record<string, string> = {
	en: "English",
	ja: "Japanese",
	ko: "Korean",
	zh: "Chinese",
	fr: "French",
	de: "German",
	es: "Spanish",
	it: "Italian",
	pt: "Portuguese",
	ru: "Russian",
};

interface MetadataItemProps {
	label: string;
	value: string | number | null | undefined;
}

function MetadataItem({ label, value }: MetadataItemProps) {
	if (!value && value !== 0) return null;

	return (
		<Paper p="sm" radius="sm" withBorder>
			<Stack gap={2}>
				<Text size="xs" c="dimmed" tt="uppercase" fw={500}>
					{label}
				</Text>
				<Text size="sm" fw={500}>
					{value}
				</Text>
			</Stack>
		</Paper>
	);
}

export function SeriesMetadata({ metadata }: SeriesMetadataProps) {
	const statusDisplay = metadata.status
		? STATUS_DISPLAY[metadata.status] || metadata.status
		: null;
	const readingDirDisplay = metadata.readingDirection
		? READING_DIRECTION_DISPLAY[metadata.readingDirection] ||
			metadata.readingDirection
		: null;
	const languageDisplay = metadata.language
		? LANGUAGE_DISPLAY[metadata.language] || metadata.language
		: null;
	const ageRatingDisplay = metadata.ageRating
		? `${metadata.ageRating}+`
		: null;

	// Count how many items we have to display
	const items = [
		{ label: "Status", value: statusDisplay },
		{ label: "Year", value: metadata.year },
		{ label: "Publisher", value: metadata.publisher },
		{ label: "Imprint", value: metadata.imprint },
		{ label: "Language", value: languageDisplay },
		{ label: "Age Rating", value: ageRatingDisplay },
		{ label: "Reading Direction", value: readingDirDisplay },
		{ label: "Total Books", value: metadata.totalBookCount },
	].filter((item) => item.value !== null && item.value !== undefined);

	if (items.length === 0) {
		return null;
	}

	return (
		<Stack gap="sm">
			<Title order={4}>Metadata</Title>
			<Grid gutter="sm">
				{items.map((item) => (
					<Grid.Col key={item.label} span={{ base: 6, sm: 4, md: 3, lg: 2 }}>
						<MetadataItem label={item.label} value={item.value} />
					</Grid.Col>
				))}
			</Grid>
		</Stack>
	);
}
