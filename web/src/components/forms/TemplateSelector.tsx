import {
	Badge,
	Button,
	Card,
	Grid,
	Group,
	Modal,
	Stack,
	Text,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconTemplate, IconCheck } from "@tabler/icons-react";
import { useMemo, useState } from "react";
import { MarkdownContent } from "@/components/common";
import {
	EXAMPLE_TEMPLATES,
	type ExampleTemplate,
} from "@/data/exampleTemplates";
import { renderTemplate } from "@/utils/templateEngine";

export interface TemplateSelectorProps {
	/** Callback when a template is selected - receives template and its sample data */
	onSelect: (template: string, sampleData: Record<string, unknown>) => void;
	/** The currently active template (for highlighting) */
	currentTemplate?: string;
}

/**
 * A component that displays available example templates and allows
 * users to select one as a starting point for customization.
 */
export function TemplateSelector({
	onSelect,
	currentTemplate,
}: TemplateSelectorProps) {
	const [opened, { open, close }] = useDisclosure(false);
	const [selectedTemplate, setSelectedTemplate] =
		useState<ExampleTemplate | null>(null);

	const handleSelectTemplate = () => {
		if (selectedTemplate) {
			onSelect(selectedTemplate.template, selectedTemplate.sampleData);
			close();
			setSelectedTemplate(null);
		}
	};

	return (
		<>
			<Button
				variant="light"
				size="xs"
				leftSection={<IconTemplate size={14} />}
				onClick={open}
			>
				Choose Example Template
			</Button>

			<Modal
				opened={opened}
				onClose={close}
				title="Example Templates"
				size="xl"
			>
				<Stack gap="md">
					<Text size="sm" c="dimmed">
						Select a template to use as a starting point. You can customize it
						further after selecting.
					</Text>

					<Grid gutter="md">
						{EXAMPLE_TEMPLATES.map((template) => (
							<Grid.Col span={{ base: 12, sm: 6 }} key={template.id}>
								<TemplateCard
									template={template}
									isSelected={selectedTemplate?.id === template.id}
									isCurrent={currentTemplate === template.template}
									onSelect={() => setSelectedTemplate(template)}
								/>
							</Grid.Col>
						))}
					</Grid>

					<Group justify="flex-end" mt="md">
						<Button variant="subtle" onClick={close}>
							Cancel
						</Button>
						<Button onClick={handleSelectTemplate} disabled={!selectedTemplate}>
							Use Template
						</Button>
					</Group>
				</Stack>
			</Modal>
		</>
	);
}

/**
 * Individual template card with preview
 */
function TemplateCard({
	template,
	isSelected,
	isCurrent,
	onSelect,
}: {
	template: ExampleTemplate;
	isSelected: boolean;
	isCurrent: boolean;
	onSelect: () => void;
}) {
	// Render preview with sample data
	const preview = useMemo(() => {
		return renderTemplate(template.template, {
			custom_metadata: template.sampleData,
		});
	}, [template]);

	return (
		<Card
			withBorder
			padding="sm"
			onClick={onSelect}
			style={{
				cursor: "pointer",
				borderColor: isSelected
					? "var(--mantine-color-blue-6)"
					: isCurrent
						? "var(--mantine-color-green-6)"
						: undefined,
				borderWidth: isSelected || isCurrent ? 2 : 1,
			}}
		>
			<Stack gap="xs">
				{/* Header */}
				<Group justify="space-between" align="flex-start">
					<Stack gap={2}>
						<Group gap="xs">
							<Text fw={500} size="sm">
								{template.name}
							</Text>
							{isCurrent && (
								<Badge size="xs" color="green" variant="light">
									<Group gap={4}>
										<IconCheck size={10} />
										Current
									</Group>
								</Badge>
							)}
						</Group>
						<Text size="xs" c="dimmed" lineClamp={2}>
							{template.description}
						</Text>
					</Stack>
				</Group>

				{/* Tags */}
				<Group gap={4}>
					{template.tags.slice(0, 3).map((tag) => (
						<Badge key={tag} size="xs" variant="light" color="gray">
							{tag}
						</Badge>
					))}
				</Group>

				{/* Preview */}
				<Card withBorder padding="xs">
					<Text size="xs" c="dimmed" mb={4}>
						Preview:
					</Text>
					{preview.success && preview.output.trim() ? (
						<div style={{ maxHeight: 100, overflow: "hidden" }}>
							<MarkdownContent compact>{preview.output}</MarkdownContent>
						</div>
					) : (
						<Text size="xs" c="dimmed" fs="italic">
							{preview.success ? "(no output)" : "Error rendering"}
						</Text>
					)}
				</Card>
			</Stack>
		</Card>
	);
}
