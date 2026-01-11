import axios, { type AxiosInstance } from "axios";
import { navigationService } from "@/services/navigation";
import { useAuthStore } from "@/store/authStore";
import type { ApiError } from "@/types";

// Create axios instance with base configuration
export const api: AxiosInstance = axios.create({
	baseURL: "/api/v1",
	timeout: 30000,
	headers: {
		"Content-Type": "application/json",
	},
	// IMPORTANT: Send cookies with requests (required for cookie-based image auth)
	withCredentials: true,
});

// Request interceptor to add auth token
api.interceptors.request.use(
	(config) => {
		const token = localStorage.getItem("jwt_token");
		if (token) {
			config.headers.Authorization = `Bearer ${token}`;
		}
		return config;
	},
	(error) => {
		return Promise.reject(error);
	},
);

// Response interceptor to handle errors
api.interceptors.response.use(
	(response) => response,
	(error) => {
		if (error.response) {
			const apiError: ApiError = {
				error: error.response.data?.error || "An error occurred",
				message: error.response.data?.message || error.message,
			};

			// Handle 401 Unauthorized - clear auth state and redirect to login
			if (error.response.status === 401) {
				const { clearAuth } = useAuthStore.getState();
				clearAuth();
				navigationService.navigateTo("/login");
			}

			return Promise.reject(apiError);
		}

		return Promise.reject({
			error: "Network Error",
			message: error.message,
		} as ApiError);
	},
);
