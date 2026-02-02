import { useMantineColorScheme } from "@mantine/core";
import { useEffect } from "react";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";

/**
 * Component that synchronizes the user's theme preference with Mantine's color scheme.
 * This component should be rendered once inside the MantineProvider.
 */
export function ThemeSync() {
  const { setColorScheme } = useMantineColorScheme();
  const themePreference = useUserPreferencesStore((state) =>
    state.getPreference("ui.theme"),
  );

  useEffect(() => {
    // Map our preference value to Mantine's color scheme value
    // Our "system" maps to Mantine's "auto"
    const mantineScheme =
      themePreference === "system" ? "auto" : themePreference;
    setColorScheme(mantineScheme);
  }, [themePreference, setColorScheme]);

  return null;
}
