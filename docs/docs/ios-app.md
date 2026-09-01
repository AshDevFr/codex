# Codex Reader (iOS)

Codex Reader is the native iOS app for Codex. This page covers what the app needs, what it can
open, and where to ask for help.

:::info[Beta]
Codex Reader is currently in a private beta through TestFlight and is not yet available on the
App Store.
:::

## You need a Codex server

Codex Reader is a client for your own Codex server. The app does not come with content: your
comics, manga, and books live on a server that you (or someone you know) run, and the app connects
to it.

If you installed the app and have nothing to read, this is why. To set up a server, start with
[Getting Started](./getting-started.mdx).

The app includes a small built-in sample library so you can try the reader without a server. The
sample is a demonstration, not the product: everything else the app does (your library, synced
progress, statistics) needs a server behind it.

## Server version

Codex Reader requires Codex **2.4.0 or newer**. The app checks the server's version when
connecting:

- A server below 2.4.0 is refused, and the app tells you both the version it found and the version
  it needs.
- A server that does not report a version the app recognises (a fork, a development build, a proxy
  that rewrites responses) gets a warning but is allowed through. Most features will work; some may
  not.

## What the app can open

CBZ, CBR, ZIP, RAR, PDF, and EPUB.

EPUBs open in a dedicated reader that handles both reflowable and fixed-layout books, with the
table of contents, bookmarks, and in-book search.

## Your settings and progress follow you

Reader settings and reading positions belong to the server, not the app. A book started on your
phone resumes in the web reader and vice versa, including the position inside an EPUB chapter. See
[Reader Settings](./reader-settings.md) and [Reading Progress](./reading-progress.md) for how the
server tracks them.

## Signing in with OIDC

If your server uses [OIDC single sign-on](./users/oidc.md), the server must be told to trust the
app's redirect target before sign-in can complete. Add the exact URI `codexreader://auth` to
`auth.oidc.allowed_redirect_uris` in your server configuration:

```yaml
auth:
  oidc:
    allowed_redirect_uris:
      - codexreader://auth
```

Without this entry the server refuses the login request and sign-in from the app fails
immediately. Note that this is **Codex server configuration, not identity-provider
configuration**: nothing needs to change in your IdP. See
[Configuration](./configuration.md#oidc-single-sign-on) for the full OIDC reference.

## Privacy

What you read stays between your device and your server. The app collects nothing unless you
explicitly opt into diagnostics sharing, and even then your content, identity, and server address
never leave the device. The full [privacy policy](/privacy) has the details.

## Getting help

Found a bug in the app? Report it on the
[Codex issue tracker](https://github.com/AshDevFr/codex/issues), and include the app version, your
server version, and what you were doing when it happened.

Codex is a solo side project. Bug reports are read and appreciated, but there is no SLA, and
installation support for the server is not provided: the [documentation](./intro.md) covers
deployment, and setting up a server is expected reading.
