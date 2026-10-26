import {
  ActionIcon,
  Box,
  Button,
  Group,
  type MantineSpacing,
  Select,
  Stack,
  TextInput,
} from "@mantine/core";
import { IconPlus, IconTrash } from "@tabler/icons-react";
import { useRef } from "react";
import {
  AUTHOR_ROLE_DISPLAY,
  type BookAuthor,
  type BookAuthorRole,
} from "@/types/book-metadata";

const AUTHOR_ROLE_OPTIONS = Object.entries(AUTHOR_ROLE_DISPLAY).map(
  ([value, label]) => ({ value, label }),
);

// Row keys only need to be unique within this page's lifetime. A counter is
// used instead of crypto.randomUUID(), which is missing outside secure
// contexts (Codex is often served over plain HTTP on a LAN).
let lastRowKey = 0;
const nextRowKey = () => ++lastRowKey;

export interface AuthorListEditorProps {
  authors: BookAuthor[];
  onChange: (authors: BookAuthor[]) => void;
  /** Spacing between rows and before the add button */
  gap?: MantineSpacing;
}

/**
 * Editable list of authors, one role select and name input per row.
 *
 * Authors have no id of their own, so each row gets a client-only key. Keying
 * by index (or by role, which is editable) makes React hand a deleted row's
 * elements to its neighbour, or remount a row whenever its role changes.
 */
export function AuthorListEditor({
  authors,
  onChange,
  gap = "xs",
}: AuthorListEditorProps) {
  const rowKeys = useRef<number[]>([]);
  // The list can also be replaced from outside (form reset, new data loaded);
  // a length mismatch means the keys no longer describe these rows.
  if (rowKeys.current.length !== authors.length) {
    rowKeys.current = authors.map(nextRowKey);
  }

  const updateAuthor = (index: number, updates: Partial<BookAuthor>) => {
    const next = [...authors];
    next[index] = { ...next[index], ...updates };
    onChange(next);
  };

  const removeAuthor = (index: number) => {
    rowKeys.current = rowKeys.current.filter((_, i) => i !== index);
    onChange(authors.filter((_, i) => i !== index));
  };

  const addAuthor = () => {
    rowKeys.current = [...rowKeys.current, nextRowKey()];
    onChange([...authors, { name: "", role: "author" as BookAuthorRole }]);
  };

  return (
    <Stack gap={gap}>
      {authors.map((author, index) => (
        <Group
          key={rowKeys.current[index]}
          gap="xs"
          wrap="nowrap"
          align="flex-end"
        >
          <Select
            label={index === 0 ? "Role" : undefined}
            data={AUTHOR_ROLE_OPTIONS}
            value={author.role}
            onChange={(value) => {
              if (value) {
                updateAuthor(index, { role: value as BookAuthorRole });
              }
            }}
            style={{ flex: 1 }}
            comboboxProps={{ zIndex: 1100 }}
          />
          <TextInput
            label={index === 0 ? "Name" : undefined}
            placeholder="Author name"
            value={author.name}
            onChange={(e) => updateAuthor(index, { name: e.target.value })}
            style={{ flex: 2 }}
          />
          <ActionIcon
            variant="subtle"
            color="red"
            onClick={() => removeAuthor(index)}
            aria-label="Remove author"
          >
            <IconTrash size={18} />
          </ActionIcon>
        </Group>
      ))}

      <Box>
        <Button
          variant="subtle"
          leftSection={<IconPlus size={16} />}
          onClick={addAuthor}
          size="sm"
        >
          Add Author
        </Button>
      </Box>
    </Stack>
  );
}
