import { AppShell } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useRef } from "react";
import type { SearchInputHandle } from "@/components/search";
import { useSearchShortcut } from "@/hooks/useSearchShortcut";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";

interface AppLayoutProps {
	children: React.ReactNode;
	currentPath?: string;
}

export function AppLayout({ children, currentPath }: AppLayoutProps) {
	const [mobileOpened, { toggle: toggleMobile }] = useDisclosure();
	const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(true);
	const searchInputRef = useRef<SearchInputHandle>(null);

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
			<Sidebar currentPath={currentPath} />

			<AppShell.Main>{children}</AppShell.Main>
		</AppShell>
	);
}
