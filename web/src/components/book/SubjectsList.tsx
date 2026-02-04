import { Badge, Group, Text, Tooltip } from "@mantine/core";
import { IconTag } from "@tabler/icons-react";
import { parseSubjects } from "@/types/book-metadata";

interface SubjectsListProps {
  /** Subjects as JSON array string or comma-separated string */
  subjects: string | string[] | null | undefined;
  /** Maximum number of subjects to display before collapsing */
  maxDisplay?: number;
  /** Size of badges */
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  /** Whether subjects are clickable (for filtering) */
  clickable?: boolean;
  /** Callback when a subject is clicked */
  onSubjectClick?: (subject: string) => void;
}

/**
 * Component to display a list of subject/topic tags.
 * Supports both JSON array and comma-separated string formats.
 */
export function SubjectsList({
  subjects,
  maxDisplay,
  size = "sm",
  clickable = false,
  onSubjectClick,
}: SubjectsListProps) {
  // Parse subjects
  const subjectList: string[] =
    typeof subjects === "string"
      ? parseSubjects(subjects)
      : Array.isArray(subjects)
        ? subjects
        : [];

  if (subjectList.length === 0) return null;

  // Apply maxDisplay limit
  const displaySubjects = maxDisplay
    ? subjectList.slice(0, maxDisplay)
    : subjectList;
  const hiddenCount = subjectList.length - displaySubjects.length;

  return (
    <Group gap="xs" wrap="wrap">
      {displaySubjects.map((subject) => (
        <Badge
          key={subject}
          variant="outline"
          color="gray"
          size={size}
          leftSection={<IconTag size={10} />}
          style={{
            cursor: clickable ? "pointer" : "default",
            textTransform: "none",
          }}
          onClick={
            clickable && onSubjectClick
              ? () => onSubjectClick(subject)
              : undefined
          }
        >
          {subject}
        </Badge>
      ))}
      {hiddenCount > 0 && (
        <Tooltip label={subjectList.slice(maxDisplay).join(", ")}>
          <Text size="xs" c="dimmed" style={{ cursor: "help" }}>
            +{hiddenCount} more
          </Text>
        </Tooltip>
      )}
    </Group>
  );
}

/**
 * Compact display showing subject count.
 * Useful for card views where space is limited.
 */
export function SubjectsCount({
  subjects,
}: Pick<SubjectsListProps, "subjects">) {
  const subjectList: string[] =
    typeof subjects === "string"
      ? parseSubjects(subjects)
      : Array.isArray(subjects)
        ? subjects
        : [];

  if (subjectList.length === 0) return null;

  return (
    <Tooltip label={subjectList.join(", ")}>
      <Group gap={4}>
        <IconTag size={14} />
        <Text size="xs">{subjectList.length} subjects</Text>
      </Group>
    </Tooltip>
  );
}
