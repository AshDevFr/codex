import { describe, expect, it } from "vitest";
import { noContentAsNull } from "./noContent";

describe("noContentAsNull", () => {
  it("returns the payload for a 200 with a body", () => {
    const payload = { id: "progress-123" };

    expect(noContentAsNull({ status: 200, data: payload })).toEqual(payload);
  });

  it("returns null for a 204", () => {
    // Axios surfaces a body-less response as the empty string, not null.
    expect(noContentAsNull({ status: 204, data: "" })).toBeNull();
  });

  it("returns null for a legacy 200 with a null body", () => {
    // Older servers answer the absent case with `200 null`; a client that has
    // been updated ahead of its server must keep working against both.
    expect(noContentAsNull({ status: 200, data: null })).toBeNull();
  });
});
