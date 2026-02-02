import { ActionIcon, Box, Group, Stack, Text, Title } from "@mantine/core";
import { IconChevronLeft, IconChevronRight } from "@tabler/icons-react";
import { type ReactNode, useCallback, useRef, useState } from "react";
import classes from "./HorizontalCarousel.module.css";

interface HorizontalCarouselProps {
  title: string;
  subtitle?: string;
  children: ReactNode;
}

export function HorizontalCarousel({
  title,
  subtitle,
  children,
}: HorizontalCarouselProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(true);

  const updateScrollButtons = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const { scrollLeft, scrollWidth, clientWidth } = container;
    setCanScrollLeft(scrollLeft > 0);
    setCanScrollRight(scrollLeft + clientWidth < scrollWidth - 1);
  }, []);

  const scroll = useCallback((direction: "left" | "right") => {
    const container = scrollContainerRef.current;
    if (!container) return;

    // Scroll by approximately 4 items worth
    const scrollAmount = container.clientWidth * 0.8;
    const newScrollLeft =
      direction === "left"
        ? container.scrollLeft - scrollAmount
        : container.scrollLeft + scrollAmount;

    container.scrollTo({
      left: newScrollLeft,
      behavior: "smooth",
    });
  }, []);

  return (
    <Stack gap="sm">
      <Group justify="space-between" align="flex-end">
        <Box>
          <Title order={2}>{title}</Title>
          {subtitle && (
            <Text size="sm" c="dimmed">
              {subtitle}
            </Text>
          )}
        </Box>
        <Group gap="xs">
          <ActionIcon
            variant="subtle"
            size="lg"
            onClick={() => scroll("left")}
            disabled={!canScrollLeft}
            aria-label="Scroll left"
          >
            <IconChevronLeft size={20} />
          </ActionIcon>
          <ActionIcon
            variant="subtle"
            size="lg"
            onClick={() => scroll("right")}
            disabled={!canScrollRight}
            aria-label="Scroll right"
          >
            <IconChevronRight size={20} />
          </ActionIcon>
        </Group>
      </Group>

      <div
        ref={scrollContainerRef}
        className={classes.scrollContainer}
        onScroll={updateScrollButtons}
      >
        <div className={classes.itemsContainer}>{children}</div>
      </div>
    </Stack>
  );
}
