import {
	ActionIcon,
	AppShell,
	Burger,
	Group,
	Text,
	useMantineColorScheme,
} from "@mantine/core";
import { IconMenu2, IconMoon, IconSun } from "@tabler/icons-react";
import { SearchInput } from "@/components/search";

interface HeaderProps {
	mobileOpened: boolean;
	toggleMobile: () => void;
	toggleDesktop: () => void;
}

export function Header({
	mobileOpened,
	toggleMobile,
	toggleDesktop,
}: HeaderProps) {
	const { colorScheme, toggleColorScheme } = useMantineColorScheme();

	return (
		<AppShell.Header>
			<Group h="100%" px="md" justify="space-between">
				<Group>
					<Burger
						opened={mobileOpened}
						onClick={toggleMobile}
						hiddenFrom="sm"
						size="sm"
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
						Codex
					</Text>
				</Group>

				<Group>
					<SearchInput />

					<ActionIcon
						variant="subtle"
						onClick={toggleColorScheme}
						title="Toggle color scheme"
					>
						{colorScheme === "dark" ? (
							<IconSun size={18} />
						) : (
							<IconMoon size={18} />
						)}
					</ActionIcon>
				</Group>
			</Group>
		</AppShell.Header>
	);
}
