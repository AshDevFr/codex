import {
  ActionIcon,
  AppShell,
  Burger,
  Group,
  Text,
  useComputedColorScheme,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconAdjustmentsHorizontal,
  IconMenu2,
  IconMoon,
  IconSearch,
  IconSun,
} from "@tabler/icons-react";
import { type RefObject, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  MobileSearchSheet,
  SearchInput,
  type SearchInputHandle,
} from "@/components/search";
import { useAppName } from "@/hooks/useAppName";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";

interface HeaderProps {
  mobileOpened: boolean;
  toggleMobile: () => void;
  toggleDesktop: () => void;
  searchInputRef?: RefObject<SearchInputHandle | null>;
}

export function Header({
  mobileOpened,
  toggleMobile,
  toggleDesktop,
  searchInputRef,
}: HeaderProps) {
  const appName = useAppName();
  const navigate = useNavigate();
  const computedColorScheme = useComputedColorScheme("dark");
  const setPreference = useUserPreferencesStore((state) => state.setPreference);
  const [
    searchSheetOpened,
    { open: openSearchSheet, close: closeSearchSheet },
  ] = useDisclosure(false);
  const [themeIconSpinning, setThemeIconSpinning] = useState(false);

  // Clear the spin marker after the keyframe completes so the next toggle
  // can re-trigger the animation. 400ms matches the keyframe duration in
  // index.css; the small buffer covers compositor scheduling.
  useEffect(() => {
    if (!themeIconSpinning) return;
    const timer = window.setTimeout(() => setThemeIconSpinning(false), 450);
    return () => window.clearTimeout(timer);
  }, [themeIconSpinning]);

  const toggleColorScheme = () => {
    setThemeIconSpinning(true);
    // Toggle between light and dark (not system) for explicit user action
    setPreference(
      "ui.theme",
      computedColorScheme === "dark" ? "light" : "dark",
    );
  };

  return (
    <AppShell.Header>
      <Group h="100%" px="md" justify="space-between">
        <Group>
          <Burger
            opened={mobileOpened}
            onClick={toggleMobile}
            hiddenFrom="sm"
            size="md"
            aria-label={mobileOpened ? "Close navigation" : "Open navigation"}
          />
          <ActionIcon
            variant="subtle"
            onClick={toggleDesktop}
            visibleFrom="sm"
            size="lg"
            title="Toggle sidebar"
          >
            <IconMenu2 size={20} />
          </ActionIcon>
          <Text size="xl" fw={700}>
            {appName}
          </Text>
        </Group>

        <Group gap="xs">
          <SearchInput ref={searchInputRef} />

          <ActionIcon
            variant="subtle"
            onClick={() => navigate("/search")}
            visibleFrom="xs"
            size="lg"
            aria-label="Advanced search"
            title="Advanced search"
          >
            <IconAdjustmentsHorizontal size={20} />
          </ActionIcon>

          <ActionIcon
            variant="subtle"
            onClick={openSearchSheet}
            hiddenFrom="xs"
            size="lg"
            aria-label="Open search"
            title="Search"
          >
            <IconSearch size={20} />
          </ActionIcon>

          <ActionIcon
            variant="subtle"
            onClick={toggleColorScheme}
            aria-label="Toggle color scheme"
            title="Toggle color scheme"
          >
            <span
              className={`theme-toggle-icon${
                themeIconSpinning ? " theme-toggle-icon--spinning" : ""
              }`}
            >
              {computedColorScheme === "dark" ? (
                <IconSun size={18} />
              ) : (
                <IconMoon size={18} />
              )}
            </span>
          </ActionIcon>
        </Group>
      </Group>

      <MobileSearchSheet
        opened={searchSheetOpened}
        onClose={closeSearchSheet}
      />
    </AppShell.Header>
  );
}
