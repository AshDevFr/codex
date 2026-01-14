import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { userIntegrationsApi } from "@/api/userIntegrations";
import {
	selectAvailableIntegrations,
	selectIntegration,
	selectIntegrations,
	selectIsConnected,
	selectIsLoaded,
	selectIsLoading,
	selectLoadError,
	selectOperationStatus,
	useUserIntegrationsStore,
} from "./userIntegrationsStore";

// Mock the API client
vi.mock("@/api/userIntegrations", () => ({
	userIntegrationsApi: {
		getAll: vi.fn(),
		getByName: vi.fn(),
		connect: vi.fn(),
		oauthCallback: vi.fn(),
		update: vi.fn(),
		enable: vi.fn(),
		disable: vi.fn(),
		disconnect: vi.fn(),
		sync: vi.fn(),
	},
}));

const mockIntegration = {
	id: "550e8400-e29b-41d4-a716-446655440000",
	integrationName: "anilist",
	displayName: "My AniList",
	connected: true,
	enabled: true,
	externalUserId: "123456",
	externalUsername: "testuser",
	settings: { sync_progress: true, sync_ratings: true },
	lastSyncAt: "2024-01-15T18:00:00Z",
	syncStatus: "idle",
	lastError: null,
	tokenExpiresAt: "2024-02-15T18:00:00Z",
	createdAt: "2024-01-01T00:00:00Z",
	updatedAt: "2024-01-15T18:00:00Z",
};

const mockAvailableIntegration = {
	name: "anilist",
	displayName: "AniList",
	description: "Sync your reading progress and ratings with AniList",
	authType: "oauth2",
	features: ["sync_progress", "sync_ratings", "import_lists"],
	connected: false,
};

const mockMangadexIntegration = {
	...mockIntegration,
	id: "550e8400-e29b-41d4-a716-446655440001",
	integrationName: "mangadex",
	displayName: null,
};

const mockMangadexAvailable = {
	name: "mangadex",
	displayName: "MangaDex",
	description: "Sync your reading progress with MangaDex",
	authType: "api_key",
	features: ["sync_progress"],
	connected: false,
};

describe("userIntegrationsStore", () => {
	beforeEach(() => {
		// Reset store state before each test
		useUserIntegrationsStore.setState({
			integrations: [],
			available: [],
			isLoaded: false,
			isLoading: false,
			loadError: null,
			operationStatus: {},
		});
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("initial state", () => {
		it("should have empty integrations list", () => {
			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toEqual([]);
		});

		it("should have empty available list", () => {
			const state = useUserIntegrationsStore.getState();
			expect(state.available).toEqual([]);
		});

		it("should not be loaded initially", () => {
			const state = useUserIntegrationsStore.getState();
			expect(state.isLoaded).toBe(false);
		});

		it("should not be loading initially", () => {
			const state = useUserIntegrationsStore.getState();
			expect(state.isLoading).toBe(false);
		});

		it("should have no load error", () => {
			const state = useUserIntegrationsStore.getState();
			expect(state.loadError).toBeNull();
		});
	});

	describe("loadFromServer", () => {
		it("should load integrations from server", async () => {
			vi.mocked(userIntegrationsApi.getAll).mockResolvedValue({
				integrations: [mockIntegration],
				available: [mockAvailableIntegration],
			});

			const { loadFromServer } = useUserIntegrationsStore.getState();

			await act(async () => {
				await loadFromServer();
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toHaveLength(1);
			expect(state.integrations[0].integrationName).toBe("anilist");
			expect(state.available).toHaveLength(1);
			expect(state.isLoaded).toBe(true);
			expect(state.isLoading).toBe(false);
		});

		it("should set isLoading during load", async () => {
			let resolveLoad: () => void;
			const loadPromise = new Promise<{
				integrations: (typeof mockIntegration)[];
				available: (typeof mockAvailableIntegration)[];
			}>((resolve) => {
				resolveLoad = () => resolve({ integrations: [], available: [] });
			});

			vi.mocked(userIntegrationsApi.getAll).mockReturnValue(loadPromise);

			const { loadFromServer } = useUserIntegrationsStore.getState();

			// Start loading
			const loadOperation = loadFromServer();

			// Should be loading
			expect(useUserIntegrationsStore.getState().isLoading).toBe(true);

			// Complete the load
			await act(async () => {
				resolveLoad?.();
				await loadOperation;
			});

			// Should no longer be loading
			expect(useUserIntegrationsStore.getState().isLoading).toBe(false);
		});

		it("should set loadError on failure", async () => {
			// Suppress expected console.error for this test
			const consoleErrorSpy = vi
				.spyOn(console, "error")
				.mockImplementation(() => {});

			vi.mocked(userIntegrationsApi.getAll).mockRejectedValue(
				new Error("Network error"),
			);

			const { loadFromServer } = useUserIntegrationsStore.getState();

			await act(async () => {
				try {
					await loadFromServer();
				} catch {
					// Expected to throw
				}
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.loadError).toBe("Network error");
			expect(state.isLoading).toBe(false);

			consoleErrorSpy.mockRestore();
		});
	});

	describe("getIntegration", () => {
		it("should return integration by name", () => {
			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
			});

			const { getIntegration } = useUserIntegrationsStore.getState();
			const integration = getIntegration("anilist");

			expect(integration).toBeDefined();
			expect(integration?.integrationName).toBe("anilist");
		});

		it("should return undefined for non-existent integration", () => {
			const { getIntegration } = useUserIntegrationsStore.getState();
			const integration = getIntegration("non-existent");

			expect(integration).toBeUndefined();
		});
	});

	describe("getAvailableIntegration", () => {
		it("should return available integration by name", () => {
			useUserIntegrationsStore.setState({
				available: [mockAvailableIntegration],
			});

			const { getAvailableIntegration } = useUserIntegrationsStore.getState();
			const available = getAvailableIntegration("anilist");

			expect(available).toBeDefined();
			expect(available?.displayName).toBe("AniList");
		});

		it("should return undefined for non-existent available integration", () => {
			const { getAvailableIntegration } = useUserIntegrationsStore.getState();
			const available = getAvailableIntegration("non-existent");

			expect(available).toBeUndefined();
		});
	});

	describe("isConnected", () => {
		it("should return true for connected integration", () => {
			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
			});

			const { isConnected } = useUserIntegrationsStore.getState();
			expect(isConnected("anilist")).toBe(true);
		});

		it("should return false for non-connected integration", () => {
			const { isConnected } = useUserIntegrationsStore.getState();
			expect(isConnected("anilist")).toBe(false);
		});
	});

	describe("connect", () => {
		it("should initiate OAuth connection", async () => {
			vi.mocked(userIntegrationsApi.connect).mockResolvedValue({
				authUrl: "https://anilist.co/api/v2/oauth/authorize",
				connected: false,
				integration: null,
			});

			useUserIntegrationsStore.setState({
				available: [mockAvailableIntegration],
			});

			const { connect } = useUserIntegrationsStore.getState();

			let result!: { authUrl?: string; connected: boolean };
			await act(async () => {
				result = await connect({
					integrationName: "anilist",
					redirectUri: "https://app.example.com/callback",
				});
			});

			expect(result.authUrl).toBe("https://anilist.co/api/v2/oauth/authorize");
			expect(result.connected).toBe(false);
		});

		it("should connect API key integration immediately", async () => {
			vi.mocked(userIntegrationsApi.connect).mockResolvedValue({
				authUrl: null,
				connected: true,
				integration: mockMangadexIntegration,
			});

			useUserIntegrationsStore.setState({
				available: [mockMangadexAvailable],
			});

			const { connect } = useUserIntegrationsStore.getState();

			let result!: { authUrl?: string; connected: boolean };
			await act(async () => {
				result = await connect({
					integrationName: "mangadex",
					apiKey: "my-api-key",
				});
			});

			expect(result.connected).toBe(true);
			expect(result.authUrl).toBeUndefined();

			// Should add to integrations list
			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toHaveLength(1);
			expect(state.integrations[0].integrationName).toBe("mangadex");

			// Should update available list
			expect(state.available[0].connected).toBe(true);
		});

		it("should set operation status during connect", async () => {
			let resolveConnect: (value: {
				authUrl: string | null;
				connected: boolean;
				integration: null;
			}) => void;
			const connectPromise = new Promise<{
				authUrl: string | null;
				connected: boolean;
				integration: null;
			}>((resolve) => {
				resolveConnect = resolve;
			});

			vi.mocked(userIntegrationsApi.connect).mockReturnValue(connectPromise);

			const { connect } = useUserIntegrationsStore.getState();

			// Start connecting
			const connectOperation = connect({
				integrationName: "anilist",
				redirectUri: "https://app.example.com/callback",
			});

			// Should be loading
			expect(useUserIntegrationsStore.getState().operationStatus.anilist).toBe(
				"loading",
			);

			// Complete the connect
			await act(async () => {
				resolveConnect?.({
					authUrl: "https://anilist.co/oauth",
					connected: false,
					integration: null,
				});
				await connectOperation;
			});

			// Should no longer be loading
			expect(useUserIntegrationsStore.getState().operationStatus.anilist).toBe(
				"idle",
			);
		});

		it("should set error status on connect failure", async () => {
			vi.mocked(userIntegrationsApi.connect).mockRejectedValue(
				new Error("Connection failed"),
			);

			const { connect } = useUserIntegrationsStore.getState();

			await act(async () => {
				try {
					await connect({
						integrationName: "anilist",
						redirectUri: "https://app.example.com/callback",
					});
				} catch {
					// Expected to throw
				}
			});

			expect(useUserIntegrationsStore.getState().operationStatus.anilist).toBe(
				"error",
			);
		});
	});

	describe("completeOAuth", () => {
		it("should complete OAuth and add integration", async () => {
			vi.mocked(userIntegrationsApi.oauthCallback).mockResolvedValue(
				mockIntegration,
			);

			useUserIntegrationsStore.setState({
				available: [mockAvailableIntegration],
			});

			const { completeOAuth } = useUserIntegrationsStore.getState();

			await act(async () => {
				await completeOAuth(
					"anilist",
					"auth-code",
					"csrf-state",
					"https://app.example.com/callback",
				);
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toHaveLength(1);
			expect(state.available[0].connected).toBe(true);
		});

		it("should update existing integration on re-auth", async () => {
			const updatedIntegration = {
				...mockIntegration,
				externalUsername: "newuser",
			};

			vi.mocked(userIntegrationsApi.oauthCallback).mockResolvedValue(
				updatedIntegration,
			);

			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
				available: [{ ...mockAvailableIntegration, connected: true }],
			});

			const { completeOAuth } = useUserIntegrationsStore.getState();

			await act(async () => {
				await completeOAuth(
					"anilist",
					"auth-code",
					"csrf-state",
					"https://app.example.com/callback",
				);
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toHaveLength(1);
			expect(state.integrations[0].externalUsername).toBe("newuser");
		});
	});

	describe("updateSettings", () => {
		it("should update integration settings", async () => {
			const updatedIntegration = {
				...mockIntegration,
				settings: { sync_progress: false, sync_ratings: true },
			};

			vi.mocked(userIntegrationsApi.update).mockResolvedValue(
				updatedIntegration,
			);

			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
			});

			const { updateSettings } = useUserIntegrationsStore.getState();

			await act(async () => {
				await updateSettings("anilist", {
					settings: { sync_progress: false, sync_ratings: true },
				});
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations[0].settings).toEqual({
				sync_progress: false,
				sync_ratings: true,
			});
		});
	});

	describe("enable", () => {
		it("should enable integration", async () => {
			const enabledIntegration = { ...mockIntegration, enabled: true };

			vi.mocked(userIntegrationsApi.enable).mockResolvedValue(
				enabledIntegration,
			);

			useUserIntegrationsStore.setState({
				integrations: [{ ...mockIntegration, enabled: false }],
			});

			const { enable } = useUserIntegrationsStore.getState();

			await act(async () => {
				await enable("anilist");
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations[0].enabled).toBe(true);
		});
	});

	describe("disable", () => {
		it("should disable integration", async () => {
			const disabledIntegration = { ...mockIntegration, enabled: false };

			vi.mocked(userIntegrationsApi.disable).mockResolvedValue(
				disabledIntegration,
			);

			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
			});

			const { disable } = useUserIntegrationsStore.getState();

			await act(async () => {
				await disable("anilist");
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations[0].enabled).toBe(false);
		});
	});

	describe("disconnect", () => {
		it("should disconnect and remove integration", async () => {
			vi.mocked(userIntegrationsApi.disconnect).mockResolvedValue(undefined);

			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
				available: [{ ...mockAvailableIntegration, connected: true }],
			});

			const { disconnect } = useUserIntegrationsStore.getState();

			await act(async () => {
				await disconnect("anilist");
			});

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toHaveLength(0);
			expect(state.available[0].connected).toBe(false);
		});
	});

	describe("sync", () => {
		it("should trigger sync and update integration", async () => {
			const syncingIntegration = { ...mockIntegration, syncStatus: "idle" };

			vi.mocked(userIntegrationsApi.sync).mockResolvedValue({
				started: true,
				message: "Sync completed",
				integration: syncingIntegration,
			});

			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
			});

			const { sync } = useUserIntegrationsStore.getState();

			let result!: { started: boolean; message: string };
			await act(async () => {
				result = await sync("anilist");
			});

			expect(result.started).toBe(true);
			expect(result.message).toBe("Sync completed");
		});
	});

	describe("clearCache", () => {
		it("should clear all state", () => {
			useUserIntegrationsStore.setState({
				integrations: [mockIntegration],
				available: [mockAvailableIntegration],
				isLoaded: true,
				isLoading: false,
				loadError: "Some error",
				operationStatus: { anilist: "loading" },
			});

			const { clearCache } = useUserIntegrationsStore.getState();
			clearCache();

			const state = useUserIntegrationsStore.getState();
			expect(state.integrations).toEqual([]);
			expect(state.available).toEqual([]);
			expect(state.isLoaded).toBe(false);
			expect(state.isLoading).toBe(false);
			expect(state.loadError).toBeNull();
			expect(state.operationStatus).toEqual({});
		});
	});

	describe("selectors", () => {
		describe("selectIntegrations", () => {
			it("should select all integrations", () => {
				useUserIntegrationsStore.setState({
					integrations: [mockIntegration],
				});

				const state = useUserIntegrationsStore.getState();
				const result = selectIntegrations(state);

				expect(result).toHaveLength(1);
				expect(result[0].integrationName).toBe("anilist");
			});
		});

		describe("selectAvailableIntegrations", () => {
			it("should select all available integrations", () => {
				useUserIntegrationsStore.setState({
					available: [mockAvailableIntegration],
				});

				const state = useUserIntegrationsStore.getState();
				const result = selectAvailableIntegrations(state);

				expect(result).toHaveLength(1);
				expect(result[0].name).toBe("anilist");
			});
		});

		describe("selectIntegration", () => {
			it("should select a specific integration", () => {
				useUserIntegrationsStore.setState({
					integrations: [mockIntegration],
				});

				const state = useUserIntegrationsStore.getState();
				const result = selectIntegration("anilist")(state);

				expect(result?.integrationName).toBe("anilist");
			});

			it("should return undefined for non-existent integration", () => {
				const state = useUserIntegrationsStore.getState();
				const result = selectIntegration("non-existent")(state);

				expect(result).toBeUndefined();
			});
		});

		describe("selectIsConnected", () => {
			it("should return true for connected integration", () => {
				useUserIntegrationsStore.setState({
					integrations: [mockIntegration],
				});

				const state = useUserIntegrationsStore.getState();
				expect(selectIsConnected("anilist")(state)).toBe(true);
			});

			it("should return false for non-connected integration", () => {
				const state = useUserIntegrationsStore.getState();
				expect(selectIsConnected("anilist")(state)).toBe(false);
			});
		});

		describe("selectIsLoaded", () => {
			it("should select isLoaded state", () => {
				useUserIntegrationsStore.setState({ isLoaded: true });

				const state = useUserIntegrationsStore.getState();
				expect(selectIsLoaded(state)).toBe(true);
			});
		});

		describe("selectIsLoading", () => {
			it("should select isLoading state", () => {
				useUserIntegrationsStore.setState({ isLoading: true });

				const state = useUserIntegrationsStore.getState();
				expect(selectIsLoading(state)).toBe(true);
			});
		});

		describe("selectLoadError", () => {
			it("should select loadError state", () => {
				useUserIntegrationsStore.setState({ loadError: "Network error" });

				const state = useUserIntegrationsStore.getState();
				expect(selectLoadError(state)).toBe("Network error");
			});
		});

		describe("selectOperationStatus", () => {
			it("should select operation status for integration", () => {
				useUserIntegrationsStore.setState({
					operationStatus: { anilist: "loading" },
				});

				const state = useUserIntegrationsStore.getState();
				expect(selectOperationStatus("anilist")(state)).toBe("loading");
			});

			it("should return idle for unknown integration", () => {
				const state = useUserIntegrationsStore.getState();
				expect(selectOperationStatus("unknown")(state)).toBe("idle");
			});
		});
	});
});
