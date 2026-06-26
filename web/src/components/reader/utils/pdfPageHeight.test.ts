import { describe, expect, it } from "vitest";
import { pdfPageReservedHeight } from "./pdfPageHeight";

describe("pdfPageReservedHeight", () => {
  // US Letter-ish portrait page in points.
  const portrait = { width: 612, height: 792 };

  it("returns null for invalid dimensions", () => {
    expect(
      pdfPageReservedHeight({
        zoomLevel: "100%",
        availableWidth: 800,
        orig: { width: 0, height: 792 },
      }),
    ).toBeNull();
    expect(
      pdfPageReservedHeight({
        zoomLevel: "100%",
        availableWidth: 800,
        orig: { width: 612, height: 0 },
      }),
    ).toBeNull();
  });

  describe("fixed-zoom levels", () => {
    it("renders at the intrinsic point height for 100%", () => {
      expect(
        pdfPageReservedHeight({
          zoomLevel: "100%",
          availableWidth: 800,
          orig: portrait,
        }),
      ).toBe(792);
    });

    it("scales the height for 50% and 200%", () => {
      expect(
        pdfPageReservedHeight({
          zoomLevel: "50%",
          availableWidth: 800,
          orig: portrait,
        }),
      ).toBe(396);
      expect(
        pdfPageReservedHeight({
          zoomLevel: "200%",
          availableWidth: 800,
          orig: portrait,
        }),
      ).toBe(1584);
    });

    it("ignores the available width for fixed zoom", () => {
      expect(
        pdfPageReservedHeight({
          zoomLevel: "125%",
          availableWidth: 0,
          orig: portrait,
        }),
      ).toBe(990);
    });
  });

  describe("fit modes", () => {
    it("derives height from the available width and aspect ratio", () => {
      // 600px wide → 600 * (792/612) = 776.47...
      expect(
        pdfPageReservedHeight({
          zoomLevel: "fit-width",
          availableWidth: 600,
          orig: portrait,
        }),
      ).toBeCloseTo(776.47, 1);
      expect(
        pdfPageReservedHeight({
          zoomLevel: "fit-page",
          availableWidth: 612,
          orig: portrait,
        }),
      ).toBe(792);
    });

    it("returns null when the container width isn't known yet", () => {
      expect(
        pdfPageReservedHeight({
          zoomLevel: "fit-width",
          availableWidth: 0,
          orig: portrait,
        }),
      ).toBeNull();
    });
  });
});
