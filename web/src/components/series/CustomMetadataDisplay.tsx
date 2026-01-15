import {
	Alert,
	Blockquote,
	Box,
	Code,
	Group,
	List,
	Table,
	Text,
} from "@mantine/core";
import { IconAlertCircle, IconPointFilled } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { renderTemplate } from "@/utils/templateEngine";

export interface CustomMetadataDisplayProps {
	/**
	 * The custom metadata object to display
	 */
	customMetadata: Record<string, unknown> | null | undefined;
	/**
	 * The Handlebars template to use for rendering.
	 * If empty or not provided, nothing will be rendered.
	 */
	template?: string;
	/**
	 * Whether to show error messages when template rendering fails
	 * @default false
	 */
	showErrors?: boolean;
}

/**
 * Parses list item content to extract label and value from "**label**: value" pattern
 */
function parseListItemContent(children: ReactNode): {
	label: string;
	value: ReactNode;
} | null {
	// Convert children to string to check for pattern
	const childArray = Array.isArray(children) ? children : [children];

	// Look for pattern: [strong element, ": ", value]
	if (childArray.length >= 2) {
		const firstChild = childArray[0];
		// Check if first child is a strong/bold element
		if (
			firstChild &&
			typeof firstChild === "object" &&
			"type" in firstChild &&
			(firstChild.type === "strong" || firstChild.type === "b")
		) {
			const label = String(
				(firstChild as { props?: { children?: ReactNode } }).props?.children ||
					"",
			);
			// Rest of the children after removing the ": " separator
			const rest = childArray.slice(1);
			// Remove leading ": " from the value
			let value: ReactNode = rest;
			if (rest.length > 0 && typeof rest[0] === "string") {
				const firstText = rest[0].replace(/^:\s*/, "");
				value = [firstText, ...rest.slice(1)];
			}
			return { label, value };
		}
	}
	return null;
}

/**
 * Displays custom metadata rendered using a Handlebars template and Markdown
 */
export function CustomMetadataDisplay({
	customMetadata,
	template,
	showErrors = false,
}: CustomMetadataDisplayProps) {
	const result = useMemo(() => {
		// If no template or no custom metadata, return empty result
		if (!template || !customMetadata || Object.keys(customMetadata).length === 0) {
			return { success: true, output: "" };
		}

		// Render the template with the custom metadata
		return renderTemplate(template, {
			custom_metadata: customMetadata,
		});
	}, [customMetadata, template]);

	// Nothing to display if empty
	if (!result.output || result.output.trim() === "") {
		return null;
	}

	// Show error if rendering failed
	if (!result.success && showErrors) {
		return (
			<Alert
				icon={<IconAlertCircle size={16} />}
				color="red"
				title="Template Error"
			>
				{result.error || "Failed to render custom metadata"}
			</Alert>
		);
	}

	// If rendering failed but we're not showing errors, return null
	if (!result.success) {
		return null;
	}

	return (
		<Box className="custom-metadata-display">
			<ReactMarkdown
				components={{
					// Customize heading styles to match the page - dimmed, uppercase labels
					h1: ({ children }) => (
						<Text size="sm" c="dimmed" tt="uppercase" mt="md" mb="xs">
							{children}
						</Text>
					),
					h2: ({ children }) => (
						<Text size="sm" c="dimmed" tt="uppercase" mt="md" mb="xs">
							{children}
						</Text>
					),
					h3: ({ children }) => (
						<Text size="sm" c="dimmed" tt="uppercase" mt="sm" mb="xs">
							{children}
						</Text>
					),
					// Style paragraphs
					p: ({ children }) => (
						<Text size="sm" mb="xs">
							{children}
						</Text>
					),
					// Style unordered lists using Mantine's List component
					ul: ({ children }) => (
						<List
							size="sm"
							spacing="xs"
							icon={<IconPointFilled size={8} style={{ color: "var(--mantine-color-dimmed)" }} />}
						>
							{children}
						</List>
					),
					// Style list items to match the metadata row pattern
					li: ({ children }) => {
						const parsed = parseListItemContent(children);
						if (parsed) {
							// Render as a styled row matching Publisher/Genre/Tags format
							return (
								<List.Item icon={<span />}>
									<Group gap="md" align="flex-start">
										<Text size="sm" c="dimmed" w={100} tt="uppercase">
											{parsed.label}
										</Text>
										<Box style={{ flex: 1 }}>
											<Text size="sm" component="span">
												{parsed.value}
											</Text>
										</Box>
									</Group>
								</List.Item>
							);
						}
						// Fallback for non-key-value list items
						return <List.Item>{children}</List.Item>;
					},
					// Hide strong tags since we extract them for labels
					strong: ({ children }) => (
						<Text component="span" fw={600} size="sm">
							{children}
						</Text>
					),
					// Style links to open in new tab for external links
					a: ({ href, children }) => (
						<Text
							component="a"
							href={href}
							target={href?.startsWith("http") ? "_blank" : undefined}
							rel={href?.startsWith("http") ? "noopener noreferrer" : undefined}
							c="blue"
							size="sm"
							style={{ textDecoration: "underline" }}
						>
							{children}
						</Text>
					),
					// Style inline code using Mantine's Code component
					code: ({ children }) => <Code fz="sm">{children}</Code>,
					// Style preformatted/code blocks
					pre: ({ children }) => (
						<Code block mb="xs">
							{children}
						</Code>
					),
					// Style blockquotes using Mantine's Blockquote
					blockquote: ({ children }) => (
						<Blockquote
							color="gray"
							icon={null}
							mt="xs"
							mb="xs"
							p="sm"
							styles={{
								root: {
									borderLeftWidth: 3,
									backgroundColor: "transparent",
								},
							}}
						>
							<Text size="sm" fs="italic" c="dimmed">
								{children}
							</Text>
						</Blockquote>
					),
					// Style deleted/strikethrough text
					del: ({ children }) => (
						<Text
							component="span"
							size="sm"
							c="dimmed"
							td="line-through"
						>
							{children}
						</Text>
					),
					// Style horizontal rules
					hr: () => (
						<Box
							component="hr"
							style={{
								border: "none",
								borderTop: "1px solid var(--mantine-color-dark-4)",
								margin: "var(--mantine-spacing-md) 0",
							}}
						/>
					),
					// Table components using Mantine's Table
					table: ({ children }) => (
						<Table
							striped
							highlightOnHover
							withTableBorder
							withColumnBorders
							mb="xs"
							fz="sm"
						>
							{children}
						</Table>
					),
					thead: ({ children }) => <Table.Thead>{children}</Table.Thead>,
					tbody: ({ children }) => <Table.Tbody>{children}</Table.Tbody>,
					tr: ({ children }) => <Table.Tr>{children}</Table.Tr>,
					th: ({ children }) => <Table.Th>{children}</Table.Th>,
					td: ({ children }) => <Table.Td>{children}</Table.Td>,
				}}
				remarkPlugins={[remarkGfm]}
			>
				{result.output}
			</ReactMarkdown>
		</Box>
	);
}
