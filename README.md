# Tailgate

A desktop viewer for [e621](https://e621.net) on macOS and Windows. It can reach e621 through its own
WireGuard tunnel — a WireGuard file or a Mullvad account — without a system VPN, and only the
app's own traffic goes through it.

**Adults only.** e621 hosts sexually explicit content. Use Tailgate only if you are 18 or older
and such content is legal where you live.

Tailgate is not made by or affiliated with e621.

## Features

- Search with tag autocomplete, sorting and an endless grid of thumbnails
- Popular posts by day, week or month
- Pools, read in order, and sets: browse yours or find others, and add posts to your own
- Full-window viewer for images, GIFs and videos, with zoom
- Favorites, votes and downloads with your e621 account (API key)
- Your account's blacklist, applied the same way as on the site
- Connection: direct, a WireGuard `.conf` file, or a Mullvad account with a choice of location
- English and Korean
- Updates from inside the app

## Install (macOS)

Requires an Apple silicon Mac with macOS 14 or later.

1. Download `Tailgate_<version>_aarch64.dmg` from the
   [latest release](https://github.com/nyabi-gh/Tailgate/releases/latest).
2. Open it and drag **Tailgate** into **Applications**.
3. Tailgate is not notarized by Apple, so macOS refuses to open it at first. Run this once in
   Terminal:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Tailgate.app
   ```

   It removes the "downloaded from the internet" mark that makes macOS block the app. Do this
   only for a copy you downloaded from this repository's releases.

Later versions install from inside the app (Settings → About) and do not need the command again.

macOS may ask whether Tailgate can use items in your keychain, where it keeps your e621 API key
and VPN settings. Choose **Always Allow**. Because the app is not signed with an Apple
developer certificate, macOS may ask again after an update.

## Install (Windows)

Requires 64-bit Windows 10 or 11.

1. Download `Tailgate_<version>_x64-setup.exe` from the
   [latest release](https://github.com/nyabi-gh/Tailgate/releases/latest).
2. Run it. Tailgate is not signed with a Microsoft-trusted certificate, so Windows SmartScreen
   may say "Windows protected your PC". Choose **More info**, then **Run anyway**. Do this only
   for a copy you downloaded from this repository's releases.

Later versions install from inside the app (Settings → About). Your e621 API key and VPN settings
are kept in Windows Credential Manager.

## If you installed 0.1.0 before November 2, 2026

The first 0.1.0 build was signed with a key that has since been replaced, so it cannot install
any later version: an update stops with "The update could not be verified". Install the latest
release over it once, as above (on macOS, replace the app in **Applications** and run the
`xattr` command again). Your settings, e621 account and VPN settings stay. Updates from inside
the app work again from then on.

## Build from source

You need [Node.js](https://nodejs.org) 26 and [Rust](https://rustup.rs) 1.98 or later.

```sh
npm ci
npm run tauri dev     # run in development
npm run tauri build   # build the app into src-tauri/target/release/bundle
```

## License

Tailgate is released under the [MIT License](LICENSE). The licenses of the libraries and the font
it includes are listed in
[THIRD-PARTY-NOTICES.html](src-tauri/resources/THIRD-PARTY-NOTICES.html), also available in the
app under Settings → About.
