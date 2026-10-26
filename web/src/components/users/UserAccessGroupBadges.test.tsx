import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { UserAccessGroupBadges } from "./UserAccessGroupBadges";

describe("UserAccessGroupBadges", () => {
  it("renders a badge per group", () => {
    renderWithProviders(
      <UserAccessGroupBadges
        groups={[
          { id: "1", name: "Kids", source: "manual" },
          { id: "2", name: "Manga", source: "oidc" },
        ]}
      />,
    );

    expect(screen.getByText("Kids")).toBeInTheDocument();
    expect(screen.getByText("Manga")).toBeInTheDocument();
  });

  it("labels memberships that come from the identity provider", () => {
    renderWithProviders(
      <UserAccessGroupBadges
        groups={[
          { id: "1", name: "Kids", source: "manual" },
          { id: "2", name: "Manga", source: "oidc" },
        ]}
      />,
    );

    expect(screen.getByLabelText("Manga (from OIDC)")).toBeInTheDocument();
    expect(screen.queryByLabelText("Kids (from OIDC)")).not.toBeInTheDocument();
  });

  it("shows a placeholder when the user has no groups", () => {
    renderWithProviders(<UserAccessGroupBadges groups={[]} />);

    expect(screen.getByText("None")).toBeInTheDocument();
  });

  it("says the groups do not restrict an admin or maintainer", () => {
    renderWithProviders(
      <UserAccessGroupBadges
        unrestricted
        groups={[{ id: "1", name: "Kids", source: "default" }]}
      />,
    );

    expect(
      screen.getByText(/not restricted/i, { exact: false }),
    ).toBeInTheDocument();
  });
});
