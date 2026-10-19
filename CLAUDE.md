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

- A plugin for [Feishin](https://github.com/jeffvli/feishin), in TypeScript with Elgato's Stream
  Deck SDK. It runs in the Stream Deck app (Windows, macOS) and in OpenDeck on Linux.
- Plugin ID is `de.felitendo.feishin` (folder `de.felitendo.feishin.sdPlugin`). Do not change it:
  the Stream Deck app and OpenDeck keep profiles and settings by it.
- `npm run build` builds `de.felitendo.feishin.sdPlugin/bin/plugin.js`, `npm run typecheck` checks
  the types, `npm run pack` makes `dist/de.felitendo.feishin.streamDeckPlugin`.
- Keys and settings pages look like the other FeloDeck plugins, OBS Tools and the Voicenux plugin.

## Releases

1. Read `git log <last tag>..HEAD` and pick the version: only fixes → patch, something new → minor,
   something that breaks or needs the user to act → major.
2. Set `Version` in `de.felitendo.feishin.sdPlugin/manifest.json` (`2.1.0.0` for v2.1.0), in one
   commit (`chore: release v2.1.0`).
3. Push, then push the tag: `git tag v2.1.0 && git push origin main v2.1.0`.
4. `npm run pack`, then `gh release create v2.1.0` with notes like the earlier releases (What's new,
   Install, Full Changelog link) and only the plugin file, named `Feishin-v2.1.0.streamDeckPlugin`.
   Never attach a `.zip` as well: the plugin store of OpenDeck then asks which file to download.

Write release notes for users: what they notice, not how the code does it.
