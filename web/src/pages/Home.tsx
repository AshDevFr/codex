import { Box, Stack, Title } from "@mantine/core";
import { BulkSelectionToolbar } from "@/components/library/BulkSelectionToolbar";
import { RecommendedSection } from "@/components/library/RecommendedSection";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";

export function Home() {
	useDocumentTitle("Home");

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Home</Title>
				{/* Bulk Selection Toolbar - shows when items are selected */}
				<BulkSelectionToolbar />
				<RecommendedSection libraryId="all" />
			</Stack>
		</Box>
	);
}
