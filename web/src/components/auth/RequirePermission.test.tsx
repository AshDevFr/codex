import { Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import { useAuthStore } from "@/store/authStore";
import { renderWithProviders, screen } from "@/test/utils";
import { PERMISSIONS } from "@/types/permissions";
import { RequirePermission } from "./RequirePermission";

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
          <RequirePermission permission={PERMISSIONS.TASKS_READ}>
            <div>Metrics page</div>
          </RequirePermission>
        }
      />
      <Route path="/" element={<div>Home page</div>} />
    </Routes>,
    { initialEntries: [path] },
  );
}

describe("RequirePermission", () => {
  beforeEach(() => {
    useAuthStore.setState({
      user: null,
      token: null,
      isAuthenticated: false,
    } as never);
  });

  it.each([
    "admin",
    "maintainer",
  ] as const)("shows the page to a %s", (role) => {
    signIn(role);
    renderAt("/settings/metrics");
    expect(screen.getByText("Metrics page")).toBeInTheDocument();
  });

  it("sends a reader home", () => {
    signIn("reader");
    renderAt("/settings/metrics");
    expect(screen.queryByText("Metrics page")).not.toBeInTheDocument();
    expect(screen.getByText("Home page")).toBeInTheDocument();
  });
});
