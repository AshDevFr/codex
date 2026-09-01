---
title: Privacy Policy
description: Privacy policy for the Codex Reader iOS app and the Codex documentation site.
---

# Privacy Policy

**Effective date: August 31, 2026**

This policy covers the **Codex Reader** iOS app and this website (codex.4sh.dev).

## The short version

Codex Reader is a client for a Codex server that you choose and control. Your reading data goes to
that server and nowhere else. The app has no ads, no analytics, and no accounts with us. Nothing
leaves your device for us unless you explicitly turn on diagnostics sharing, and even then, what
you read, who you are, and your server's address are never included.

## Codex Reader (iOS)

### Your content and account

The app connects only to Codex servers you add yourself. Your library, reading progress, reader
settings, ratings, and statistics are exchanged directly between your device and those servers. We
do not operate those servers and never see that data.

Your sign-in credentials and tokens are stored on your device, in the iOS Keychain. They are sent
only to the server you created them for.

The built-in sample library is bundled with the app and works entirely on the device.

### Diagnostics sharing (optional, off by default)

The app can send crash and performance diagnostics so that problems on devices like yours can be
found and fixed. This is **off by default**: until you turn it on, the diagnostics framework is
never started and nothing is ever sent. The choice is stored per device, and you can turn it off
at any time in the app's settings.

If you opt in, the app sends the following to [Sentry](https://sentry.io), an error-monitoring
service (ingested in the United States):

- Crash reports, including reports of app hangs and terminations by the system
- Performance timings for app operations (a sampled subset as detailed traces, the rest as
  aggregate numbers)
- Short log entries about failed or unusually slow operations

Before anything is sent, the app scrubs it: URLs and server hostnames are removed, request data
and machine names are stripped, and no personally identifying information is attached. Your
reading content, your identity, and your server's address never leave the device, whether
diagnostics sharing is on or off.

### What the app does not do

- No advertising, and no advertising identifiers
- No analytics or behavioral tracking
- No selling or sharing of data with third parties (Sentry, above, processes opt-in diagnostics
  only)
- No tracking across other companies' apps or websites

## This website

This documentation site is a static site. It runs no analytics, sets no tracking cookies, and has
no user accounts.

## Changes

If this policy changes, the new version will be published at this address with an updated
effective date.

## Contact

Questions about this policy or about your data can be raised on the
[Codex issue tracker](https://github.com/AshDevFr/codex/issues).
