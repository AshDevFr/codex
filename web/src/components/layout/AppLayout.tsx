import { AppShell } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";

interface AppLayoutProps {
	children: React.ReactNode;
	currentPath?: string;
}

export function AppLayout({ children, currentPath }: AppLayoutProps) {
	const [mobileOpened, { toggle: toggleMobile }] = useDisclosure();
	const [desktopOpened, { toggle: toggleDesktop }] = useDisclosure(true);

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
			/>
			<Sidebar currentPath={currentPath} />

			<AppShell.Main>{children}</AppShell.Main>
		</AppShell>
	);
}
