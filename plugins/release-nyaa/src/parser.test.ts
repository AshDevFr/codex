import { describe, expect, it } from "vitest";
import { parseFeed, parseItem, parseTitle } from "./parser.js";

// -----------------------------------------------------------------------------
// parseTitle — corpus mirroring real-world Nyaa titles, including the user's
// 1r0n / mixed-format examples that motivated this phase.
// -----------------------------------------------------------------------------

describe("parseTitle", () => {
  it("parses a 1r0n volume release with leading group token and trailing tags", () => {
    const t = parseTitle("[1r0n] Boruto - Two Blue Vortex - Volume 02 (Digital) (1r0n)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.group).toBe("1r0n");
    expect(t.volume).toBe(2);
    expect(t.chapter).toBeNull();
    expect(t.formatHints.digital).toBe(true);
    // Series guess strips group, volume token, and parenthesized tags.
    expect(t.seriesGuess).toBe("Boruto Two Blue Vortex");
  });

  it("parses a v107 short-form volume release", () => {
    const t = parseTitle("[1r0n] One Piece v107 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(107);
    expect(t.chapter).toBeNull();
    expect(t.formatHints.digital).toBe(true);
    expect(t.seriesGuess).toBe("One Piece");
  });

  it("parses a single chapter release with `Chapter NNN` long form", () => {
    const t = parseTitle("[1r0n] Chainsaw Man - Chapter 142 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBe(142);
    expect(t.volume).toBeNull();
    expect(t.seriesGuess).toBe("Chainsaw Man");
  });

  it("parses a chapter range (the screenshot's loose-chapter shape)", () => {
    const t = parseTitle("[Group] Dandadan c126-142 (2024) (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBe(126);
    expect(t.chapterRangeEnd).toBe(142);
    expect(t.volume).toBeNull();
    expect(t.formatHints.digital).toBe(true);
    expect(t.seriesGuess).toBe("Dandadan");
  });

  it("parses a volume range (`v01-14` from the user's mixed-format screenshot)", () => {
    const t = parseTitle("[1r0n] Boruto v01-14 (Digital) (1r0n)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(14);
    expect(t.seriesGuess).toBe("Boruto");
  });

  it("parses a Tankobon-Blur Vol. NN release", () => {
    const t = parseTitle("[Tankobon Blur] Solo Leveling Vol. 13 (2024) (Digital) (Tankobon Blur)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.group).toBe("Tankobon Blur");
    expect(t.volume).toBe(13);
    expect(t.formatHints.digital).toBe(true);
    expect(t.seriesGuess).toBe("Solo Leveling");
  });

  it("parses a plain release without leading group token", () => {
    const t = parseTitle("Berserk Volume 42 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.group).toBeNull();
    expect(t.volume).toBe(42);
    expect(t.formatHints.digital).toBe(true);
    expect(t.seriesGuess).toBe("Berserk");
  });

  it("preserves decimal chapters", () => {
    const t = parseTitle("[Group] Some Series c47.5 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBe(47.5);
    expect(t.seriesGuess).toBe("Some Series");
  });

  it("captures JXL format hint", () => {
    const t = parseTitle("[1r0n] One Piece v107 (Digital) (JXL)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.formatHints.digital).toBe(true);
    expect(t.formatHints.jxl).toBe(true);
  });

  it("returns null for an empty title", () => {
    expect(parseTitle("")).toBeNull();
    expect(parseTitle("   ")).toBeNull();
  });

  it("falls back to the raw title (no axis info) when no chapter/volume tokens are present", () => {
    const t = parseTitle("Just Some Manga Tanks (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBeNull();
    expect(t.volume).toBeNull();
    expect(t.seriesGuess).toBe("Just Some Manga Tanks");
    expect(t.formatHints.digital).toBe(true);
  });

  it("handles the 'ch.' prefix variant alongside the c.NNN form", () => {
    const t = parseTitle("[Group] My Series ch.143 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBe(143);
    expect(t.seriesGuess).toBe("My Series");
  });

  it("ignores leading bracketed token when not followed by content", () => {
    const t = parseTitle("[Group]");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.group).toBe("Group");
    expect(t.seriesGuess).toBe("");
  });
});

// -----------------------------------------------------------------------------
// parseTitle — multi-uploader/aggregated bundles (1r0n, danke-Empire, LuCaZ).
//
// Real-world bundle titles mix volumes, bare-numeric chapter ranges, and
// numeric "extras" (chapters not yet collected into a tankobon). Patterns we
// see in the wild:
//
//   v01-09                    → volume range only
//   v01-111 + 1134-1176       → vol range + bare chapter range, "+" joined
//   v01-28,125-137            → vol range + bare chapter range, "," joined
//   v01-31, 276-293           → same, with whitespace after comma
//   v01,009-090               → single volume + bare chapter range
//   v01-16 + 70               → vol range + single bare chapter
//   001-069 as v01-16 + 70    → bare chapter range followed by vol info
//   031-037                   → bare chapter range as primary identifier
//
// Bare numeric ranges are zero-padded to 3 digits in the corpus, which we use
// to distinguish chapter tokens from incidental numbers in series names.
// Year ranges always live inside `(...)` so they stay clear of the chapter
// tokenizer.
// -----------------------------------------------------------------------------

describe("parseTitle — aggregated bundle releases", () => {
  it("After God v01-09 — volume range only", () => {
    const t = parseTitle("After God v01-09 (2024-2026) (Digital) (1r0n)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(9);
    expect(t.chapter).toBeNull();
    expect(t.chapterRangeEnd).toBeNull();
    expect(t.seriesGuess).toBe("After God");
    expect(t.formatHints.digital).toBe(true);
  });

  it("One Piece v001-111 + 1134-1176 — vol range + bare chapter range joined by '+'", () => {
    const t = parseTitle("One Piece v001-111 + 1134-1176 (2003-2026) (Digital) (1r0n)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(111);
    expect(t.chapter).toBe(1134);
    expect(t.chapterRangeEnd).toBe(1176);
    expect(t.seriesGuess).toBe("One Piece");
  });

  it("Tensei… v01-28,125-137 — alias-split series, comma-joined chapter range", () => {
    const t = parseTitle(
      "Tensei Shitara Slime Datta Ken / That Time I Got Reincarnated as a Slime v01-28,125-137 (2017-2025) (Digital) (danke-Empire + nao)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(28);
    expect(t.chapter).toBe(125);
    expect(t.chapterRangeEnd).toBe(137);
    // Primary guess is the first alias.
    expect(t.seriesGuess).toBe("Tensei Shitara Slime Datta Ken");
    // Both halves of `A / B` are exposed for matching.
    expect(t.seriesGuessAliases).toEqual([
      "Tensei Shitara Slime Datta Ken",
      "That Time I Got Reincarnated as a Slime",
    ]);
  });

  it("Chillin'… 001-069 as v01-16 + 70 — bare chapter range + 'as' + vol range + extra chapter", () => {
    const t = parseTitle(
      "Chillin' in My 30s after Getting Fired from the Demon King's Army 001-069 as v01-16 + 70 (Digital) (danke-Empire + Aquila) [Oak]",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(16);
    // Aggregated min/max across all chapter tokens in the release header.
    expect(t.chapter).toBe(1);
    expect(t.chapterRangeEnd).toBe(70);
    expect(t.seriesGuess).toBe("Chillin' in My 30s after Getting Fired from the Demon King's Army");
  });

  it("Never Say Ugly 031-037 — bare chapter range only, no volume token", () => {
    const t = parseTitle(
      "Never Say Ugly 031-037 (2024-2025) (Digital) (danke-Empire, Stick, Aquila)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBeNull();
    expect(t.chapter).toBe(31);
    expect(t.chapterRangeEnd).toBe(37);
    expect(t.seriesGuess).toBe("Never Say Ugly");
  });

  it("Edens Zero v01-31, 276-293 — comma+space separator", () => {
    const t = parseTitle(
      "Edens Zero v01-31, 276-293 (2018-2025) (Digital) (danke-Empire, DeadMan, SlikkyOak)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(31);
    expect(t.chapter).toBe(276);
    expect(t.chapterRangeEnd).toBe(293);
    expect(t.seriesGuess).toBe("Edens Zero");
  });

  it("Ultimate Exorcist Kiyoshi v01,009-090 — single volume + bare chapter range", () => {
    const t = parseTitle("Ultimate Exorcist Kiyoshi v01,009-090 (2024-2026) (Digital) (LuCaZ)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBeNull();
    expect(t.chapter).toBe(9);
    expect(t.chapterRangeEnd).toBe(90);
    expect(t.seriesGuess).toBe("Ultimate Exorcist Kiyoshi");
  });

  it("Boruto - Two Blue Vortex v01-05,021-033 — subtitle dash + comma-joined ranges", () => {
    const t = parseTitle("Boruto - Two Blue Vortex v01-05,021-033 (2025-2026) (Digital) (LuCaZ)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(5);
    expect(t.chapter).toBe(21);
    expect(t.chapterRangeEnd).toBe(33);
    expect(t.seriesGuess).toBe("Boruto Two Blue Vortex");
  });

  it("Ao no Hako / Blue Box v01-20,181-240 — alias-split + comma chapters", () => {
    const t = parseTitle("Ao no Hako / Blue Box v01-20,181-240 (2022-2026) (Digital) (LuCaZ)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(20);
    expect(t.chapter).toBe(181);
    expect(t.chapterRangeEnd).toBe(240);
    expect(t.seriesGuess).toBe("Ao no Hako");
    expect(t.seriesGuessAliases).toEqual(["Ao no Hako", "Blue Box"]);
  });

  it("Ashita no Joe — Omnibus Edition is captured as a format hint", () => {
    const t = parseTitle(
      "Ashita no Joe - Fighting for Tomorrow v01-02 (2024-2025) (Omnibus Edition) (Digital) (LuCaZ)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(2);
    expect(t.formatHints.digital).toBe(true);
    expect(t.formatHints.omnibus).toBe(true);
    expect(t.seriesGuess).toBe("Ashita no Joe Fighting for Tomorrow");
  });

  it("Dragon Ball Super v01-23,101-104", () => {
    const t = parseTitle("Dragon Ball Super v01-23,101-104 (2017-2025) (Digital) (LuCaZ)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(23);
    expect(t.chapter).toBe(101);
    expect(t.chapterRangeEnd).toBe(104);
    expect(t.seriesGuess).toBe("Dragon Ball Super");
  });

  it("Becoming a Princess Knight... v01-04 — apostrophe-free long title with vol range only", () => {
    const t = parseTitle(
      "Becoming a Princess Knight and Working at a Yuri Brothel v01-04 (2024-2025) (Digital) (LuCaZ)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(4);
    expect(t.chapter).toBeNull();
    expect(t.seriesGuess).toBe("Becoming a Princess Knight and Working at a Yuri Brothel");
  });

  it("Amagami-san / Tying the Knot — alias-split + chapter range", () => {
    const t = parseTitle(
      "Amagami-san Chi no Enmusubi / Tying the Knot with an Amagami Sister v01-17,150-172 (2022-2025) (Digital) (LuCaZ)",
    );
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(17);
    expect(t.chapter).toBe(150);
    expect(t.chapterRangeEnd).toBe(172);
    expect(t.seriesGuess).toBe("Amagami-san Chi no Enmusubi");
    expect(t.seriesGuessAliases).toEqual([
      "Amagami-san Chi no Enmusubi",
      "Tying the Knot with an Amagami Sister",
    ]);
  });
});

// -----------------------------------------------------------------------------
// parseTitle — defensive: bare-number heuristics must not eat year ranges,
// and short bare numbers (1-2 digits) must not be promoted to chapters.
// -----------------------------------------------------------------------------

describe("parseTitle — bare-number safety net", () => {
  it("does not treat a year range inside (...) as a chapter range", () => {
    const t = parseTitle("Some Series v01-05 (2018-2025) (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBeNull();
    expect(t.chapterRangeEnd).toBeNull();
    expect(t.volume).toBe(1);
    expect(t.volumeRangeEnd).toBe(5);
  });

  it("ignores bare 1-2 digit numbers in the series name (avoids false positives)", () => {
    // "30s" appeared in the Chillin' title; standalone short numbers shouldn't
    // be picked up as chapters.
    const t = parseTitle("My 30s Adventure v01 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.chapter).toBeNull();
    expect(t.volume).toBe(1);
    expect(t.seriesGuess).toBe("My 30s Adventure");
  });

  it("does not split on '/' when there is no surrounding spacing (URL-like fragments)", () => {
    // Defensive: only ` / ` (spaced slash) is treated as an alias separator.
    const t = parseTitle("AC/DC Tales v01 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.seriesGuess).toBe("AC/DC Tales");
    expect(t.seriesGuessAliases).toEqual(["AC/DC Tales"]);
  });

  it("alias-split returns single-element array when no slash present", () => {
    const t = parseTitle("Berserk Volume 42 (Digital)");
    expect(t).not.toBeNull();
    if (t === null) return;
    expect(t.seriesGuessAliases).toEqual(["Berserk"]);
  });
});

// -----------------------------------------------------------------------------
// parseItem
// -----------------------------------------------------------------------------

const sampleItem = `
  <item>
    <title><![CDATA[[1r0n] Chainsaw Man - Chapter 142 (Digital)]]></title>
    <link>https://nyaa.si/download/12345.torrent</link>
    <guid isPermaLink="true">https://nyaa.si/view/12345</guid>
    <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
    <nyaa:infoHash>ABC123def456</nyaa:infoHash>
  </item>
`;

describe("parseItem", () => {
  it("extracts title, link, guid, infoHash, and pubDate", () => {
    const item = parseItem(sampleItem);
    expect(item).not.toBeNull();
    if (item === null) return;
    expect(item.title).toBe("[1r0n] Chainsaw Man - Chapter 142 (Digital)");
    expect(item.link).toBe("https://nyaa.si/download/12345.torrent");
    expect(item.pageUrl).toBe("https://nyaa.si/view/12345");
    expect(item.externalReleaseId).toBe("https://nyaa.si/view/12345"); // guid wins
    expect(item.infoHash).toBe("abc123def456"); // lowercased
    expect(item.chapter).toBe(142);
    expect(item.seriesGuess).toBe("Chainsaw Man");
    expect(new Date(item.observedAt).toISOString()).toBe("2026-05-04T02:31:00.000Z");
  });

  it("returns null when title is missing", () => {
    expect(parseItem("<item><link>x</link></item>")).toBeNull();
  });

  it("returns null pageUrl when guid is not a /view/ permalink", () => {
    const xml = `<item>
      <title><![CDATA[[1r0n] Foo c.1 (Digital)]]></title>
      <link>https://nyaa.si/download/9.torrent</link>
      <guid isPermaLink="false">tag:nyaa.si,2026:9</guid>
    </item>`;
    const item = parseItem(xml);
    expect(item).not.toBeNull();
    if (item === null) return;
    expect(item.pageUrl).toBeNull();
  });

  it("derives a deterministic externalReleaseId from infoHash when guid+link missing", () => {
    const xml = `<item>
      <title><![CDATA[[1r0n] Foo c.1 (Digital)]]></title>
      <nyaa:infoHash>DEADBEEF</nyaa:infoHash>
    </item>`;
    const item = parseItem(xml);
    expect(item).not.toBeNull();
    if (item === null) return;
    expect(item.externalReleaseId).toBe("urn:btih:deadbeef");
  });

  it("uses a hashed fallback when guid, link, and infoHash are all missing", () => {
    const xml = `<item>
      <title><![CDATA[Foo c.1 (Digital)]]></title>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
    </item>`;
    const item = parseItem(xml);
    expect(item).not.toBeNull();
    if (item === null) return;
    expect(item.externalReleaseId).toMatch(/^t:[a-z0-9]+$/);
  });
});

// -----------------------------------------------------------------------------
// parseFeed — full RSS body
// -----------------------------------------------------------------------------

const fullFeedXml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:nyaa="https://nyaa.si/xmlns/nyaa">
  <channel>
    <title>Nyaa - 1r0n's torrents</title>
    <item>
      <title><![CDATA[[1r0n] Boruto - Two Blue Vortex - Volume 02 (Digital) (1r0n)]]></title>
      <link>https://nyaa.si/download/1.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/1</guid>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
      <nyaa:infoHash>aaa</nyaa:infoHash>
    </item>
    <item>
      <title><![CDATA[[1r0n] Boruto v01-14 (Digital) (1r0n)]]></title>
      <link>https://nyaa.si/download/2.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/2</guid>
      <pubDate>Sun, 03 May 2026 12:00:00 GMT</pubDate>
      <nyaa:infoHash>bbb</nyaa:infoHash>
    </item>
    <item>
      <title><![CDATA[[1r0n] Dandadan c126-142 (2024) (Digital)]]></title>
      <link>https://nyaa.si/download/3.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/3</guid>
      <pubDate>Sat, 02 May 2026 22:00:00 GMT</pubDate>
      <nyaa:infoHash>ccc</nyaa:infoHash>
    </item>
    <item>
      <title></title>
    </item>
  </channel>
</rss>`;

describe("parseFeed", () => {
  it("parses every well-formed item and silently drops malformed ones", () => {
    const items = parseFeed(fullFeedXml);
    expect(items).toHaveLength(3); // empty-title item dropped
    expect(items.map((i) => i.seriesGuess)).toEqual([
      "Boruto Two Blue Vortex",
      "Boruto",
      "Dandadan",
    ]);
    expect(items[0]?.volume).toBe(2);
    expect(items[1]?.volumeRangeEnd).toBe(14);
    expect(items[2]?.chapterRangeEnd).toBe(142);
  });
});
