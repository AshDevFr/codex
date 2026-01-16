import { Box, Stack, Title } from "@mantine/core";
import { RecommendedSection } from "@/components/library/RecommendedSection";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";

export function Home() {
	useDocumentTitle("Home");

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Home</Title>
				<RecommendedSection libraryId="all" />
			</Stack>
		</Box>
	);
}
