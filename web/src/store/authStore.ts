import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { User } from "@/types/api";

interface AuthState {
	user: User | null;
	token: string | null;
	isAuthenticated: boolean;

	// Actions
	setAuth: (user: User, token: string) => void;
	clearAuth: () => void;
}

export const useAuthStore = create<AuthState>()(
	persist(
		(set) => ({
			user: null,
			token: null,
			isAuthenticated: false,

			setAuth: (user, token) => {
				localStorage.setItem("jwt_token", token);
				set({ user, token, isAuthenticated: true });
			},

			clearAuth: () => {
				localStorage.removeItem("jwt_token");
				set({ user: null, token: null, isAuthenticated: false });
			},
		}),
		{
			name: "auth-storage",
			partialize: (state) => ({
				user: state.user,
				token: state.token,
				isAuthenticated: state.isAuthenticated,
			}),
		},
	),
);
