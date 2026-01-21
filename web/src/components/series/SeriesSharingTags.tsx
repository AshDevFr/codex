import {
	ActionIcon,
	Badge,
	Group,
	Loader,
	MultiSelect,
	Stack,
	Text,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconEdit, IconShare, IconX } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { sharingTagsApi } from "@/api/sharingTags";
import { useAuthStore } from "@/store/authStore";

interface SeriesSharingTagsProps {
	seriesId: string;
}

export function SeriesSharingTags({ seriesId }: SeriesSharingTagsProps) {
	const queryClient = useQueryClient();
	const { user } = useAuthStore();
	const isAdmin = user?.role === "admin";
	const [isEditing, setIsEditing] = useState(false);

	// Fetch all sharing tags (for the multiselect options)
	// Note: Hooks must be called unconditionally, even for non-admins
	const { data: allTags, isLoading: allTagsLoading } = useQuery({
		queryKey: ["sharing-tags"],
		queryFn: sharingTagsApi.list,
		enabled: isAdmin,
	});

	// Fetch current series sharing tags
	const { data: seriesTags, isLoading: seriesTagsLoading } = useQuery({
		queryKey: ["series-sharing-tags", seriesId],
		queryFn: () => sharingTagsApi.getForSeries(seriesId),
		enabled: isAdmin,
	});

	// Mutation for setting series sharing tags
	const setTagsMutation = useMutation({
		mutationFn: (tagIds: string[]) =>
			sharingTagsApi.setForSeries(seriesId, tagIds),
		onSuccess: () => {
			queryClient.invalidateQueries({
				queryKey: ["series-sharing-tags", seriesId],
			});
			notifications.show({
				title: "Success",
				message: "Sharing tags updated",
				color: "green",
			});
			setIsEditing(false);
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update sharing tags",
				color: "red",
			});
		},
	});

	// Mutation for removing a single tag
	const removeTagMutation = useMutation({
		mutationFn: (tagId: string) =>
			sharingTagsApi.removeFromSeries(seriesId, tagId),
		onSuccess: () => {
			queryClient.invalidateQueries({
				queryKey: ["series-sharing-tags", seriesId],
			});
			notifications.show({
				title: "Success",
				message: "Sharing tag removed",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to remove sharing tag",
				color: "red",
			});
		},
	});

	// Only render for admins (after all hooks)
	if (!isAdmin) {
		return null;
	}

	const isLoading = allTagsLoading || seriesTagsLoading;
	const selectedTagIds = seriesTags?.map((t) => t.id) || [];

	// Transform tags for multiselect
	const tagOptions =
		allTags?.map((tag) => ({
			value: tag.id,
			label: tag.name,
		})) || [];

	const handleTagsChange = (newTagIds: string[]) => {
		setTagsMutation.mutate(newTagIds);
	};

	const handleRemoveTag = (tagId: string) => {
		removeTagMutation.mutate(tagId);
	};

	if (isLoading) {
		return (
			<Group gap="md" align="flex-start">
				<Text size="sm" c="dimmed" w={100}>
					SHARING
				</Text>
				<Loader size="sm" />
			</Group>
		);
	}

	// If no tags exist in the system, show a helpful message
	if (!allTags || allTags.length === 0) {
		return (
			<Group gap="md" align="flex-start">
				<Text size="sm" c="dimmed" w={100}>
					SHARING
				</Text>
				<Text size="sm" c="dimmed">
					No sharing tags configured.{" "}
					<Text
						component="a"
						href="/settings/sharing-tags"
						size="sm"
						c="blue"
						td="underline"
					>
						Create sharing tags
					</Text>
				</Text>
			</Group>
		);
	}

	return (
		<Group gap="md" align="flex-start">
			<Text size="sm" c="dimmed" w={100}>
				SHARING
			</Text>
			{isEditing ? (
				<Stack gap="xs" style={{ flex: 1 }}>
					<MultiSelect
						data={tagOptions}
						value={selectedTagIds}
						onChange={handleTagsChange}
						placeholder="Select sharing tags..."
						searchable
						clearable
						disabled={setTagsMutation.isPending}
						leftSection={<IconShare size={16} />}
					/>
					<Group gap="xs">
						<Text size="xs" c="dimmed">
							Users with "deny" access to these tags won't see this series
						</Text>
					</Group>
				</Stack>
			) : (
				<Group gap="xs" wrap="wrap">
					{seriesTags && seriesTags.length > 0 ? (
						seriesTags.map((tag) => (
							<Tooltip key={tag.id} label={tag.description || tag.name}>
								<Badge
									variant="light"
									color="violet"
									rightSection={
										<ActionIcon
											size="xs"
											variant="transparent"
											color="violet"
											onClick={() => handleRemoveTag(tag.id)}
											disabled={removeTagMutation.isPending}
										>
											<IconX size={12} />
										</ActionIcon>
									}
								>
									{tag.name}
								</Badge>
							</Tooltip>
						))
					) : (
						<Text size="sm" c="dimmed">
							No sharing tags assigned
						</Text>
					)}
					<Tooltip label="Edit sharing tags">
						<ActionIcon
							size="sm"
							variant="subtle"
							onClick={() => setIsEditing(true)}
						>
							<IconEdit size={14} />
						</ActionIcon>
					</Tooltip>
				</Group>
			)}
		</Group>
	);
}
