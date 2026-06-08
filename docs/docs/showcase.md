---
sidebar_position: 2
---

# Feature Showcase

Explore the main features of Codex through screenshots and descriptions.

## Home Page

The home page provides quick access to your reading activity and recently added content. The **Keep Reading** section lets you continue where you left off, and the **Recently Added** section helps you discover new additions to your library.

![Home Page](../screenshots/navigation/home-dashboard.png)

## Library Management

### Adding a New Library

Create libraries to organize your comics, manga, and ebooks. Configure general settings, file formats, scanning strategies, and schedules.

#### Comics Library

![Add Library - Comics General](../screenshots/libraries/add-library-general-comics.png)

![Add Library - Comics Strategy](../screenshots/libraries/add-library-strategy-comics.png)

![Add Library - Comics Formats](../screenshots/libraries/add-library-formats-comics.png)

![Add Library - Comics Scanning](../screenshots/libraries/add-library-scanning-comics.png)

#### Manga Library

![Add Library - Manga General](../screenshots/libraries/add-library-general-manga.png)

![Add Library - Manga Strategy](../screenshots/libraries/add-library-strategy-manga.png)

![Add Library - Manga Formats](../screenshots/libraries/add-library-formats-manga.png)

![Add Library - Manga Scanning](../screenshots/libraries/add-library-scanning-manga.png)

#### Books Library

![Add Library - Books General](../screenshots/libraries/add-library-general-books.png)

![Add Library - Books Strategy](../screenshots/libraries/add-library-strategy-books.png)

![Add Library - Books Formats](../screenshots/libraries/add-library-formats-books.png)

![Add Library - Books Scanning](../screenshots/libraries/add-library-scanning-books.png)

### Browsing Libraries

View your libraries by series or books with powerful filtering and sorting options.

![All Libraries - Series View](../screenshots/libraries/all-libraries-series.png)

![All Libraries - Books View](../screenshots/libraries/all-libraries-books.png)

![Library Detail - Series](../screenshots/libraries/library-detail-series.png)

## Series & Book Details

### Series Detail Page

View comprehensive information about a series including metadata, books, and reading progress.

![Series Details Page](../screenshots/libraries/series-detail.png)

### Book Detail Page

View detailed information about a specific book.

![Book Details Page](../screenshots/libraries/book-detail.png)

### Series Actions Menu

Per-series actions live behind the kebab menu in the series header. See the [Managing Series guide](./series-management.md) for details.

![Series Actions Menu](../screenshots/series-detail/actions-menu.png)

### Series Info Modal

Read-only details: UUIDs, file path, timestamps, resolved external IDs.

![Series Info Modal](../screenshots/series-detail/info-modal.png)

### Edit Metadata Modal

The metadata editor opens from the actions menu or any field's edit icon.

![Edit Metadata Modal](../screenshots/series-detail/edit-metadata-modal.png)

### Reset Metadata

Wipe manual edits and revert to scanner / provider values. Confirmation required.

![Reset Metadata Confirmation](../screenshots/series-detail/reset-metadata-confirm.png)

## Bulk Operations

### Bulk Selection Toolbar

Multi-select series cards from the library grid; the toolbar surfaces bulk actions.

![Bulk Selection Toolbar](../screenshots/series-detail/bulk-selection-toolbar.png)

### Bulk Metadata Edit

Patch fields across the entire selection. Tabs cover General, Tags, and Custom metadata.

![Bulk Metadata - General](../screenshots/series-detail/bulk-metadata-general.png)

![Bulk Metadata - Tags](../screenshots/series-detail/bulk-metadata-tags.png)

![Bulk Metadata - Custom](../screenshots/series-detail/bulk-metadata-custom.png)

## Scheduled Library Jobs

Each library can run periodic metadata refresh jobs against a provider plugin. See the [Library Jobs guide](./library-jobs.md).

![Scheduled Jobs - Empty](../screenshots/library-jobs/empty.png)

![Job Editor - Empty](../screenshots/library-jobs/editor-empty.png)

![Job Editor - Filled](../screenshots/library-jobs/editor-filled.png)

## Release Tracking

Codex polls external sources and surfaces new chapter / volume announcements in a dedicated inbox. See the [Release Tracking guide](./release-tracking.md).

### Settings Overview

![Release Tracking Overview](../screenshots/releases/settings-overview.png)

### Polling a Source

Trigger an immediate poll on a source row in **Settings → Release tracking**.

![Settings Before Poll](../screenshots/releases/settings-before-poll.png)

![Settings After Poll](../screenshots/releases/settings-after-poll.png)

### Series Tracking Toggle

Enable tracking on a series; add aliases or external IDs for matching.

![Series Tracking Enabled](../screenshots/releases/series-tracking-enabled.png)

### Releases Inbox

The inbox lists every accepted release. Default view shows new entries; the state filter switches to All / Acquired / Dismissed.

![Releases Inbox - New](../screenshots/releases/inbox-new.png)

![Releases Inbox - All](../screenshots/releases/inbox-all.png)

### Series Releases Panel

Each tracked series exposes a per-series view of the same ledger.

![Series Releases Panel](../screenshots/releases/series-releases-panel.png)

## Readers

### Comic Reader

A powerful comic reader with customizable display settings.

![Comic Reader](../screenshots/reader/comic-view.png)

![Comic Reader Toolbar](../screenshots/reader/comic-toolbar.png)

#### Comic Reader Settings

Customize reading mode, scale, background, and page layout.

![Comic Reader Settings](../screenshots/reader/comic-settings.png)

### EPUB Reader

A beautiful EPUB reader with extensive typography controls and themes.

![EPUB Reader](../screenshots/reader/epub-view.png)

![EPUB Reader Toolbar](../screenshots/reader/epub-toolbar.png)

#### EPUB Settings

Configure fonts, themes, margins, and more.

![EPUB Reader Settings](../screenshots/reader/epub-settings.png)

### PDF Reader

A native PDF reader with zoom controls and page spread options.

![PDF Reader](../screenshots/reader/pdf-view.png)

![PDF Reader Toolbar](../screenshots/reader/pdf-toolbar.png)

#### PDF Settings

Configure zoom levels and page spread modes.

![PDF Reader Settings](../screenshots/reader/pdf-settings.png)

## Mobile & PWA

Codex is a progressive web app: install it to your home screen for a full-screen experience and offline reading. The xs-breakpoint layout collapses the sidebar into a drawer, surfaces a full-screen search sheet, and reflows toolbars and admin tables for one-handed use. See the [Offline Reading guide](./offline-reading.md) for the download model and storage caveats.

| Home dashboard at iPhone width | Series list at iPhone width |
|---|---|
| ![Codex home dashboard on mobile with the Install Codex prompt](../screenshots/mobile/home-dashboard.png) | ![Series list on mobile with the Install Codex prompt](../screenshots/mobile/series-list.png) |

The **Install Codex** prompt nudges first-time iOS Safari visitors toward "Add to Home Screen", which is required for durable offline storage on iOS.

## Settings & Administration

### System

#### Server

Configure server-wide options and custom metadata templates.

![Server Settings](../screenshots/settings/server.png)

![Custom Metadata](../screenshots/settings/server-custom-metadata.png)

![Custom Metadata Templates](../screenshots/settings/server-custom-metadata-templates.png)

#### Tasks

Monitor and manage background tasks like scanning and thumbnail generation.

![Task Queue](../screenshots/settings/tasks.png)

#### Metrics

View statistics about your library contents and monitor performance.

![Metrics - Inventory](../screenshots/settings/metrics.png)

![Metrics - Tasks](../screenshots/settings/metrics-tasks.png)

![Metrics - Plugins Overview](../screenshots/settings/metrics-plugins-overview.png)

![Metrics - Plugins Expanded](../screenshots/settings/metrics-plugins-expanded.png)

#### Plugins

See the [Plugins](#plugins) section below for the full plugin showcase.

### Access

#### Users

Manage users and their permissions.

![User Management](../screenshots/settings/users.png)

#### Sharing Tags

Configure sharing tags for library access control.

![Sharing Tags](../screenshots/settings/sharing-tags.png)

### Library Health

#### Duplicates

Find and manage duplicate files across your libraries.

![Duplicate Detection](../screenshots/settings/duplicates.png)

#### Book Errors

View and manage books with parsing or processing errors.

![Book Errors](../screenshots/settings/book-errors.png)

#### Release Tracking

Configure external sources that announce new chapter and volume releases for tracked series. See the [Release tracking guide](./release-tracking.md) for the full walkthrough.

![Release Tracking Settings](../screenshots/settings/release-tracking.png)

### Storage

#### Thumbnails

Clean up orphaned thumbnail files and database entries.

![Cleanup Settings](../screenshots/settings/cleanup.png)

#### Page Cache

Manage the PDF rendering cache.

![PDF Cache](../screenshots/settings/pdf-cache.png)

#### Plugin Storage

Inspect and reclaim disk space used by plugin file storage.

![Plugin Storage](../screenshots/settings/plugin-storage.png)

#### Data Exports

Export your library catalog as JSON, CSV, or Markdown. See the [Data Exports guide](./exports.md) for details.

![Data Exports](../screenshots/settings/exports.png)

### Account

#### Profile

Manage your account details and preferences.

![Profile - Account](../screenshots/settings/profile.png)

![Profile - Preferences](../screenshots/settings/profile-preferences.png)

#### API Keys

Generate and manage API keys for integrations.

![Profile - API Keys](../screenshots/settings/profile-api-keys.png)

#### Integrations

Users connect plugin integrations (sync, recommendations) from this page.

![User Integrations](../screenshots/settings/integrations.png)

See the [User Integrations](#user-integrations) section under Plugins below for the full flow.

## Plugins

Codex supports extensible plugins for metadata enrichment, reading progress sync, and personalized recommendations. Plugins are managed by admins and connected by users.

### Plugin Management

The plugins page in Settings lets admins manage installed plugins.

![Plugin Settings - Empty](../screenshots/plugins/settings-plugins-empty.png)

### Official Plugin Store

Install plugins from the built-in store with pre-configured settings.

![Official Plugin Store](../screenshots/plugins/store-carousel.png)

Clicking "Add" on a plugin opens a pre-filled creation form with the plugin's recommended settings.

**AniList Sync Plugin:**

![Add Sync Plugin - General](../screenshots/plugins/store-add-sync-general.png)

![Add Sync Plugin - Execution](../screenshots/plugins/store-add-sync-execution.png)

**AniList Recommendations Plugin:**

![Add Recommendations Plugin - General](../screenshots/plugins/store-add-recommendations-general.png)

![Add Recommendations Plugin - Execution](../screenshots/plugins/store-add-recommendations-execution.png)

**MangaUpdates Releases Plugin:**

![Add MangaUpdates Plugin - General](../screenshots/plugins/store-add-mangaupdates-general.png)

![Add MangaUpdates Plugin - Execution](../screenshots/plugins/store-add-mangaupdates-execution.png)

### Adding a Plugin Manually

Plugins outside the store can be added by hand via the "Add Plugin" form, filling in the command and arguments. The echo plugins (used for development and testing) are installed this way.

**Echo Metadata Plugin:**

![Add Echo Metadata - General](../screenshots/plugins/create-echo-metadata-general.png)

![Add Echo Metadata - Execution](../screenshots/plugins/create-echo-metadata-execution.png)

**Echo Sync Plugin:**

![Add Echo Sync - General](../screenshots/plugins/create-echo-sync-general.png)

![Add Echo Sync - Execution](../screenshots/plugins/create-echo-sync-execution.png)

**Echo Recommendations Plugin:**

![Add Echo Recommendations - General](../screenshots/plugins/create-echo-recommendations-general.png)

![Add Echo Recommendations - Execution](../screenshots/plugins/create-echo-recommendations-execution.png)

### Installed Plugins

View and manage installed plugins, their status, and health.

![Plugin Settings - Installed Plugins](../screenshots/plugins/settings-plugins-installed.png)

![Plugin Settings - Expanded Details](../screenshots/plugins/settings-plugin-expanded.png)

You can test plugin connectivity using the "Test Connection" action.

![Plugin Settings - After Test](../screenshots/plugins/settings-plugins-after-test.png)

### Configuring a Plugin

After installing a plugin, open its configuration modal to set permissions, scopes, search templates, preprocessing rules, and conditions.

![Config Modal - General](../screenshots/plugins/config-modal-general.png)

![Config Modal - Permissions](../screenshots/plugins/config-modal-permissions.png)

![Config Modal - Permissions Filled](../screenshots/plugins/config-modal-permissions-filled.png)

![Config Modal - Template](../screenshots/plugins/config-modal-template.png)

![Config Modal - Preprocessing](../screenshots/plugins/config-modal-preprocessing.png)

![Config Modal - Conditions](../screenshots/plugins/config-modal-conditions.png)

For sync-capable plugins, the Permissions tab also exposes a **Sync Schedule (cron)** field that sets the automatic-sync cadence shared by all users of that plugin.

![Config Modal - Sync Schedule](../screenshots/plugins/config-modal-sync-cron.png)

### Using Plugins

Access plugin actions from the library sidebar or series detail pages.

If the plugin is enabled and has the library scope, you will see the plugin dropdown in the library sidebar.

![Library Sidebar - Plugin Dropdown](../screenshots/plugins/library-sidebar-plugin-dropdown.png)

If the plugin is enabled and has the series scope, you will see the plugin dropdown in the series detail page.

![Series Detail - Plugin Dropdown](../screenshots/plugins/series-detail-plugin-dropdown.png)

### Plugin Results

View results after running plugins on your library content.

![Plugin Search Results](../screenshots/plugins/search-results.png)

![Metadata Preview](../screenshots/plugins/metadata-preview.png)

![Apply Success](../screenshots/plugins/apply-success.png)

![Library Auto-Match Success](../screenshots/plugins/library-auto-match-success.png)

![Series Detail - After Plugin](../screenshots/plugins/series-detail-after-plugin.png)

### User Integrations

Users can enable or disable plugin integrations from their account settings, such as sync and recommendation services.

![User Integrations](../screenshots/plugins/user-integrations.png)

![User Integrations - Enabled Sync](../screenshots/plugins/user-integrations-enabled-sync.png)

Once an admin has set a sync schedule, each user can opt their connection into automatic sync.

![User Integrations - Automatic Sync On](../screenshots/plugins/user-integrations-auto-sync-on.png)

Running a manual sync updates the connection's last-sync status and stats.

![User Integrations - Sync Complete](../screenshots/plugins/user-integrations-sync-complete.png)

The connection's settings control which entries are sent during a sync.

![User Integrations - Sync Settings](../screenshots/plugins/user-integrations-sync-settings.png)

![User Integrations - All Enabled](../screenshots/plugins/user-integrations-all-enabled.png)

### Recommendations

When a recommendations plugin is connected, users get personalized suggestions based on their library and reading history.

![Recommendations - Initial](../screenshots/plugins/recommendations-initial.png)

![Recommendations - Results](../screenshots/plugins/recommendations-results.png)

## Authentication

### Login

Secure authentication for your library.

![Login](../screenshots/navigation/login-page.png)

### Setup Wizard

First-time setup for creating your admin account.

![Setup Wizard - Step 1 Empty](../screenshots/setup/wizard-step1-empty.png)

![Setup Wizard - Step 1 Filled](../screenshots/setup/wizard-step1-filled.png)

![Setup Wizard - Step 2 Settings](../screenshots/setup/wizard-step2-settings.png)

![Setup Complete - Dashboard](../screenshots/setup/complete-dashboard.png)

## Navigation

### Sidebar

The sidebar provides quick access to settings and navigation.

![Sidebar - Settings Expanded](../screenshots/navigation/sidebar-settings-expanded.png)

### Search

Search across your entire library.

![Search Dropdown](../screenshots/navigation/search-dropdown.png)

![Search Results](../screenshots/navigation/search-results.png)
