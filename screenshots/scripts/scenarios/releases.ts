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

  // Add a matcher alias. "Say Hello to Black Jack" is the canonical
  // MangaUpdates title for the fixture series, so the feed matches even
  // before the external ID is wired up.
  await ensureAlias(page, "Say Hello to Black Jack");

  // Add the MangaUpdates external source ID so polls match by ID rather
  // than relying on title fuzz. `hly6oqa` is the MU series slug for
  // "Say Hello to Black Jack".
  await ensureExternalId(page, "api:mangaupdates", "hly6oqa");

  // Capture the panel expanded with tracking enabled.
  await captureScreenshot(page, "releases/series-tracking-enabled");

  return seriesUrl;
}

/**
 * Add a matcher alias to the series tracking panel if it's not already
 * present. The alias input is inside the "Release tracking" card,
 * placeholder "Add an alias…", paired with an "Add" submit button.
 */
async function ensureAlias(page: Page, alias: string): Promise<void> {
  const existing = page.locator(`[aria-label="Remove alias ${alias}"]`).first();
  if ((await existing.count()) > 0) {
    console.log(`      ✓ Alias already present: ${alias}`);
    return;
  }
  const aliasInput = page.locator('input[placeholder="Add an alias…"]').first();
  if ((await aliasInput.count()) === 0) {
    console.log("      ⚠️  Alias input not found, skipping");
    return;
  }
  await aliasInput.fill(alias);
  // The input is wrapped in a <form onSubmit>; the visible submit is the
  // "Add" button next to it. Click it explicitly to avoid relying on
  // Enter-to-submit behaviour.
  const addAliasButton = page
    .locator('form:has(input[placeholder="Add an alias…"]) button[type="submit"]')
    .first();
  await addAliasButton.click();
  // Wait for the chip to render (mutation is optimistic/short).
  await page
    .locator(`[aria-label="Remove alias ${alias}"]`)
    .first()
    .waitFor({ state: "visible", timeout: 5000 })
    .catch(() => console.log(`      ⚠️  Alias chip for "${alias}" never appeared`));
}

/**
 * Open the "Edit External Source IDs" modal from the series header and
 * add a (source, externalId) pair if it's not already configured. The
 * modal is opened by the small pencil ActionIcon rendered alongside the
 * external-id badges (Tooltip label "Edit external IDs").
 */
async function ensureExternalId(
  page: Page,
  source: string,
  externalId: string,
): Promise<void> {
  // Open the modal via the pencil icon. Mantine renders the Tooltip's
  // accessible name on the wrapped ActionIcon as aria-label, but here
  // it falls back to clicking the icon button by its tooltip text.
  const editButton = page
    .locator('button[aria-label="Edit external IDs"], [data-testid="edit-external-ids"]')
    .first();
  if ((await editButton.count()) === 0) {
    // Fall back: the ActionIcon sits next to the external-id badges and
    // wraps an IconEdit svg. Find it by the icon class.
    const iconButton = page.locator('button:has(svg.tabler-icon-edit)').first();
    if ((await iconButton.count()) === 0) {
      console.log("      ⚠️  External IDs edit button not found, skipping");
      return;
    }
    await iconButton.click();
  } else {
    await editButton.click();
  }

  // Wait for the modal.
  const modal = page.locator('.mantine-Modal-content:has-text("Edit External Source IDs")').first();
  try {
    await modal.waitFor({ state: "visible", timeout: 5000 });
  } catch {
    console.log("      ⚠️  External IDs modal did not open");
    return;
  }

  // If the (source, id) pair already exists, just close.
  const existingSource = modal.locator(`input[value="${source}"]`).first();
  if ((await existingSource.count()) > 0) {
    console.log(`      ✓ External ID already present: ${source} = ${existingSource}`);
    const cancel = modal.locator('button:has-text("Cancel")').first();
    await cancel.click();
    return;
  }

  // Click "Add" inside the modal to create a new entry row, then fill
  // the source + externalId inputs by their placeholders.
  const addEntry = modal.locator('button:has-text("Add")').first();
  await addEntry.click();
  await page.waitForTimeout(200);

  const sourceInput = modal.locator('input[placeholder="e.g. plugin:anilist"]').last();
  const idInput = modal.locator('input[placeholder="e.g. 12345"]').last();
  await sourceInput.fill(source);
  await idInput.fill(externalId);

  // Save.
  const saveButton = modal.locator('button:has-text("Save Changes")').first();
  await saveButton.click();

  // Modal closes on success.
  await modal.waitFor({ state: "hidden", timeout: 5000 }).catch(() => {
    console.log("      ⚠️  External IDs modal did not close after save");
  });
  await page.waitForTimeout(400);
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

  // Wait for the poll task to finish. We watch for two signals on the
  // MangaUpdates row:
  //   1. The Mantine Loader on the Poll Now button disappears
  //      (`pollNowPending` flips back to false, driven by the SSE
  //      release_source_polled event or the 5s task-progress refetch).
  //   2. The status badge transitions away from "Never polled" — either
  //      to "OK" (success, lastPolledAt populated) or "Errored" (fail).
  //
  // The MU RSS feed is normally fast (<5s), but rate limits or network
  // hiccups can stretch it; cap at 120s so a stalled run fails loud
  // instead of silently capturing a still-spinning row.
  const start = Date.now();
  const maxWait = 120_000;
  let pollSettled = false;
  while (Date.now() - start < maxWait) {
    const spinnerCount = await row.locator('.mantine-Loader-root').count();
    const hasTerminalBadge =
      (await row.locator('.mantine-Badge-root:has-text("OK")').count()) > 0 ||
      (await row.locator('.mantine-Badge-root:has-text("Errored")').count()) > 0;
    if (spinnerCount === 0 && hasTerminalBadge) {
      pollSettled = true;
      break;
    }
    await page.waitForTimeout(1000);
  }
  if (!pollSettled) {
    console.log("      ⚠️  Poll did not settle within 120s — capturing current state anyway");
  } else {
    const errored = await row.locator('.mantine-Badge-root:has-text("Errored")').count();
    if (errored > 0) {
      console.log("      ⚠️  Poll completed with errors (Errored badge present)");
    } else {
      console.log("      ✓ Poll completed (OK badge visible)");
    }
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
