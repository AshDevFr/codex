import { AppShell } from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { useRef } from "react";
import type { SearchInputHandle } from "@/components/search";
import { useSearchShortcut } from "@/hooks/useSearchShortcut";
import { Header } from "./Header";
import { OfflineBanner } from "./OfflineBanner";
import { PluginStatusBanner } from "./PluginStatusBanner";
import { Sidebar } from "./Sidebar";

interface AppLayoutProps {
  children: React.ReactNode;
}

export function AppLayout({ children }: AppLayoutProps) {
  const [mobileOpened, { toggle: toggleMobile, close: closeMobile }] =
    useDisclosure();
  const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(true);
  const searchInputRef = useRef<SearchInputHandle>(null);

  // Below the navbar `breakpoint` (sm = 48em) the navbar collapses into a
  // fixed-position overlay (the burger "drawer"). When it's open we must lock
  // page scroll so swipes inside the menu don't bleed through to the main
  // content underneath. At >= 48em the navbar is part of the layout (toggled
  // by `desktopOpened`), so the lock must stay off even if `mobileOpened` is
  // left stale-true after a resize.
  const navOverlayActive =
    (useMediaQuery("(max-width: 47.99em)") ?? false) && mobileOpened;

  // Enable 'S' keyboard shortcut to focus search bar
  useSearchShortcut({ searchInputRef });

  return (
    <AppShell
      header={{ height: 64 }}
      navbar={{
        width: 280,
        breakpoint: "sm",
        collapsed: { mobile: !mobileOpened, desktop: !desktopOpened },
      }}
      padding="md"
    >
      <Header
        mobileOpened={mobileOpened}
        toggleMobile={toggleMobile}
        toggleDesktop={toggleDesktop}
        searchInputRef={searchInputRef}
      />
      <Sidebar onNavigate={closeMobile} scrollLocked={navOverlayActive} />

      <AppShell.Main>
        <PluginStatusBanner />
        <OfflineBanner />
        {children}
      </AppShell.Main>
    </AppShell>
  );
}
