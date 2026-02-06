import type {
  LoginRequest,
  LoginResponse,
  OidcLoginResponse,
  OidcProvidersResponse,
  RegisterRequest,
  RegisterResponse,
  User,
} from "@/types";
import { api } from "./client";

export const authApi = {
  // Login
  login: async (credentials: LoginRequest): Promise<LoginResponse> => {
    const response = await api.post<LoginResponse>("/auth/login", credentials);
    return response.data;
  },

  // Register
  register: async (data: RegisterRequest): Promise<RegisterResponse> => {
    const response = await api.post<RegisterResponse>("/auth/register", data);
    return response.data;
  },

  // Get current user
  getCurrentUser: async (): Promise<User> => {
    const response = await api.get<User>("/auth/me");
    return response.data;
  },

  // Logout (just clear local storage, no API call needed)
  logout: () => {
    localStorage.removeItem("jwt_token");
    localStorage.removeItem("user");
  },

  // Get available OIDC providers
  getOidcProviders: async (): Promise<OidcProvidersResponse> => {
    const response = await api.get<OidcProvidersResponse>(
      "/auth/oidc/providers",
    );
    return response.data;
  },

  // Initiate OIDC login flow (returns redirect URL)
  initiateOidcLogin: async (provider: string): Promise<OidcLoginResponse> => {
    const response = await api.post<OidcLoginResponse>(
      `/auth/oidc/${provider}/login`,
    );
    return response.data;
  },
};
