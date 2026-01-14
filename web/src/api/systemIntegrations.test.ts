import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";
import { systemIntegrationsApi } from "./systemIntegrations";

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
	name: "mangaupdates",
	displayName: "MangaUpdates",
	integrationType: "metadata_provider",
	config: { rate_limit: 60 },
	hasCredentials: true,
	enabled: true,
	healthStatus: "healthy",
	lastHealthCheckAt: "2024-01-15T18:45:00Z",
	lastSyncAt: "2024-01-15T18:00:00Z",
	errorMessage: null,
	createdAt: "2024-01-01T00:00:00Z",
	updatedAt: "2024-01-15T18:45:00Z",
};

describe("systemIntegrationsApi", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe("getAll", () => {
		it("should fetch all system integrations", async () => {
			const mockResponse = {
				integrations: [mockIntegration],
				total: 1,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.getAll();

			expect(api.get).toHaveBeenCalledWith("/admin/integrations");
			expect(result).toEqual(mockResponse);
			expect(result.integrations).toHaveLength(1);
			expect(result.total).toBe(1);
		});

		it("should return empty list when no integrations exist", async () => {
			const mockResponse = {
				integrations: [],
				total: 0,
			};

			vi.mocked(api.get).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.getAll();

			expect(result.integrations).toHaveLength(0);
			expect(result.total).toBe(0);
		});
	});

	describe("getById", () => {
		it("should fetch a single integration by ID", async () => {
			vi.mocked(api.get).mockResolvedValueOnce({ data: mockIntegration });

			const result = await systemIntegrationsApi.getById(mockIntegration.id);

			expect(api.get).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}`,
			);
			expect(result).toEqual(mockIntegration);
		});

		it("should throw for non-existent integration", async () => {
			const error = { response: { status: 404 } };
			vi.mocked(api.get).mockRejectedValueOnce(error);

			await expect(
				systemIntegrationsApi.getById("non-existent-id"),
			).rejects.toEqual(error);
		});
	});

	describe("create", () => {
		it("should create a new integration", async () => {
			const createRequest = {
				name: "mangaupdates",
				displayName: "MangaUpdates",
				integrationType: "metadata_provider",
				credentials: { api_key: "test_key" },
				config: { rate_limit: 60 },
				enabled: false,
			};

			vi.mocked(api.post).mockResolvedValueOnce({
				data: { ...mockIntegration, enabled: false },
			});

			const result = await systemIntegrationsApi.create(createRequest);

			expect(api.post).toHaveBeenCalledWith(
				"/admin/integrations",
				createRequest,
			);
			expect(result.name).toBe("mangaupdates");
		});

		it("should create integration without credentials", async () => {
			const createRequest = {
				name: "test_provider",
				displayName: "Test Provider",
				integrationType: "notification",
			};

			vi.mocked(api.post).mockResolvedValueOnce({
				data: {
					...mockIntegration,
					name: "test_provider",
					hasCredentials: false,
				},
			});

			const result = await systemIntegrationsApi.create(createRequest);

			expect(api.post).toHaveBeenCalledWith(
				"/admin/integrations",
				createRequest,
			);
			expect(result.hasCredentials).toBe(false);
		});
	});

	describe("update", () => {
		it("should update an integration display name", async () => {
			const updateRequest = {
				displayName: "MangaUpdates API v2",
			};

			vi.mocked(api.patch).mockResolvedValueOnce({
				data: { ...mockIntegration, displayName: "MangaUpdates API v2" },
			});

			const result = await systemIntegrationsApi.update(
				mockIntegration.id,
				updateRequest,
			);

			expect(api.patch).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}`,
				updateRequest,
			);
			expect(result.displayName).toBe("MangaUpdates API v2");
		});

		it("should update an integration config", async () => {
			const updateRequest = {
				config: { rate_limit: 120, timeout: 60 },
			};

			vi.mocked(api.patch).mockResolvedValueOnce({
				data: { ...mockIntegration, config: updateRequest.config },
			});

			const result = await systemIntegrationsApi.update(
				mockIntegration.id,
				updateRequest,
			);

			expect(result.config).toEqual(updateRequest.config);
		});
	});

	describe("delete", () => {
		it("should delete an integration", async () => {
			vi.mocked(api.delete).mockResolvedValueOnce({ data: {} });

			await systemIntegrationsApi.delete(mockIntegration.id);

			expect(api.delete).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}`,
			);
		});
	});

	describe("enable", () => {
		it("should enable an integration", async () => {
			const mockResponse = {
				integration: { ...mockIntegration, enabled: true },
				message: "Integration enabled successfully",
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.enable(mockIntegration.id);

			expect(api.post).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}/enable`,
			);
			expect(result.integration.enabled).toBe(true);
			expect(result.message).toContain("enabled");
		});
	});

	describe("disable", () => {
		it("should disable an integration", async () => {
			const mockResponse = {
				integration: { ...mockIntegration, enabled: false },
				message: "Integration disabled successfully",
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.disable(mockIntegration.id);

			expect(api.post).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}/disable`,
			);
			expect(result.integration.enabled).toBe(false);
			expect(result.message).toContain("disabled");
		});
	});

	describe("test", () => {
		it("should test an integration connection", async () => {
			const mockResponse = {
				success: true,
				message: "Successfully connected to MangaUpdates API",
				latencyMs: 150,
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.test(mockIntegration.id);

			expect(api.post).toHaveBeenCalledWith(
				`/admin/integrations/${mockIntegration.id}/test`,
			);
			expect(result.success).toBe(true);
			expect(result.latencyMs).toBe(150);
		});

		it("should handle test failure", async () => {
			const mockResponse = {
				success: false,
				message: "Connection failed: timeout",
				latencyMs: null,
			};

			vi.mocked(api.post).mockResolvedValueOnce({ data: mockResponse });

			const result = await systemIntegrationsApi.test(mockIntegration.id);

			expect(result.success).toBe(false);
			expect(result.message).toContain("failed");
		});
	});
});
