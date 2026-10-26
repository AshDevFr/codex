import { Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, screen } from "@/test/utils";
import { AdminRoute } from "./AdminRoute";

function signIn(role: "admin" | "maintainer" | "reader") {
  useAuthStore.setState({
    user: {
      id: "u",
      username: "u",
      email: "u@example.com",
      role,
      emailVerified: true,
      permissions: [],
    },
    token: "token",
    isAuthenticated: true,
  } as never);
}

function renderAt(path: string) {
  renderWithProviders(
    <Routes>
      <Route
        path="/settings/metrics"
        element={
          <AdminRoute>
            <div>Metrics page</div>
          </AdminRoute>
        }
      />
      <Route path="/" element={<div>Home page</div>} />
    </Routes>,
    { initialEntries: [path] },
  );
}

describe("AdminRoute", () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: null,
      token: null,
      isAuthenticated: false,
    } as never);
  });

  it("shows the page to an admin", () => {
    signIn("admin");
    renderAt("/settings/metrics");
    expect(screen.getByText("Metrics page")).toBeInTheDocument();
  });

  it("sends anyone else home", () => {
    signIn("maintainer");
    renderAt("/settings/metrics");
    expect(screen.queryByText("Metrics page")).not.toBeInTheDocument();
    expect(screen.getByText("Home page")).toBeInTheDocument();
  });
});
