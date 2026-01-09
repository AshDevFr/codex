import {
	Container,
	Loader,
	Stack,
	Tabs,
	Title,
	Text,
	Center,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import {
	Navigate,
	useLocation,
	useNavigate,
	useParams,
	useSearchParams,
} from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import { BooksSection } from "@/components/library/BooksSection";
import { RecommendedSection } from "@/components/library/RecommendedSection";
import { SeriesSection } from "@/components/library/SeriesSection";

export function LibraryPage() {
	const { libraryId } = useParams<{ libraryId: string }>();
	const location = useLocation();
	const navigate = useNavigate();
	const [searchParams] = useSearchParams();

	// Determine current tab from URL
	const pathParts = location.pathname.split("/");
	const currentTab = pathParts[pathParts.length - 1] || "recommended";

	// Handle libraryId === "all" case
	const isAllLibraries = libraryId === "all";

	// Fetch library data (if not "all")
	const {
		data: library,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["library", libraryId],
		queryFn: () => librariesApi.getById(libraryId!),
		enabled: !isAllLibraries && !!libraryId,
	});

	// Redirect to base path if no tab specified
	useEffect(() => {
		if (
			location.pathname === `/libraries/${libraryId}` ||
			location.pathname === `/libraries/${libraryId}/`
		) {
			navigate(`/libraries/${libraryId}/recommended`, { replace: true });
		}
	}, [location.pathname, libraryId, navigate]);

	// Handle 404 - redirect to home
	useEffect(() => {
		if (error && !isAllLibraries) {
			navigate("/", { replace: true });
		}
	}, [error, isAllLibraries, navigate]);

	// Tab navigation
	const handleTabChange = (value: string | null) => {
		if (value) {
			navigate(`/libraries/${libraryId}/${value}`);
		}
	};

	if (!libraryId) {
		return <Navigate to="/" replace />;
	}

	if (isLoading && !isAllLibraries) {
		return (
			<Center h={400}>
				<Loader size="lg" />
			</Center>
		);
	}

	if (error && !isAllLibraries) {
		return (
			<Container size="xl" py="xl">
				<Center h={400}>
					<Stack align="center" gap="md">
						<Text size="xl" fw={600}>
							Library Not Found
						</Text>
						<Text c="dimmed">The requested library could not be found.</Text>
					</Stack>
				</Center>
			</Container>
		);
	}

	return (
		<Container size="xl" py="xl">
			<Stack gap="xl">
				{/* Header with library name (or "All Libraries") */}
				<Title order={1} tt="capitalize">
					{isAllLibraries ? "All Libraries" : library?.name || "Library"}
				</Title>

				{/* Tabs Navigation */}
				<Tabs value={currentTab} onChange={handleTabChange}>
					<Tabs.List>
						<Tabs.Tab value="recommended">Recommended</Tabs.Tab>
						<Tabs.Tab value="series">Series</Tabs.Tab>
						<Tabs.Tab value="books">Books</Tabs.Tab>
					</Tabs.List>

					{/* Recommended Tab */}
					<Tabs.Panel value="recommended" pt="xl">
						<RecommendedSection libraryId={libraryId} />
					</Tabs.Panel>

					{/* Series Tab */}
					<Tabs.Panel value="series" pt="xl">
						<SeriesSection libraryId={libraryId} searchParams={searchParams} />
					</Tabs.Panel>

					{/* Books Tab */}
					<Tabs.Panel value="books" pt="xl">
						<BooksSection libraryId={libraryId} searchParams={searchParams} />
					</Tabs.Panel>
				</Tabs>
			</Stack>
		</Container>
	);
}
