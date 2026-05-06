import { Page, BrowserContext } from "playwright";
import { captureScreenshot } from "../utils/screenshot.js";
import { waitForPageReady } from "../utils/wait.js";

/**
 * Release tracking scenario.
 *
 * Walks through the end-to-end MangaUpdates flow:
 *  1. Settings → Release tracking (sources table, default schedule)
 *  2. Manga series detail → enable Tracking, add a matching alias
 *  3. Settings → Release tracking → Poll now on the MangaUpdates source
 *  4. /releases inbox after the poll (filters + entries if any returned)
 *  5. Series detail again with the SeriesReleasesPanel populated
 *
 * Assumes the plugins scenario has already installed and tested
 * the MangaUpdates Releases plugin (so the source row exists).
 */
export async function run(page: Page, _context: BrowserContext): Promise<void> {
  console.log("  📡 Capturing release-tracking screenshots...");

  // === STEP 1: Settings page (sources, schedule, notifications) ===
  await captureSettingsPage(page);

  // === STEP 2: Series detail — enable tracking + alias ===
  const seriesUrl = await enableTrackingOnMangaSeries(page);

  // === STEP 3: Trigger Poll Now on the source ===
  await pollMangaUpdatesSource(page);

  // === STEP 4: Releases inbox ===
  await captureReleasesInbox(page);

  // === STEP 5: Series releases panel populated ===
  if (seriesUrl) {
    await captureSeriesReleasesPanel(page, seriesUrl);
  }
}

/**
 * Capture the Release tracking settings page after the MangaUpdates
 * plugin has registered its source. The default-schedule and
 * notification-preferences cards are visible at the top, with the
 * source table below.
 */
async function captureSettingsPage(page: Page): Promise<void> {
  console.log("    📷 Settings — Release tracking");

  await page.goto("/settings/release-tracking");
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  await captureScreenshot(page, "releases/settings-overview");
}

/**
 * Navigate to the manga library's first series, expand the Tracking
 * panel, flip the toggle on, and add an alias matching the
 * MangaUpdates title. Returns the series URL for later reuse.
 */
async function enableTrackingOnMangaSeries(page: Page): Promise<string | null> {
  console.log("    📷 Series detail — enable tracking");

  // Prefer the Manga library so RTL covers show in screenshots.
  const mangaLibraryLink = page.locator('nav a[href*="/libraries/"]:has-text("Manga")').first();
  if ((await mangaLibraryLink.count()) > 0) {
    await mangaLibraryLink.click();
  } else {
    await page.goto("/libraries/all/series");
  }
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  const seriesCard = await page.$('[data-testid="series-card"], .series-card, a[href*="/series/"]');
  if (!seriesCard) {
    console.log("      ⚠️  No series found, skipping tracking enable");
    return null;
  }
  await seriesCard.click();
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  const seriesUrl = page.url();

  // Find the Release tracking card (header reads "Release tracking").
  const trackingHeader = page.locator(
    'button[aria-label="Expand release tracking"], button[aria-label="Collapse release tracking"]',
  ).first();
  if ((await trackingHeader.count()) === 0) {
    console.log("      ⚠️  Tracking panel not found (no release-source plugin enabled?)");
    return seriesUrl;
  }

  // Expand the panel.
  await trackingHeader.click();
  await page.waitForTimeout(400);

  // Flip the tracked toggle. The Switch is a sibling of the header
  // button inside the same Card — querying by aria-label is robust.
  const trackedSwitch = page.locator('input[aria-label="Toggle release tracking"]').first();
  if ((await trackedSwitch.count()) > 0) {
    const isOn = await trackedSwitch.isChecked();
    if (!isOn) {
      // Mantine wraps the input in a label, so click the visible track
      // (the input itself is visually hidden).
      const switchTrack = page.locator(
        'label:has(input[aria-label="Toggle release tracking"])',
      ).first();
      await switchTrack.click();
      await page.waitForTimeout(800);
    }
  }

  // Add an alias the MangaUpdates feed will match against.
  // The fixture is "Give My Regards to Black Jack" — its MU listing
  // matches that exact title; no external ID is required.
  const aliasInput = page.locator('input[placeholder*="alias" i], input[aria-label*="alias" i]').first();
  if ((await aliasInput.count()) === 0) {
    // Fall back to the only TextInput inside the matcher-aliases section.
    const fallback = page.locator(
      'div:has(> div:has-text("Matcher aliases")) input[type="text"], div:has-text("Matcher aliases") + * input[type="text"]',
    ).first();
    if ((await fallback.count()) > 0) {
      await fallback.fill("Give My Regards to Black Jack");
      await page.keyboard.press("Enter");
    }
  } else {
    await aliasInput.fill("Give My Regards to Black Jack");
    await page.keyboard.press("Enter");
  }
  await page.waitForTimeout(800);

  // Capture the panel expanded with tracking enabled.
  await captureScreenshot(page, "releases/series-tracking-enabled");

  return seriesUrl;
}

/**
 * Hit "Poll now" on the MangaUpdates source row in
 * Settings → Release tracking. Waits for the in-flight indicator to
 * clear so any returned entries land in the ledger before we capture
 * the inbox.
 */
async function pollMangaUpdatesSource(page: Page): Promise<void> {
  console.log("    📷 Triggering Poll Now on MangaUpdates source");

  await page.goto("/settings/release-tracking");
  await waitForPageReady(page);
  await page.waitForTimeout(500);

  // Find the row containing "MangaUpdates" and click its Poll Now action.
  // The action icons render a refresh icon inside a Tooltip — we click
  // the IconRefresh button on the matching row.
  const row = page.locator('tr:has-text("MangaUpdates")').first();
  if ((await row.count()) === 0) {
    console.log("      ⚠️  MangaUpdates source row not found");
    return;
  }

  // Capture the row before polling (status idle).
  await captureScreenshot(page, "releases/settings-before-poll");

  const pollNowButton = row.locator('button:has(svg.tabler-icon-refresh)').first();
  if ((await pollNowButton.count()) === 0) {
    console.log("      ⚠️  Poll Now button not found on MangaUpdates row");
    return;
  }
  await pollNowButton.click();
  await page.waitForTimeout(500);

  // The button shows a loading spinner while the poll is in flight.
  // Wait for it to clear (max 60s — MangaUpdates RSS is usually fast
  // but can stall on rate limits).
  const start = Date.now();
  const maxWait = 60_000;
  while (Date.now() - start < maxWait) {
    const stillLoading = await row.locator('.mantine-Loader-root').count();
    if (stillLoading === 0) break;
    await page.waitForTimeout(1000);
  }

  // Re-fetch the page to surface the updated last-poll timestamp.
  await page.reload();
  await waitForPageReady(page);
  await page.waitForTimeout(500);
  await captureScreenshot(page, "releases/settings-after-poll");
}

/**
 * Capture the /releases inbox after the poll. Captures both states
 * since on a fresh MangaUpdates poll the inbox may be empty (no
 * recent chapters in the user's languages) or populated.
 */
async function captureReleasesInbox(page: Page): Promise<void> {
  console.log("    📷 Releases inbox");

  await page.goto("/releases");
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  // Default state filter is "New" (announced).
  await captureScreenshot(page, "releases/inbox-new");

  // Switch to "All" to surface anything regardless of state — useful
  // when the poll only landed dismissed/ignored entries.
  const stateFilter = page.locator('[data-testid="releases-state-filter"]').first();
  if ((await stateFilter.count()) > 0) {
    await stateFilter.click();
    await page.waitForTimeout(300);
    const allOption = page.locator('[role="option"]:has-text("All")').first();
    if ((await allOption.count()) > 0) {
      await allOption.click();
      await page.waitForTimeout(800);
      await captureScreenshot(page, "releases/inbox-all");
    } else {
      await page.keyboard.press("Escape");
    }
  }
}

/**
 * Re-open the manga series and capture the SeriesReleasesPanel.
 * The panel only renders once `tracking.tracked === true` and a
 * release-source plugin is applicable to the library.
 */
async function captureSeriesReleasesPanel(page: Page, seriesUrl: string): Promise<void> {
  console.log("    📷 Series detail — releases panel");

  await page.goto(seriesUrl);
  await waitForPageReady(page);
  await page.waitForTimeout(800);

  // The releases panel header is a div with role=button + aria-label
  // "Expand releases".
  const releasesHeader = page.locator('[aria-label="Expand releases"]').first();
  if ((await releasesHeader.count()) > 0) {
    await releasesHeader.click();
    await page.waitForTimeout(800);
  }

  await captureScreenshot(page, "releases/series-releases-panel");
}
