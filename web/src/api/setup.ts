import type {
	SetupStatusResponse,
	InitializeSetupRequest,
	InitializeSetupResponse,
	ConfigureSettingsRequest,
	ConfigureSettingsResponse,
} from "@/types/api";
import { api } from "./client";

export const setupApi = {
	// Check if setup is required
	checkStatus: async (): Promise<SetupStatusResponse> => {
		const response = await api.get<SetupStatusResponse>("/setup/status");
		return response.data;
	},

	// Initialize setup by creating first admin user
	initialize: async (
		data: InitializeSetupRequest,
	): Promise<InitializeSetupResponse> => {
		const response = await api.post<InitializeSetupResponse>(
			"/setup/initialize",
			data,
		);
		return response.data;
	},

	// Configure initial settings (optional step)
	configureSettings: async (
		data: ConfigureSettingsRequest,
	): Promise<ConfigureSettingsResponse> => {
		const response = await api.patch<ConfigureSettingsResponse>(
			"/setup/settings",
			data,
		);
		return response.data;
	},
};
