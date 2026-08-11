import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/mix-response.json" with { type: "json" };
import {
  buildReason,
  mapFormat,
  mapRecommendation,
  mapStatus,
  mapTags,
  pickTitle,
} from "./mappers.js";
import { LibraryIndex } from "./seeds.js";
import type { MbRecommendationEntry, MbSeries } from "./types.js";

const entries = fixture.data as unknown as MbRecommendationEntry[];

/** Minimal mapping context: two seeds, nothing already in the library. */
function ctx(overrides: Partial<Parameters<typeof mapRecommendation>[1]> = {}) {
  return {
    seedTitles: new Map<number, string>([
      [3397, "Solo Leveling"],
      [84926, "Re:Zero"],
    ]),
    library: new LibraryIndex(),
    ...overrides,
  };
}

/** Build a series with only the fields under test. */
function series(overrides: Partial<MbSeries> = {}): MbSeries {
  return { id: 1, title: "Test Series", ...overrides };
}

describe("mapStatus", () => {
  it.each([
    ["releasing", "ongoing"],
    ["completed", "ended"],
    ["hiatus", "hiatus"],
    ["cancelled", "abandoned"],
    ["upcoming", "unknown"],
    ["unknown", "unknown"],
  ] as const)("maps %s to %s", (upstream, expected) => {
    expect(mapStatus(upstream)).toBe(expected);
  });

  it("returns undefined for a missing or unrecognised status", () => {
    expect(mapStatus(null)).toBeUndefined();
    expect(mapStatus(undefined)).toBeUndefined();
    expect(mapStatus("something-new" as never)).toBeUndefined();
  });
});

describe("mapFormat", () => {
  it("uppercases the series type to match the UI's expectations", () => {
    // RecommendationCard hides the badge for "MANGA" and translates "NOVEL",
    // both compared as uppercase.
    expect(mapFormat("manga")).toBe("MANGA");
    expect(mapFormat("novel")).toBe("NOVEL");
    expect(mapFormat("manhwa")).toBe("MANHWA");
  });

  it("returns undefined when the type is absent", () => {
    expect(mapFormat(null)).toBeUndefined();
  });
});

describe("mapTags", () => {
  it("converts the ordinal weight into a 0-100 rank", () => {
    const tags = mapTags([
      { id: 1, name: "Core", weight: "core" },
      { id: 2, name: "Defining", weight: "defining" },
      { id: 3, name: "Recurrent", weight: "recurrent" },
      { id: 4, name: "Incidental", weight: "incidental" },
      { id: 5, name: "Unweighted", weight: "unweighted" },
    ]);

    expect(tags?.map((t) => t.rank)).toEqual([100, 85, 60, 35, 10]);
  });

  it("treats an absent weight as the weakest signal, distinct from unweighted", () => {
    const tags = mapTags([{ id: 1, name: "Nulled", weight: null }]);

    expect(tags?.[0].rank).toBe(0);
  });

  it("uses the root of name_path as the category", () => {
    const tags = mapTags([
      {
        id: 38,
        name: "Shounen",
        name_path: "Audience Demographics > Male Oriented > Shounen",
        weight: "defining",
      },
    ]);

    expect(tags?.[0].category).toBe("Audience Demographics");
  });

  it("falls back to a generic category when name_path is missing", () => {
    const tags = mapTags([{ id: 1, name: "Loose", weight: "core" }]);

    expect(tags?.[0].category).toBe("Tag");
  });

  it("drops spoiler tags", () => {
    // These would spoil the series in a recommendation card.
    const tags = mapTags([
      { id: 1, name: "Safe", weight: "core" },
      { id: 2, name: "Big Twist", weight: "core", is_spoiler: true },
    ]);

    expect(tags?.map((t) => t.name)).toEqual(["Safe"]);
  });

  it("returns undefined rather than an empty array when there are no tags", () => {
    expect(mapTags(null)).toBeUndefined();
    expect(mapTags([])).toBeUndefined();
  });
});

describe("pickTitle", () => {
  it("uses the primary title when it is already Latin script", () => {
    expect(
      pickTitle(series({ title: "Solo Leveling", romanized_title: "Na Honjaman Lebel-eob" })),
    ).toBe("Solo Leveling");
  });

  it("prefers the romanization when the primary title is not Latin script", () => {
    expect(
      pickTitle(series({ title: "나 혼자만 레벨업", romanized_title: "Na Honjaman Lebel-eob" })),
    ).toBe("Na Honjaman Lebel-eob");
  });

  it("keeps a non-Latin title when there is no romanization", () => {
    expect(pickTitle(series({ title: "나 혼자만 레벨업" }))).toBe("나 혼자만 레벨업");
  });

  it("falls back through romanized and native before giving up", () => {
    expect(pickTitle(series({ title: null, romanized_title: "Romanized" }))).toBe("Romanized");
    expect(pickTitle(series({ title: null, native_title: "Native" }))).toBe("Native");
    expect(pickTitle(series({ id: 42, title: null }))).toBe("Series 42");
  });
});

describe("buildReason", () => {
  it("names the shared tags and the seeds they came from", () => {
    const reason = buildReason([{ id: 1, name: "Isekai", weight: "core" }], ["Solo Leveling"]);

    expect(reason).toContain("Isekai");
    expect(reason).toContain("Solo Leveling");
  });

  it("falls back to a plain similarity statement with no shared tags", () => {
    expect(buildReason(null, ["Solo Leveling"])).toBe("Similar to Solo Leveling");
  });

  it("summarises rather than listing every seed", () => {
    // Asserted as an exact string: a substring check passed happily on the
    // earlier "A and B and 2 more", which had a doubled conjunction.
    expect(buildReason(null, ["A", "B", "C", "D"])).toBe("Similar to A, B and 2 more");
  });

  it("uses a single conjunction when listing exactly two seeds", () => {
    expect(buildReason(null, ["A", "B"])).toBe("Similar to A and B");
  });

  it("reads correctly when both tags and truncated seeds are present", () => {
    const reason = buildReason(
      [{ id: 1, name: "Isekai", weight: "core" }],
      ["Berserk", "Solo Leveling", "Re:Zero"],
    );

    expect(reason).toBe("Shares Isekai with Berserk, Solo Leveling and 1 more");
  });

  it("stays generic when there is nothing to attribute", () => {
    expect(buildReason(null, [])).toBe("Matches your library's taste profile");
  });
});

describe("mapRecommendation", () => {
  it("maps a real upstream entry end to end", () => {
    const rec = mapRecommendation(entries[0], ctx());

    expect(rec).not.toBeNull();
    expect(rec?.externalId).toBe("20299");
    expect(rec?.externalUrl).toBe("https://mangabaka.org/20299");
    expect(rec?.title).toBeTruthy();
    expect(rec?.score).toBeGreaterThan(0);
    expect(rec?.score).toBeLessThanOrEqual(1);
  });

  it("maps every entry in the fixture without throwing", () => {
    const mapped = entries.map((e) => mapRecommendation(e, ctx()));

    expect(mapped.filter(Boolean)).toHaveLength(entries.length);
  });

  it("resolves matched_seed_ids into seed titles for basedOn", () => {
    const rec = mapRecommendation(entries[0], ctx());

    expect(rec?.basedOn).toEqual(expect.arrayContaining(["Solo Leveling", "Re:Zero"]));
  });

  it("omits seed IDs it cannot name rather than emitting a raw number", () => {
    const rec = mapRecommendation(
      { score: 0.5, matched_seed_ids: [3397, 999999], series: series() },
      ctx(),
    );

    expect(rec?.basedOn).toEqual(["Solo Leveling"]);
  });

  it("parses total_chapters, which upstream sends as a string", () => {
    const rec = mapRecommendation({ series: series({ total_chapters: "50" }) }, ctx());

    expect(rec?.totalChapterCount).toBe(50);
  });

  it("keeps a fractional chapter count", () => {
    const rec = mapRecommendation({ series: series({ total_chapters: "50.5" }) }, ctx());

    expect(rec?.totalChapterCount).toBe(50.5);
  });

  it("rounds the fractional 0-100 rating to an integer", () => {
    const rec = mapRecommendation({ series: series({ rating: 67.16 }) }, ctx());

    expect(rec?.rating).toBe(67);
  });

  it("derives country of origin from the series type", () => {
    expect(mapRecommendation({ series: series({ type: "manga" }) }, ctx())?.countryOfOrigin).toBe(
      "JP",
    );
    expect(mapRecommendation({ series: series({ type: "manhwa" }) }, ctx())?.countryOfOrigin).toBe(
      "KR",
    );
    expect(mapRecommendation({ series: series({ type: "manhua" }) }, ctx())?.countryOfOrigin).toBe(
      "CN",
    );
  });

  it("leaves country of origin unset for types that do not imply one", () => {
    // A novel or OEL title carries no reliable country signal.
    expect(
      mapRecommendation({ series: series({ type: "novel" }) }, ctx())?.countryOfOrigin,
    ).toBeUndefined();
    expect(
      mapRecommendation({ series: series({ type: "oel" }) }, ctx())?.countryOfOrigin,
    ).toBeUndefined();
  });

  it("title-cases the lowercase upstream genres", () => {
    const rec = mapRecommendation(
      { series: series({ genres: ["action", "slice_of_life", "sci-fi"] }) },
      ctx(),
    );

    expect(rec?.genres).toEqual(["Action", "Slice of Life", "Sci-Fi"]);
  });

  it("prefers a sized cover variant over the raw original", () => {
    const rec = mapRecommendation(
      {
        series: series({
          cover: { raw: { url: "https://img/raw.jpg" }, x350: { x1: "https://cdn/x350.jpg" } },
        }),
      },
      ctx(),
    );

    expect(rec?.coverUrl).toBe("https://cdn/x350.jpg");
  });

  it("falls back to the raw cover when no sized variant exists", () => {
    const rec = mapRecommendation(
      { series: series({ cover: { raw: { url: "https://img/raw.jpg" } } }) },
      ctx(),
    );

    expect(rec?.coverUrl).toBe("https://img/raw.jpg");
  });

  it("flags results already present in the user's library", () => {
    const library = new LibraryIndex();
    library.addMangaBakaId(500);

    const rec = mapRecommendation({ series: series({ id: 500 }) }, ctx({ library }));

    expect(rec?.inLibrary).toBe(true);
  });

  it("clamps an out-of-range score into 0-1", () => {
    expect(mapRecommendation({ score: 1.8, series: series() }, ctx())?.score).toBe(1);
    expect(mapRecommendation({ score: -0.5, series: series() }, ctx())?.score).toBe(0);
  });

  it("rounds away upstream's noise-level score precision", () => {
    // Upstream returns e.g. 0.69555523762905; nothing downstream can act on
    // differences that small.
    expect(mapRecommendation({ score: 0.69555523762905, series: series() }, ctx())?.score).toBe(
      0.7,
    );
    expect(mapRecommendation({ score: 0.344, series: series() }, ctx())?.score).toBe(0.34);
  });

  it("skips merged and deleted series", () => {
    // Upstream keeps tombstones addressable; recommending one sends the user
    // to a dead page.
    expect(
      mapRecommendation({ series: series({ state: "merged", merged_with: 9 }) }, ctx()),
    ).toBeNull();
    expect(mapRecommendation({ series: series({ state: "deleted" }) }, ctx())).toBeNull();
  });

  it("survives a series stripped of every optional field", () => {
    const rec = mapRecommendation({ series: { id: 7 } }, ctx());

    expect(rec).not.toBeNull();
    expect(rec?.externalId).toBe("7");
    expect(rec?.genres).toEqual([]);
    expect(rec?.score).toBe(0);
  });

  it("ignores a non-numeric chapter count instead of emitting NaN", () => {
    const rec = mapRecommendation({ series: series({ total_chapters: "unknown" }) }, ctx());

    expect(rec?.totalChapterCount).toBeUndefined();
  });

  it("ignores a zero volume count, which upstream uses to mean unknown", () => {
    const rec = mapRecommendation({ series: series({ final_volume: 0 }) }, ctx());

    expect(rec?.totalVolumeCount).toBeUndefined();
  });
});

describe("host schema conformance", () => {
  /**
   * The host deserialises into a strongly-typed Rust struct and rejects the
   * whole batch on a single type mismatch, so a nested object where an integer
   * is expected fails every recommendation, not just one field.
   *
   * This is exactly how `popularity` slipped through to a real task failure:
   * MangaBaka returns a nested rank object, the local type wrongly declared it
   * a number, and no assertion checked the mapped value's runtime type. The
   * fixture contained the object all along.
   */
  const NUMERIC_FIELDS = [
    "score",
    "startYear",
    "totalVolumeCount",
    "totalChapterCount",
    "rating",
    "popularity",
  ] as const;

  const STRING_FIELDS = [
    "externalId",
    "externalUrl",
    "title",
    "coverUrl",
    "summary",
    "reason",
    "status",
    "format",
    "countryOfOrigin",
  ] as const;

  it.each(entries.map((entry, index) => [index, entry] as const))(
    "emits only host-compatible primitives for fixture entry %i",
    (_index, entry) => {
      const rec = mapRecommendation(entry, ctx());
      expect(rec).not.toBeNull();
      if (!rec) return;

      for (const field of NUMERIC_FIELDS) {
        const value = rec[field];
        if (value !== undefined) {
          expect(typeof value, `${field} must be a number`).toBe("number");
          expect(Number.isFinite(value as number)).toBe(true);
        }
      }

      for (const field of STRING_FIELDS) {
        const value = rec[field];
        if (value !== undefined) {
          expect(typeof value, `${field} must be a string`).toBe("string");
        }
      }

      expect(Array.isArray(rec.genres)).toBe(true);
      for (const genre of rec.genres) expect(typeof genre).toBe("string");

      expect(Array.isArray(rec.basedOn)).toBe(true);
      for (const title of rec.basedOn) expect(typeof title).toBe("string");

      expect(typeof rec.inLibrary).toBe("boolean");

      for (const tag of rec.tags ?? []) {
        expect(typeof tag.name).toBe("string");
        expect(typeof tag.rank).toBe("number");
        expect(typeof tag.category).toBe("string");
      }
    },
  );

  it("does not forward the popularity rank, whose meaning is inverted here", () => {
    // MangaBaka ranks (lower is more popular); the field is rendered as a count
    // (higher is more popular). Passing it would be wrong even once the type is.
    const rec = mapRecommendation(entries[0], ctx());

    expect(rec?.popularity).toBeUndefined();
  });
});
