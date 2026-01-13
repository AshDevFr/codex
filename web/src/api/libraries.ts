import type {
	CreateLibraryRequest,
	Library,
	PreviewScanRequest,
	PreviewScanResponse,
	ScanningConfig,
} from "@/types";
import { api } from "./client";

export const librariesApi = {
	// Get all libraries
	getAll: async (): Promise<Library[]> => {
		const response = await api.get<Library[]>("/libraries");
		return response.data;
	},

	// Get a single library by ID
	getById: async (id: string): Promise<Library> => {
		const response = await api.get<Library>(`/libraries/${id}`);
		return response.data;
	},

	// Create a new library
	create: async (library: CreateLibraryRequest): Promise<Library> => {
		const response = await api.post<Library>("/libraries", library);
		return response.data;
	},

	// Update a library
	update: async (
		id: string,
		library:
			| Partial<Library>
			| {
					name?: string;
					scanningConfig?: ScanningConfig;
			  },
	): Promise<Library> => {
		const response = await api.patch<Library>(`/libraries/${id}`, library);
		return response.data;
	},

	// Delete a library
	delete: async (id: string): Promise<void> => {
		await api.delete(`/libraries/${id}`);
	},

	// Trigger a scan
	scan: async (
		id: string,
		mode: "normal" | "deep" = "normal",
	): Promise<void> => {
		await api.post(`/libraries/${id}/scan?mode=${mode}`);
	},

	// Purge deleted books from a library
	purgeDeleted: async (id: string): Promise<number> => {
		const response = await api.delete<number>(`/libraries/${id}/purge-deleted`);
		return response.data;
	},

	// Preview scan to detect series before creating library
	previewScan: async (request: PreviewScanRequest): Promise<PreviewScanResponse> => {
		const response = await api.post<PreviewScanResponse>(
			"/libraries/preview-scan",
			request,
		);
		return response.data;
	},
};
