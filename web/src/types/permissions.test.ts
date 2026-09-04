import { describe, expect, it } from "vitest";

import openapi from "../../openapi.json";
import {
  ALL_PERMISSIONS,
  getPermissionLabel,
  PERMISSION_GROUPS,
  ROLE_PERMISSIONS,
} from "./permissions";

/**
 * The backend exports its Permission enum in the OpenAPI spec precisely so
 * this hand-maintained catalog can be checked against it. If this test
 * fails, a permission was added or renamed on the backend without updating
 * `permissions.ts`, which silently drops it from the key-creation dialog
 * and the user permission editor.
 */
const backendPermissions = (
  openapi as {
    components: { schemas: { Permission: { enum: string[] } } };
  }
).components.schemas.Permission.enum;

describe("permission catalog parity", () => {
  it("matches the backend Permission enum exactly", () => {
    expect([...ALL_PERMISSIONS].sort()).toEqual([...backendPermissions].sort());
  });

  it("lists every permission in exactly one UI group", () => {
    const grouped = PERMISSION_GROUPS.flatMap((group) =>
      group.permissions.map((p) => p.value),
    );
    expect([...grouped].sort()).toEqual([...backendPermissions].sort());
    expect(new Set(grouped).size).toBe(grouped.length);
  });

  it("gives the admin role every permission", () => {
    expect([...ROLE_PERMISSIONS.admin].sort()).toEqual(
      [...backendPermissions].sort(),
    );
  });

  it("has a human-readable label for every permission", () => {
    for (const permission of backendPermissions) {
      const label = getPermissionLabel(permission as never);
      expect(label, `label for ${permission}`).not.toBe(permission);
    }
  });
});
