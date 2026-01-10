import { Box, Stack, Title } from "@mantine/core";
import { RecommendedSection } from "@/components/library/RecommendedSection";

export function Home() {
	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Home</Title>
				<RecommendedSection libraryId="all" />
			</Stack>
		</Box>
	);
}
