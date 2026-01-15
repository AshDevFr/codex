import { Button, Group, Text, Tooltip } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconStar } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { ratingsApi, storageToDisplayRating } from "@/api/ratings";
import { SeriesRatingModal } from "./SeriesRatingModal";

interface SeriesRatingProps {
	seriesId: string;
}

export function SeriesRating({ seriesId }: SeriesRatingProps) {
	const [modalOpened, { open: openModal, close: closeModal }] =
		useDisclosure(false);

	// Fetch existing rating
	const { data: existingRating, isLoading } = useQuery({
		queryKey: ["series-rating", seriesId],
		queryFn: () => ratingsApi.getUserRating(seriesId),
	});

	if (isLoading) {
		return (
			<Text size="sm" c="dimmed">
				Loading...
			</Text>
		);
	}

	const displayValue = existingRating
		? storageToDisplayRating(existingRating.rating)
		: null;

	return (
		<>
			<Group gap="sm" align="center">
				<IconStar
					size={16}
					style={{
						color: existingRating
							? "var(--mantine-color-yellow-5)"
							: "var(--mantine-color-dimmed)",
						flexShrink: 0,
					}}
				/>
				{existingRating ? (
					<>
						<Text fw={700} size="sm">
							{displayValue?.toFixed(1)}
						</Text>
						<Button size="xs" variant="subtle" onClick={openModal}>
							Edit
						</Button>
						{existingRating.notes && (
							<Tooltip label={existingRating.notes} multiline maw={300}>
								<Text
									size="xs"
									c="dimmed"
									lineClamp={1}
									style={{ maxWidth: 200, cursor: "help" }}
								>
									{existingRating.notes}
								</Text>
							</Tooltip>
						)}
					</>
				) : (
					<Button size="xs" variant="light" onClick={openModal}>
						Add Rating
					</Button>
				)}
			</Group>

			<SeriesRatingModal
				seriesId={seriesId}
				opened={modalOpened}
				onClose={closeModal}
			/>
		</>
	);
}
