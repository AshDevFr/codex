import { describe, expect, it } from "vitest";
import { parseFeed, parseItem, parseTitle } from "./parser.js";

// -----------------------------------------------------------------------------
// parseTitle
// -----------------------------------------------------------------------------

describe("parseTitle", () => {
  it("extracts chapter, group, and language from canonical English entry", () => {
    const t = parseTitle("c.143 by Best Group (en)");
    expect(t.chapter).toBe(143);
    expect(t.volume).toBeNull();
    expect(t.group).toBe("Best Group");
    expect(t.language).toBe("en");
  });

  it("extracts chapter and volume when both present", () => {
    const t = parseTitle("Vol.2 c.14 by GroupName (en)");
    expect(t.chapter).toBe(14);
    expect(t.volume).toBe(2);
    expect(t.group).toBe("GroupName");
    expect(t.language).toBe("en");
  });

  it("handles decimal chapter numbers", () => {
    const t = parseTitle("c.47.5 by SubScans (en)");
    expect(t.chapter).toBe(47.5);
  });

  it("handles long-form vol./ch. prefixes", () => {
    const t = parseTitle("vol.5 ch.30 by Group (es)");
    expect(t.volume).toBe(5);
    expect(t.chapter).toBe(30);
    expect(t.language).toBe("es");
  });

  it("extracts Spanish entry", () => {
    const t = parseTitle("c.144 by Hablada Scans (es)");
    expect(t.chapter).toBe(144);
    expect(t.language).toBe("es");
    expect(t.group).toBe("Hablada Scans");
  });

  it("extracts Indonesian entry", () => {
    const t = parseTitle("c.145 by ID Translators (id)");
    expect(t.chapter).toBe(145);
    expect(t.language).toBe("id");
  });

  it("defaults language to 'en' when no language tag is present", () => {
    // The MangaUpdates v1 RSS endpoint serves the English-localized release
    // stream and titles ship without a language tag. Defaulting to "en"
    // (rather than the legacy `UNKNOWN_LANGUAGE` sentinel) keeps the
    // client-side language gate from dropping every item.
    const t = parseTitle("c.143 by Best Group");
    expect(t.chapter).toBe(143);
    expect(t.group).toBe("Best Group");
    expect(t.language).toBe("en");
  });

  it("handles volume-only bundle (no chapter)", () => {
    const t = parseTitle("Vol.15 by VolBundlerScans (en)");
    expect(t.volume).toBe(15);
    expect(t.chapter).toBeNull();
    expect(t.group).toBe("VolBundlerScans");
    expect(t.language).toBe("en");
  });

  it("handles entry with no group", () => {
    const t = parseTitle("c.143 (en)");
    expect(t.chapter).toBe(143);
    expect(t.language).toBe("en");
    expect(t.group).toBeNull();
  });

  it("lowercases language codes regardless of source casing", () => {
    const t = parseTitle("c.143 by Group (EN)");
    expect(t.language).toBe("en");
  });

  it("ignores trailing whitespace before language code", () => {
    const t = parseTitle("c.143 by Group (en)   ");
    expect(t.language).toBe("en");
  });
});

// -----------------------------------------------------------------------------
// parseItem
// -----------------------------------------------------------------------------

const englishItem = `
  <title><![CDATA[c.143 by Best Group (en)]]></title>
  <link>https://www.mangaupdates.com/release.html?id=12345</link>
  <guid isPermaLink="false">12345</guid>
  <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
`;

describe("parseItem", () => {
  it("parses a canonical English item", () => {
    const item = parseItem(englishItem);
    expect(item).not.toBeNull();
    if (!item) return;
    expect(item.externalReleaseId).toBe("12345");
    expect(item.title).toBe("c.143 by Best Group (en)");
    expect(item.chapter).toBe(143);
    expect(item.volume).toBeNull();
    expect(item.group).toBe("Best Group");
    expect(item.language).toBe("en");
    expect(item.link).toBe("https://www.mangaupdates.com/release.html?id=12345");
    expect(item.observedAt).toBe("2026-05-04T02:31:00.000Z");
  });

  it("falls back to link as externalReleaseId when guid is missing", () => {
    const xml = `
      <title>c.144 by Group (en)</title>
      <link>https://www.mangaupdates.com/release.html?id=99</link>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
    `;
    const item = parseItem(xml);
    expect(item?.externalReleaseId).toBe("https://www.mangaupdates.com/release.html?id=99");
  });

  it("derives a deterministic id when both guid and link are missing", () => {
    const xml = `
      <title>c.144 by Group (en)</title>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
    `;
    const a = parseItem(xml);
    const b = parseItem(xml);
    expect(a?.externalReleaseId).toBeTruthy();
    expect(a?.externalReleaseId).toBe(b?.externalReleaseId);
    expect(a?.externalReleaseId.startsWith("t:")).toBe(true);
  });

  it("returns null for a malformed item missing title", () => {
    const xml = `<link>https://example.com</link>`;
    expect(parseItem(xml)).toBeNull();
  });

  it("falls back to current time when pubDate is invalid", () => {
    const xml = `
      <title>c.1 by G (en)</title>
      <pubDate>not a real date</pubDate>
    `;
    const item = parseItem(xml);
    expect(item).not.toBeNull();
    if (!item) return;
    expect(Number.isNaN(new Date(item.observedAt).getTime())).toBe(false);
  });

  it("decodes XML entities in title", () => {
    const xml = `
      <title>c.1 by G &amp; B (en)</title>
      <link>https://example.com/x</link>
      <pubDate>Mon, 04 May 2026 02:31:00 GMT</pubDate>
    `;
    const item = parseItem(xml);
    expect(item?.title).toBe("c.1 by G & B (en)");
    expect(item?.group).toBe("G & B");
  });
});

// -----------------------------------------------------------------------------
// parseFeed
// -----------------------------------------------------------------------------

const multilingualFeed = `<?xml version="1.0"?>
<rss version="2.0">
  <channel>
    <title>Test Series Releases</title>
    <item>
      <title><![CDATA[c.142 by EnglishGroup (en)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1001</link>
      <guid isPermaLink="false">1001</guid>
      <pubDate>Mon, 04 May 2026 01:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[c.144 by SpanishGroup (es)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1002</link>
      <guid isPermaLink="false">1002</guid>
      <pubDate>Sun, 03 May 2026 12:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[c.145 by IndonesianGroup (id)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1003</link>
      <guid isPermaLink="false">1003</guid>
      <pubDate>Sat, 02 May 2026 22:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[Vol.15 by VolBundler (en)]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1004</link>
      <guid isPermaLink="false">1004</guid>
      <pubDate>Fri, 01 May 2026 10:00:00 GMT</pubDate>
    </item>
    <item>
      <title><![CDATA[c.146 by NoLanguageTagGroup]]></title>
      <link>https://www.mangaupdates.com/release.html?id=1005</link>
      <guid isPermaLink="false">1005</guid>
      <pubDate>Thu, 30 Apr 2026 09:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>`;

describe("parseFeed", () => {
  it("parses all items in a multi-language fixture", () => {
    const items = parseFeed(multilingualFeed);
    expect(items).toHaveLength(5);
    expect(items[0]?.language).toBe("en");
    expect(items[1]?.language).toBe("es");
    expect(items[2]?.language).toBe("id");
    expect(items[3]?.language).toBe("en");
    expect(items[3]?.volume).toBe(15);
    expect(items[3]?.chapter).toBeNull();
    // Item 4's title carries no language tag; parser defaults to "en"
    // because the MU v1 RSS feed is the English release stream.
    expect(items[4]?.language).toBe("en");
  });

  it("returns an empty array for an empty channel", () => {
    expect(parseFeed("<rss><channel></channel></rss>")).toEqual([]);
  });

  it("returns an empty array for malformed XML", () => {
    // Non-fatal: parseFeed should never throw, just return whatever it can.
    expect(parseFeed("<<<not xml>>>")).toEqual([]);
  });
});
