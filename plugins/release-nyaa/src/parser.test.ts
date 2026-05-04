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
    expect(item.externalReleaseId).toBe("https://nyaa.si/view/12345"); // guid wins
    expect(item.infoHash).toBe("abc123def456"); // lowercased
    expect(item.chapter).toBe(142);
    expect(item.seriesGuess).toBe("Chainsaw Man");
    expect(new Date(item.observedAt).toISOString()).toBe("2026-05-04T02:31:00.000Z");
  });

  it("returns null when title is missing", () => {
    expect(parseItem("<item><link>x</link></item>")).toBeNull();
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
