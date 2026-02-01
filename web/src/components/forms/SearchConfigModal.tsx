import {
	Alert,
	Badge,
	Button,
	Card,
	Code,
	Group,
	Modal,
	Paper,
	Stack,
	Switch,
	Tabs,
	Text,
	Textarea,
	Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { notifications } from "@mantine/notifications";
import {
	IconCode,
	IconInfoCircle,
	IconSearch,
	IconSettings,
} from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import type { PluginDto } from "@/api/plugins";
import { pluginsApi } from "@/api/plugins";
import { SAMPLE_SERIES_CONTEXT } from "@/utils/templateUtils";
import { type AutoMatchConditions, ConditionsEditor } from "./ConditionsEditor";
import {
	type PreprocessingRule,
	PreprocessingRulesEditor,
} from "./PreprocessingRulesEditor";

/**
 * Available template helpers.
 */
const TEMPLATE_HELPERS = [
	{
		name: "clean",
		example: "{{clean metadata.title}}",
		description: "Remove noise (Digital, year, etc.)",
	},
	{
		name: "truncate",
		example: "{{truncate metadata.title 50}}",
		description: "Limit to N characters",
	},
	{
		name: "first_word",
		example: "{{first_word metadata.title}}",
		description: "First word only",
	},
	{
		name: "lowercase",
		example: "{{lowercase metadata.title}}",
		description: "Convert to lowercase",
	},
] as const;

/**
 * Render a preview of the template with sample data.
 */
function renderTemplatePreview(template: string): string {
	if (!template.trim()) return "(default: series title)";

	let preview = template;
	const ctx = SAMPLE_SERIES_CONTEXT;
	const meta = ctx.metadata;

	// Replace top-level field references
	preview = preview.replace(/\{\{bookCount\}\}/g, String(ctx.bookCount ?? 0));
	preview = preview.replace(/\{\{seriesId\}\}/g, ctx.seriesId ?? "");

	// Replace metadata field references
	preview = preview.replace(/\{\{metadata\.title\}\}/g, meta?.title ?? "");
	preview = preview.replace(
		/\{\{metadata\.titleSort\}\}/g,
		meta?.titleSort ?? "",
	);
	preview = preview.replace(
		/\{\{metadata\.year\}\}/g,
		String(meta?.year ?? ""),
	);
	preview = preview.replace(
		/\{\{metadata\.publisher\}\}/g,
		meta?.publisher ?? "",
	);
	preview = preview.replace(
		/\{\{metadata\.language\}\}/g,
		meta?.language ?? "",
	);
	preview = preview.replace(/\{\{metadata\.status\}\}/g, meta?.status ?? "");
	preview = preview.replace(
		/\{\{metadata\.ageRating\}\}/g,
		String(meta?.ageRating ?? ""),
	);
	preview = preview.replace(
		/\{\{metadata\.genres\}\}/g,
		meta?.genres?.join(", ") ?? "",
	);
	preview = preview.replace(
		/\{\{metadata\.tags\}\}/g,
		meta?.tags?.join(", ") ?? "",
	);

	// Simplify helper calls for preview
	preview = preview.replace(/\{\{clean metadata\.title\}\}/g, "One Piece");
	preview = preview.replace(
		/\{\{truncate metadata\.title \d+\}\}/g,
		"One Piece (D...",
	);
	preview = preview.replace(/\{\{first_word metadata\.title\}\}/g, "One");
	preview = preview.replace(
		/\{\{lowercase metadata\.title\}\}/g,
		"one piece (digital)",
	);

	// Handle conditionals (simplified)
	preview = preview.replace(/\{\{#if [\w.]+\}\}(.*?)\{\{\/if\}\}/g, "$1");
	preview = preview.replace(/\{\{#unless [\w.]+\}\}(.*?)\{\{\/unless\}\}/g, "");

	return preview || "(empty)";
}

interface SearchConfigModalProps {
	/** The plugin to configure */
	plugin: PluginDto;
	/** Whether the modal is open */
	opened: boolean;
	/** Callback when modal is closed */
	onClose: () => void;
}

interface SearchConfigFormValues {
	searchQueryTemplate: string;
	useExistingExternalId: boolean;
}

/**
 * Inner content component that handles the form state.
 * Separated to ensure state resets when plugin changes via key prop.
 */
function SearchConfigContent({
	plugin,
	onClose,
}: {
	plugin: PluginDto;
	onClose: () => void;
}) {
	const queryClient = useQueryClient();
	const [activeTab, setActiveTab] = useState<string | null>("template");

	// Parse initial preprocessing rules from plugin
	const initialPreprocessingRules: PreprocessingRule[] =
		plugin.searchPreprocessingRules &&
		Array.isArray(plugin.searchPreprocessingRules)
			? (plugin.searchPreprocessingRules as PreprocessingRule[])
			: [];

	// Parse initial auto-match conditions from plugin
	const initialAutoMatchConditions: AutoMatchConditions | null =
		plugin.autoMatchConditions && typeof plugin.autoMatchConditions === "object"
			? (plugin.autoMatchConditions as AutoMatchConditions)
			: null;

	// State for the complex editors
	const [preprocessingRules, setPreprocessingRules] = useState<
		PreprocessingRule[]
	>(initialPreprocessingRules);
	const [autoMatchConditions, setAutoMatchConditions] =
		useState<AutoMatchConditions | null>(initialAutoMatchConditions);
	const [testTitle, setTestTitle] = useState("");

	// Form for simple fields
	const form = useForm<SearchConfigFormValues>({
		initialValues: {
			searchQueryTemplate: plugin.searchQueryTemplate ?? "",
			useExistingExternalId: plugin.useExistingExternalId ?? true,
		},
	});

	// Live preview of the template
	const templatePreview = useMemo(
		() => renderTemplatePreview(form.values.searchQueryTemplate),
		[form.values.searchQueryTemplate],
	);

	const updateMutation = useMutation({
		mutationFn: async () => {
			return pluginsApi.update(plugin.id, {
				searchQueryTemplate: form.values.searchQueryTemplate.trim() || null,
				// Always send the value to allow clearing - empty array or null clears the rules
				searchPreprocessingRules: preprocessingRules,
				autoMatchConditions: autoMatchConditions,
				useExistingExternalId: form.values.useExistingExternalId,
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["plugins"] });
			notifications.show({
				title: "Success",
				message: "Search configuration updated successfully",
				color: "green",
			});
			onClose();
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Error",
				message: error.message || "Failed to update search configuration",
				color: "red",
			});
		},
	});

	const handleSubmit = () => {
		updateMutation.mutate();
	};

	return (
		<>
			<Tabs value={activeTab} onChange={setActiveTab}>
				<Tabs.List>
					<Tabs.Tab value="template" leftSection={<IconCode size={14} />}>
						Template
					</Tabs.Tab>
					<Tabs.Tab
						value="preprocessing"
						leftSection={<IconSettings size={14} />}
					>
						Preprocessing
					</Tabs.Tab>
					<Tabs.Tab value="conditions" leftSection={<IconSearch size={14} />}>
						Conditions
					</Tabs.Tab>
				</Tabs.List>

				<Stack gap="md" mt="md">
					<Tabs.Panel value="template">
						<Stack gap="md">
							<Alert
								icon={<IconInfoCircle size={16} />}
								color="blue"
								variant="light"
							>
								<Text size="sm">
									Customize the search query using Handlebars syntax. The
									template has access to series context data shown below.
								</Text>
							</Alert>

							{/* Template input */}
							<Stack gap="xs">
								<Text fw={500} size="sm">
									Search Query Template
								</Text>
								<Textarea
									placeholder="{{metadata.title}}"
									rows={2}
									styles={{ input: { fontFamily: "monospace" } }}
									{...form.getInputProps("searchQueryTemplate")}
								/>

								{/* Available helpers */}
								<Group gap="xs" align="center">
									<Text size="xs" c="dimmed">
										Helpers:
									</Text>
									{TEMPLATE_HELPERS.map((helper) => (
										<Tooltip
											key={helper.name}
											label={`${helper.description} — ${helper.example}`}
										>
											<Badge
												size="xs"
												variant="light"
												color="blue"
												style={{ cursor: "help", textTransform: "none" }}
											>
												{helper.name}
											</Badge>
										</Tooltip>
									))}
								</Group>

								{/* Live preview */}
								<Paper p="xs" withBorder bg="var(--mantine-color-dark-7)">
									<Group gap="xs">
										<Text size="xs" c="dimmed">
											Result:
										</Text>
										<Text size="xs" ff="monospace">
											{templatePreview}
										</Text>
									</Group>
								</Paper>
							</Stack>

							{/* Sample context JSON */}
							<Card padding="sm" withBorder bg="var(--mantine-color-dark-7)">
								<Stack gap="xs">
									<Group justify="space-between" align="center">
										<Text size="xs" fw={500}>
											Available Context
										</Text>
										<Text size="xs" c="dimmed">
											Access fields using dot notation, e.g.,{" "}
											<Code style={{ fontSize: 10 }}>
												{"{{metadata.title}}"}
											</Code>
										</Text>
									</Group>
									<Textarea
										size="xs"
										value={JSON.stringify(SAMPLE_SERIES_CONTEXT, null, 2)}
										readOnly
										rows={10}
										styles={{
											input: { fontFamily: "monospace", fontSize: "11px" },
										}}
									/>
								</Stack>
							</Card>
						</Stack>
					</Tabs.Panel>

					<Tabs.Panel value="preprocessing">
						<Stack gap="md">
							<Alert
								icon={<IconInfoCircle size={16} />}
								color="blue"
								variant="light"
							>
								<Text size="sm">
									Transform series titles before metadata search. Rules are
									applied in order, before the search query template.
								</Text>
							</Alert>

							<PreprocessingRulesEditor
								value={preprocessingRules}
								onChange={setPreprocessingRules}
								testInput={testTitle}
								onTestInputChange={setTestTitle}
								label="Title Preprocessing Rules"
								description="Transform series titles before metadata search. Rules are applied in order."
							/>
						</Stack>
					</Tabs.Panel>

					<Tabs.Panel value="conditions">
						<Stack gap="md">
							<Alert
								icon={<IconInfoCircle size={16} />}
								color="blue"
								variant="light"
							>
								<Text size="sm">
									Define conditions that control when auto-matching runs for
									this plugin. Without conditions, auto-matching will run for
									all series.
								</Text>
							</Alert>

							<Switch
								label="Use Existing External ID"
								description="Skip search when series already has an external ID from this plugin"
								checked={form.values.useExistingExternalId}
								onChange={(e) =>
									form.setFieldValue(
										"useExistingExternalId",
										e.currentTarget.checked,
									)
								}
							/>

							<ConditionsEditor
								value={autoMatchConditions}
								onChange={setAutoMatchConditions}
								label="Auto-Match Conditions"
								description="Define conditions that must be met for auto-matching to run."
							/>
						</Stack>
					</Tabs.Panel>
				</Stack>
			</Tabs>

			<Group justify="flex-end" mt="xl">
				<Button variant="subtle" onClick={onClose}>
					Cancel
				</Button>
				<Button onClick={handleSubmit} loading={updateMutation.isPending}>
					Save Changes
				</Button>
			</Group>
		</>
	);
}

/**
 * Modal for configuring search settings for metadata provider plugins.
 * Provides a visual interface for preprocessing rules and auto-match conditions.
 */
export function SearchConfigModal({
	plugin,
	opened,
	onClose,
}: SearchConfigModalProps) {
	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title={`Search Configuration: ${plugin.displayName}`}
			size="lg"
			centered
		>
			{/* Key forces remount when plugin changes, resetting all form state */}
			<SearchConfigContent key={plugin.id} plugin={plugin} onClose={onClose} />
		</Modal>
	);
}
