# CLAUDE.md

## Style

- Keep everything short: replies, explanations, comments, docs.
- Use simple English: short sentences, common words.
- Avoid em dashes (—). Do not just swap them for "-" either. Rewrite the sentence instead, for
  example with a comma, a colon, brackets or two sentences.

## Commits

- English only.
- Conventional Commits prefix: `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `ci:`, `build:`, `test:`.
- Subject as short as possible. Imperative, lowercase, no trailing period.
- Body only when something genuinely cannot be inferred from the diff.
- Never add `Co-Authored-By`, "Generated with" or any other AI attribution to commits or PR descriptions.
- Do not commit or push before I have checked the changes locally and said they are fine. Then
  commit and push. Several attempts at the same thing make one commit, and attempts that did not
  work make none. Different things done in one session get a commit each. This also goes for
  releases and tags.

## Project

- A plugin for [Feishin](https://github.com/jeffvli/feishin) in two versions, with the same plugin
  ID, actions, pictures, settings page and texts:
  - TypeScript with Elgato's Stream Deck SDK (`src/`), for the Stream Deck app (Windows, macOS).
  - Rust (`opendeck/`), for OpenDeck on Linux, without Node.js. It talks to OpenDeck through the
    `openaction` crate and runs on one thread (tokio `LocalSet`). Never hold a `RefCell` borrow
    across an `.await`.
- Plugin ID is `de.felitendo.feishin` (folder `de.felitendo.feishin.sdPlugin`). Do not change it:
  the Stream Deck app and OpenDeck keep profiles and settings by it.
- `de.felitendo.feishin.sdPlugin` holds what both versions share: pictures, layouts, the settings
  page (`ui/`), the texts (`en.json`, `de.json`) and the manifest. `cargo xtask build` puts the
  plugin folder for OpenDeck together in `opendeck/build/`, with a manifest for Linux.
- Change both versions together, so they work and look the same. `cargo test` compares the pictures
  with the ones of the TypeScript version (`opendeck/test/*.txt`, written by
  `node opendeck/test/pictures.mts`). Settings come from the settings page as loose JSON: Rust reads
  them into typed structs with `src/settings.rs` (like JavaScript would: truthy, `Number()`,
  `String()`).
- TypeScript: `npm run build` builds `de.felitendo.feishin.sdPlugin/bin/plugin.js`, `npm run
  typecheck` checks the types, `npm run pack` makes `dist/de.felitendo.feishin.streamDeckPlugin`.
- Rust, in `opendeck/`: `make check` runs the tests, `make install` builds for this PC and installs it
  into OpenDeck. `make package` makes `dist/de.felitendo.feishin.streamDeckPlugin` for x86_64 and
  aarch64 with an old glibc (2.28). It needs zig, `cargo-zigbuild` and the Rust library for aarch64
  (Arch: `rust-aarch64-gnu`). `cargo xtask` lists the dev tools.
- Test in the real apps. On a real Stream Deck (`lsusb | grep -i 0fd9`), only test in a new profile of
  its own and select the old profile again when done. Else use the virtual deck of OBS Tools
  (`make install-virtual-deck` there): stop OpenDeck, run `cargo xtask profile test/e2e/layout.json`,
  start OpenDeck and run `cargo xtask e2e`. The test brings its own fake Feishin on port 4333, so quit
  Feishin first. With `SHOTS_DIR` set, it saves what the deck shows, to compare both versions.
  `cargo xtask fake-feishin` runs the fake alone, to try the keys by hand.
- Keys and settings pages look like the other FeloDeck plugins, OBS Tools and the Voicenux plugin.

## Releases

1. Read `git log <last tag>..HEAD` and pick the version: only fixes → patch, something new → minor,
   something that breaks or needs the user to act → major.
2. Set `Version` in `de.felitendo.feishin.sdPlugin/manifest.json` (`2.1.0.0` for v2.1.0), in one
   commit (`chore: release v2.1.0`).
3. Push, then push the tag: `git tag v2.1.0 && git push origin main v2.1.0`.
4. In `opendeck/`, `make package`. Then `gh release create v2.1.0` with notes like the earlier
   releases (What's new, Install, Full Changelog link) and only this file (the OpenDeck version),
   named `Feishin-v2.1.0.streamDeckPlugin`. Never attach a second file (the Stream Deck app version,
   a `.zip`): the plugin store of OpenDeck then asks which file to download.
5. `npm run pack` makes the Stream Deck app version. It only goes to the Elgato Marketplace, and I
   upload it there myself.

Write release notes for users: what they notice, not how the code does it.
