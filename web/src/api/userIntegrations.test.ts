import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { userIntegrationsApi } from "./userIntegrations";
import { api } from "./client";

// Mock the api client
vi.mock("./client", () => ({
	api: {
		get: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
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

describe("userIntegrationsApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all user integrations and available integrations", async () => {
			const mockResponse = {
				integrations: [mockIntegration],
				available: [mockAvailableIntegration],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await userIntegrationsApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/user/integrations");
			expect(result).toEqual(mockResponse);
			expect(result.integrations).toHaveLength(1);
			expect(result.available).toHaveLength(1);
		});

		it("should return empty lists when no integrations exist", async () => {
			const mockResponse = {
				integrations: [],
				available: [mockAvailableIntegration],
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await userIntegrationsApi.getAll();

			expect(result.integrations).toHaveLength(0);
			expect(result.available).toHaveLength(1);
		});
	});

	describe("getByName", () => {
		it("should fetch a single integration by name", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: mockIntegration });

			const result = await userIntegrationsApi.getByName("anilist");

			expect(api.get).toHaveBeenCalledWith("/user/integrations/anilist");
			expect(result).toEqual(mockIntegration);
		});

		it("should throw for non-existent integration", async () => {
			const error = { response: { status: 404 } };
			vi.mocked(api.get).mockRejectedValueOnce(error);

			await expect(
				userIntegrationsApi.getByName("non-existent"),
			).rejects.toEqual(error);
		});

		it("should encode special characters in integration name", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: mockIntegration });

			await userIntegrationsApi.getByName("my/integration");

			expect(api.get).toHaveBeenCalledWith(
				"/user/integrations/my%2Fintegration",
			);
		});
	});

	describe("connect", () => {
		it("should initiate OAuth connection and return auth URL", async () => {
			const request = {
				integrationName: "anilist",
				redirectUri: "https://app.example.com/callback",
			};

			const mockResponse = {
				authUrl: "https://anilist.co/api/v2/oauth/authorize?client_id=...",
				connected: false,
				integration: null,
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await userIntegrationsApi.connect(request);

			expect(api.post).toHaveBeenCalledWith("/user/integrations", request);
			expect(result.authUrl).toBe(mockResponse.authUrl);
			expect(result.connected).toBe(false);
		});

		it("should connect API key integration immediately", async () => {
			const request = {
				integrationName: "mangadex",
				apiKey: "my-api-key",
			};

			const mockResponse = {
				authUrl: null,
				connected: true,
				integration: { ...mockIntegration, integrationName: "mangadex" },
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await userIntegrationsApi.connect(request);

			expect(api.post).toHaveBeenCalledWith("/user/integrations", request);
			expect(result.connected).toBe(true);
			expect(result.integration).toBeDefined();
		});

		it("should handle connection error", async () => {
			const request = {
				integrationName: "anilist",
				redirectUri: "https://app.example.com/callback",
			};

			const error = { response: { status: 409, data: { message: "Already connected" } } };
			vi.mocked(api.post).mockRejectedValueOnce(error);

			await expect(userIntegrationsApi.connect(request)).rejects.toEqual(error);
		});
	});

	describe("oauthCallback", () => {
		it("should complete OAuth callback", async () => {
			const callbackRequest = {
				code: "auth-code-123",
				state: "csrf-state",
				redirectUri: "https://app.example.com/callback",
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockIntegration });

			const result = await userIntegrationsApi.oauthCallback(
				"anilist",
				callbackRequest,
			);

			expect(api.post).toHaveBeenCalledWith(
				"/user/integrations/anilist/callback",
				callbackRequest,
			);
			expect(result).toEqual(mockIntegration);
		});

		it("should handle invalid callback", async () => {
			const callbackRequest = {
				code: "invalid-code",
				state: "wrong-state",
				redirectUri: "https://app.example.com/callback",
			};

			const error = { response: { status: 400 } };
			vi.mocked(api.post).mockRejectedValueOnce(error);

			await expect(
				userIntegrationsApi.oauthCallback("anilist", callbackRequest),
			).rejects.toEqual(error);
		});
	});

	describe("update", () => {
		it("should update integration settings", async () => {
			const updateRequest = {
				settings: { sync_progress: false, sync_ratings: true },
			};

			const updatedIntegration = {
				...mockIntegration,
				settings: updateRequest.settings,
			};

			vi.mocked(api.patch).mockResolvedValueOnce({ data: updatedIntegration });

			const result = await userIntegrationsApi.update("anilist", updateRequest);

			expect(api.patch).toHaveBeenCalledWith(
				"/user/integrations/anilist",
				updateRequest,
			);
			expect(result.settings).toEqual(updateRequest.settings);
		});

		it("should update integration display name", async () => {
			const updateRequest = {
				displayName: "Updated AniList",
			};

			const updatedIntegration = {
				...mockIntegration,
				displayName: "Updated AniList",
			};

			vi.mocked(api.patch).mockResolvedValueOnce({ data: updatedIntegration });

			const result = await userIntegrationsApi.update("anilist", updateRequest);

			expect(result.displayName).toBe("Updated AniList");
		});
	});

	describe("enable", () => {
		it("should enable an integration", async () => {
			const enabledIntegration = { ...mockIntegration, enabled: true };

			vi.mocked(api.patch).mockResolvedValueOnce({ data: enabledIntegration });

			const result = await userIntegrationsApi.enable("anilist");

			expect(api.patch).toHaveBeenCalledWith("/user/integrations/anilist", {
				enabled: true,
			});
			expect(result.enabled).toBe(true);
		});
	});

	describe("disable", () => {
		it("should disable an integration", async () => {
			const disabledIntegration = { ...mockIntegration, enabled: false };

			vi.mocked(api.patch).mockResolvedValueOnce({ data: disabledIntegration });

			const result = await userIntegrationsApi.disable("anilist");

			expect(api.patch).toHaveBeenCalledWith("/user/integrations/anilist", {
				enabled: false,
			});
			expect(result.enabled).toBe(false);
		});
	});

	describe("disconnect", () => {
		it("should disconnect an integration", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await userIntegrationsApi.disconnect("anilist");

			expect(api.delete).toHaveBeenCalledWith("/user/integrations/anilist");
		});

		it("should throw for non-existent integration", async () => {
			const error = { response: { status: 404 } };
			vi.mocked(api.delete).mockRejectedValueOnce(error);

			await expect(
				userIntegrationsApi.disconnect("non-existent"),
			).rejects.toEqual(error);
		});
	});

	describe("sync", () => {
		it("should trigger a sync for an integration", async () => {
			const mockResponse = {
				started: true,
				message: "Sync started",
				integration: { ...mockIntegration, syncStatus: "syncing" },
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await userIntegrationsApi.sync("anilist");

			expect(api.post).toHaveBeenCalledWith("/user/integrations/anilist/sync");
			expect(result.started).toBe(true);
			expect(result.integration.syncStatus).toBe("syncing");
		});

		it("should handle sync conflict (already syncing)", async () => {
			const error = { response: { status: 409 } };
			vi.mocked(api.post).mockRejectedValueOnce(error);

			await expect(userIntegrationsApi.sync("anilist")).rejects.toEqual(error);
		});

		it("should handle disabled integration sync attempt", async () => {
			const error = { response: { status: 400 } };
			vi.mocked(api.post).mockRejectedValueOnce(error);

			await expect(userIntegrationsApi.sync("anilist")).rejects.toEqual(error);
		});
	});
});
