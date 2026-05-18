import {
  Drawer,
  Group,
  Loader,
  ScrollArea,
  Stack,
  Text,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { IconSearch } from "@tabler/icons-react";
import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSearch } from "@/hooks/useSearch";
import classes from "./MobileSearchSheet.module.css";
import { BookResultContent, SeriesResultContent } from "./SearchResultItem";

interface MobileSearchSheetProps {
  opened: boolean;
  onClose: () => void;
}

export function MobileSearchSheet({ opened, onClose }: MobileSearchSheetProps) {
  const [query, setQuery] = useState("");
  const navigate = useNavigate();
  const { results, isLoading } = useSearch(query);

  const series = results?.series ?? [];
  const books = results?.books ?? [];
  const hasResults = series.length > 0 || books.length > 0;
  const showResults = query.trim().length >= 2;
  const showMoreLink = series.length > 5 || books.length > 5;

  useEffect(() => {
    if (!opened) {
      setQuery("");
    }
  }, [opened]);

  const handleNavigate = useCallback(
    (path: string) => {
      onClose();
      navigate(path);
    },
    [navigate, onClose],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter" && query.trim().length >= 2) {
        event.preventDefault();
        handleNavigate(`/search?q=${encodeURIComponent(query.trim())}`);
      }
    },
    [query, handleNavigate],
  );

  return (
    <Drawer
      opened={opened}
      onClose={onClose}
      position="top"
      size="100%"
      withCloseButton
      title="Search"
      classNames={{ body: classes.body, content: "is-translucent-drawer" }}
    >
      <Stack gap="md" h="100%">
        <TextInput
          data-autofocus
          placeholder="Search series and books..."
          leftSection={
            isLoading ? <Loader size={16} /> : <IconSearch size={16} />
          }
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          onKeyDown={handleKeyDown}
          size="md"
          aria-label="Search query"
        />

        {showResults && (
          <ScrollArea style={{ flex: 1 }} type="scroll">
            {isLoading ? (
              <Group justify="center" p="md">
                <Loader size="sm" />
                <Text size="sm" c="dimmed">
                  Searching...
                </Text>
              </Group>
            ) : !hasResults ? (
              <Text ta="center" c="dimmed" py="md">
                No results found
              </Text>
            ) : (
              <Stack gap="xs">
                {series.length > 0 && (
                  <Stack gap={4}>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Series
                    </Text>
                    {series.slice(0, 5).map((s) => (
                      <UnstyledButton
                        key={s.id}
                        className={classes.option}
                        onClick={() => handleNavigate(`/series/${s.id}`)}
                      >
                        <SeriesResultContent series={s} />
                      </UnstyledButton>
                    ))}
                  </Stack>
                )}
                {books.length > 0 && (
                  <Stack gap={4}>
                    <Text size="xs" c="dimmed" tt="uppercase" fw={600}>
                      Books
                    </Text>
                    {books.slice(0, 5).map((b) => (
                      <UnstyledButton
                        key={b.id}
                        className={classes.option}
                        onClick={() => handleNavigate(`/books/${b.id}`)}
                      >
                        <BookResultContent book={b} />
                      </UnstyledButton>
                    ))}
                  </Stack>
                )}
                {showMoreLink && (
                  <UnstyledButton
                    className={classes.footer}
                    onClick={() =>
                      handleNavigate(
                        `/search?q=${encodeURIComponent(query.trim())}`,
                      )
                    }
                  >
                    <Text size="sm" c="dimmed" ta="center">
                      See all results
                    </Text>
                  </UnstyledButton>
                )}
              </Stack>
            )}
          </ScrollArea>
        )}
      </Stack>
    </Drawer>
  );
}
