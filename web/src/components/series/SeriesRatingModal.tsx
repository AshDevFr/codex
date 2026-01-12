import {
	Box,
	Button,
	Group,
	Modal,
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

interface SeriesRatingModalProps {
	seriesId: string;
	opened: boolean;
	onClose: () => void;
}

export function SeriesRatingModal({
	seriesId,
	opened,
	onClose,
}: SeriesRatingModalProps) {
	const queryClient = useQueryClient();
	const [displayRating, setDisplayRating] = useState<number>(5);
	const [notes, setNotes] = useState<string>("");
	const [isDirty, setIsDirty] = useState(false);

	// Fetch existing rating
	const { data: existingRating, isLoading } = useQuery({
		queryKey: ["series-rating", seriesId],
		queryFn: () => ratingsApi.getUserRating(seriesId),
		enabled: opened,
	});

	// Update local state when existing rating loads or modal opens
	useEffect(() => {
		if (existingRating) {
			setDisplayRating(storageToDisplayRating(existingRating.rating));
			setNotes(existingRating.notes || "");
			setIsDirty(false);
		} else if (opened) {
			// Reset to defaults when modal opens with no existing rating
			setDisplayRating(5);
			setNotes("");
			setIsDirty(false);
		}
	}, [existingRating, opened]);

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
			queryClient.invalidateQueries({
				queryKey: ["series-metadata", seriesId],
			});
			setIsDirty(false);
			onClose();
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
			queryClient.invalidateQueries({
				queryKey: ["series-metadata", seriesId],
			});
			setDisplayRating(5);
			setNotes("");
			setIsDirty(false);
			onClose();
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

	// Ensure we have a valid number for the slider
	const sliderValue = Number.isFinite(displayRating) ? displayRating : 5;

	return (
		<Modal opened={opened} onClose={onClose} title="Rate this series" size="md">
			<Stack gap="md">
				{isLoading ? (
					<Text size="sm" c="dimmed">
						Loading...
					</Text>
				) : (
					<>
						<Group gap="xs" align="center">
							<IconStar
								size={20}
								style={{ color: "var(--mantine-color-yellow-5)" }}
							/>
							<Text fw={500}>Your Rating</Text>
							<Text fw={700} size="lg" ml="auto">
								{sliderValue.toFixed(1)}
							</Text>
						</Group>

						<Box px="xs">
							<Slider
								value={sliderValue}
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
						</Box>

						<Textarea
							label="Notes (optional)"
							placeholder="Add your thoughts about this series..."
							value={notes}
							onChange={(e) => handleNotesChange(e.currentTarget.value)}
							minRows={3}
							maxRows={6}
							autosize
						/>

						<Group justify="space-between" mt="md">
							<Box>
								{existingRating && (
									<Button
										variant="subtle"
										color="red"
										leftSection={<IconTrash size={16} />}
										onClick={() => deleteMutation.mutate()}
										loading={deleteMutation.isPending}
									>
										Delete Rating
									</Button>
								)}
							</Box>
							<Group gap="sm">
								<Button variant="default" onClick={onClose}>
									Cancel
								</Button>
								<Button
									onClick={() => saveMutation.mutate()}
									loading={saveMutation.isPending}
									disabled={!isDirty && !!existingRating}
								>
									{existingRating ? "Update Rating" : "Save Rating"}
								</Button>
							</Group>
						</Group>
					</>
				)}
			</Stack>
		</Modal>
	);
}
