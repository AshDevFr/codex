import {
	ActionIcon,
	AppShell,
	Burger,
	Group,
	Text,
	useComputedColorScheme,
} from "@mantine/core";
import { IconMenu2, IconMoon, IconSun } from "@tabler/icons-react";
import type { RefObject } from "react";
import { SearchInput, type SearchInputHandle } from "@/components/search";
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
	const computedColorScheme = useComputedColorScheme("dark");
	const setPreference = useUserPreferencesStore((state) => state.setPreference);

	const toggleColorScheme = () => {
		// Toggle between light and dark (not system) for explicit user action
		setPreference("ui.theme", computedColorScheme === "dark" ? "light" : "dark");
	};

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
					<SearchInput ref={searchInputRef} />

					<ActionIcon
						variant="subtle"
						onClick={toggleColorScheme}
						title="Toggle color scheme"
					>
						{computedColorScheme === "dark" ? (
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
