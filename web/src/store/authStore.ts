import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { User } from "@/types";

interface AuthState {
  user: User | null;
  token: string | null;
  refreshToken: string | null;
  isAuthenticated: boolean;

  // Actions
  setAuth: (user: User, token: string, refreshToken?: string | null) => void;
  updateTokens: (token: string, refreshToken: string) => void;
  clearAuth: () => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      token: null,
      refreshToken: null,
      isAuthenticated: false,

      setAuth: (user, token, refreshToken) => {
        localStorage.setItem("jwt_token", token);
        if (refreshToken) {
          localStorage.setItem("jwt_refresh_token", refreshToken);
        } else {
          localStorage.removeItem("jwt_refresh_token");
        }
        set({
          user,
          token,
          refreshToken: refreshToken ?? null,
          isAuthenticated: true,
        });
      },

      updateTokens: (token, refreshToken) => {
        localStorage.setItem("jwt_token", token);
        localStorage.setItem("jwt_refresh_token", refreshToken);
        set({ token, refreshToken });
      },

      clearAuth: () => {
        localStorage.removeItem("jwt_token");
        localStorage.removeItem("jwt_refresh_token");
        set({
          user: null,
          token: null,
          refreshToken: null,
          isAuthenticated: false,
        });
      },
    }),
    {
      name: "auth-storage",
      partialize: (state) => ({
        user: state.user,
        token: state.token,
        refreshToken: state.refreshToken,
        isAuthenticated: state.isAuthenticated,
      }),
    },
  ),
);
