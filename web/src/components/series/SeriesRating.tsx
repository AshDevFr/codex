import {
	Box,
	Button,
	Group,
	Slider,
	Stack,
	Text,
	Textarea,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconStar, IconTrash } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import {
	displayToStorageRating,
	ratingsApi,
	storageToDisplayRating,
	type UserSeriesRating,
} from "@/api/ratings";

interface SeriesRatingProps {
	seriesId: string;
}

export function SeriesRating({ seriesId }: SeriesRatingProps) {
	const queryClient = useQueryClient();
	const [displayRating, setDisplayRating] = useState<number>(5);
	const [notes, setNotes] = useState<string>("");
	const [isDirty, setIsDirty] = useState(false);

	// Fetch existing rating
	const { data: existingRating, isLoading } = useQuery({
		queryKey: ["series-rating", seriesId],
		queryFn: () => ratingsApi.getUserRating(seriesId),
	});

	// Update local state when existing rating loads
	useEffect(() => {
		if (existingRating) {
			setDisplayRating(storageToDisplayRating(existingRating.rating));
			setNotes(existingRating.notes || "");
			setIsDirty(false);
		}
	}, [existingRating]);

	// Save rating mutation
	const saveMutation = useMutation({
		mutationFn: () =>
			ratingsApi.setUserRating(
				seriesId,
				displayToStorageRating(displayRating),
				notes || undefined,
			),
		onSuccess: (data: UserSeriesRating) => {
			notifications.show({
				title: "Rating saved",
				message: `Your rating of ${storageToDisplayRating(data.rating).toFixed(1)} has been saved`,
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["series-rating", seriesId] });
			queryClient.invalidateQueries({ queryKey: ["series-metadata", seriesId] });
			setIsDirty(false);
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to save rating",
				message: error.message || "An error occurred",
				color: "red",
			});
		},
	});

	// Delete rating mutation
	const deleteMutation = useMutation({
		mutationFn: () => ratingsApi.deleteUserRating(seriesId),
		onSuccess: () => {
			notifications.show({
				title: "Rating deleted",
				message: "Your rating has been removed",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series-rating", seriesId] });
			queryClient.invalidateQueries({ queryKey: ["series-metadata", seriesId] });
			setDisplayRating(5);
			setNotes("");
			setIsDirty(false);
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Failed to delete rating",
				message: error.message || "An error occurred",
				color: "red",
			});
		},
	});

	const handleRatingChange = (value: number) => {
		setDisplayRating(value);
		setIsDirty(true);
	};

	const handleNotesChange = (value: string) => {
		setNotes(value);
		setIsDirty(true);
	};

	if (isLoading) {
		return (
			<Box>
				<Text size="sm" c="dimmed">
					Loading rating...
				</Text>
			</Box>
		);
	}

	return (
		<Stack gap="md">
			<Group gap="xs">
				<IconStar
					size={20}
					style={{ color: "var(--mantine-color-yellow-5)" }}
				/>
				<Text fw={500}>Your Rating</Text>
			</Group>

			<Box px="xs">
				<Slider
					value={displayRating}
					onChange={handleRatingChange}
					min={1}
					max={10}
					step={0.5}
					marks={[
						{ value: 1, label: "1" },
						{ value: 5, label: "5" },
						{ value: 10, label: "10" },
					]}
					label={(value) => value.toFixed(1)}
					styles={{
						markLabel: { fontSize: 12 },
					}}
				/>
				<Text ta="center" size="xl" fw={700} mt="md">
					{displayRating.toFixed(1)}
				</Text>
			</Box>

			<Textarea
				placeholder="Add notes (optional)"
				value={notes}
				onChange={(e) => handleNotesChange(e.currentTarget.value)}
				minRows={2}
				maxRows={4}
				autosize
			/>

			<Group gap="sm">
				<Button
					onClick={() => saveMutation.mutate()}
					loading={saveMutation.isPending}
					disabled={!isDirty && !!existingRating}
				>
					{existingRating ? "Update Rating" : "Save Rating"}
				</Button>
				{existingRating && (
					<Button
						variant="subtle"
						color="red"
						leftSection={<IconTrash size={16} />}
						onClick={() => deleteMutation.mutate()}
						loading={deleteMutation.isPending}
					>
						Delete
					</Button>
				)}
			</Group>
		</Stack>
	);
}
