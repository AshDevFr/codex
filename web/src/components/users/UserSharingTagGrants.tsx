import {
	ActionIcon,
	Badge,
	Card,
	Group,
	Loader,
	Select,
	Stack,
	Text,
	Title,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconPlus, IconShare, IconTrash } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type AccessMode, sharingTagsApi } from "@/api/sharingTags";

interface UserSharingTagGrantsProps {
	userId: string;
	readonly?: boolean;
}

export function UserSharingTagGrants({
	userId,
	readonly = false,
}: UserSharingTagGrantsProps) {
	const queryClient = useQueryClient();
	const [selectedTagId, setSelectedTagId] = useState<string | null>(null);
	const [selectedAccessMode, setSelectedAccessMode] =
		useState<AccessMode>("deny");

	// Fetch all sharing tags (for the select options)
	const { data: allTags, isLoading: allTagsLoading } = useQuery({
		queryKey: ["sharing-tags"],
		queryFn: sharingTagsApi.list,
	});

	// Fetch user's grants
	const { data: grantsResponse, isLoading: grantsLoading } = useQuery({
		queryKey: ["user-sharing-tag-grants", userId],
		queryFn: () => sharingTagsApi.getGrantsForUser(userId),
	});

	// Mutation for setting a grant
	const setGrantMutation = useMutation({
		mutationFn: ({
			tagId,
			accessMode,
		}: {
			tagId: string;
			accessMode: AccessMode;
		}) => sharingTagsApi.setGrantForUser(userId, tagId, accessMode),
		onSuccess: () => {
			queryClient.invalidateQueries({
				queryKey: ["user-sharing-tag-grants", userId],
			});
			setSelectedTagId(null);
			notifications.show({
				title: "Success",
				message: "Sharing tag grant added",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to add sharing tag grant",
				color: "red",
			});
		},
	});

	// Mutation for removing a grant
	const removeGrantMutation = useMutation({
		mutationFn: (tagId: string) =>
			sharingTagsApi.removeGrantFromUser(userId, tagId),
		onSuccess: () => {
			queryClient.invalidateQueries({
				queryKey: ["user-sharing-tag-grants", userId],
			});
			notifications.show({
				title: "Success",
				message: "Sharing tag grant removed",
				color: "green",
			});
		},
		onError: (error: { message?: string }) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to remove sharing tag grant",
				color: "red",
			});
		},
	});

	const isLoading = allTagsLoading || grantsLoading;
	const grants = grantsResponse?.grants || [];

	// Filter out tags that already have grants
	const availableTags =
		allTags?.filter((tag) => !grants.some((g) => g.sharingTagId === tag.id)) ||
		[];

	const tagOptions = availableTags.map((tag) => ({
		value: tag.id,
		label: tag.name,
	}));

	const handleAddGrant = () => {
		if (selectedTagId) {
			setGrantMutation.mutate({
				tagId: selectedTagId,
				accessMode: selectedAccessMode,
			});
		}
	};

	const handleRemoveGrant = (tagId: string) => {
		removeGrantMutation.mutate(tagId);
	};

	const accessModeOptions = [
		{
			value: "deny",
			label: "Deny",
		},
		{
			value: "allow",
			label: "Allow",
		},
	];

	if (isLoading) {
		return (
			<Card withBorder p="md">
				<Stack gap="md">
					<Group gap="sm">
						<IconShare size={20} />
						<Title order={5}>Sharing Tag Grants</Title>
					</Group>
					<Group justify="center">
						<Loader size="sm" />
					</Group>
				</Stack>
			</Card>
		);
	}

	// If no tags exist in the system, show a helpful message
	if (!allTags || allTags.length === 0) {
		return (
			<Card withBorder p="md">
				<Stack gap="md">
					<Group gap="sm">
						<IconShare size={20} />
						<Title order={5}>Sharing Tag Grants</Title>
					</Group>
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
						</Text>{" "}
						to control content visibility.
					</Text>
				</Stack>
			</Card>
		);
	}

	return (
		<Card withBorder p="md">
			<Stack gap="md">
				<Group gap="sm">
					<IconShare size={20} />
					<Title order={5}>Sharing Tag Grants</Title>
				</Group>

				<Text size="sm" c="dimmed">
					Control which tagged content this user can see. "Deny" hides content
					with the tag, "Allow" explicitly permits it (useful for future
					whitelist mode).
				</Text>

				{/* Current grants */}
				{grants.length > 0 ? (
					<Stack gap="xs">
						{grants.map((grant) => (
							<Group key={grant.id} justify="space-between">
								<Group gap="sm">
									<Badge
										variant="light"
										color={grant.accessMode === "deny" ? "red" : "green"}
									>
										{grant.accessMode === "deny" ? "Deny" : "Allow"}
									</Badge>
									<Text size="sm">{grant.sharingTagName}</Text>
								</Group>
								{!readonly && (
									<Tooltip label="Remove grant">
										<ActionIcon
											size="sm"
											variant="subtle"
											color="red"
											onClick={() => handleRemoveGrant(grant.sharingTagId)}
											disabled={removeGrantMutation.isPending}
										>
											<IconTrash size={14} />
										</ActionIcon>
									</Tooltip>
								)}
							</Group>
						))}
					</Stack>
				) : (
					<Text size="sm" c="dimmed">
						No grants configured. User can see all content.
					</Text>
				)}

				{/* Add new grant */}
				{!readonly && availableTags.length > 0 && (
					<Group gap="sm" align="flex-end">
						<Select
							label="Add grant"
							placeholder="Select tag..."
							data={tagOptions}
							value={selectedTagId}
							onChange={setSelectedTagId}
							searchable
							style={{ flex: 1 }}
						/>
						<Select
							label="Access"
							data={accessModeOptions}
							value={selectedAccessMode}
							onChange={(value) =>
								setSelectedAccessMode((value as AccessMode) || "deny")
							}
							w={100}
						/>
						<Tooltip label="Add grant">
							<ActionIcon
								variant="filled"
								color="blue"
								size="lg"
								onClick={handleAddGrant}
								disabled={!selectedTagId || setGrantMutation.isPending}
								loading={setGrantMutation.isPending}
							>
								<IconPlus size={16} />
							</ActionIcon>
						</Tooltip>
					</Group>
				)}

				{!readonly && availableTags.length === 0 && grants.length > 0 && (
					<Text size="xs" c="dimmed">
						All available sharing tags have been assigned.
					</Text>
				)}
			</Stack>
		</Card>
	);
}
