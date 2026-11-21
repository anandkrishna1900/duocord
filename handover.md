# PrivCord handover

Paste this into a new chat. Read `CLAUDE.md` in the repo too: it holds the standing rules.

## What PrivCord is
A private peer-to-peer messenger (formerly DuoChat / DuoCord).
- `crates/`: shared Rust engine (engine, crypto, domain, storage, ports, ffi, desktop-host, import).
- `apps/desktop`: Windows app (Tauri + React, plain CSS, no Tailwind; effects in `src/polish.css`).
- `apps/android`: Android app (Kotlin + Compose; talks to the Rust engine through uniffi bindings).
- `site/`: landing site (Vite + React), deployed by `pages.yml` to https://raiden6767.github.io/PrivCord/.
- Pages: Features, How it works, Architecture, Screens, Compare, FAQ, About, Donate. Each has a real URL and prerendered HTML (`site/scripts/pages.mjs`).

## Repo and tooling facts
- The GitHub MCP tools only accept `anandkrishna1900/privcord`. Git remotes show `raiden6767/PrivCord`.
- Work on a branch from `origin/main`, push once, open a draft PR, then merge.
- The permission classifier has sometimes refused `merge_pull_request`. Do not work around it; ask the user to say "merge N".
- Android Kotlin only compiles in CI (not locally). Disk is tight (about 3 GB), so use `CARGO_INCREMENTAL=0`.
- Verify a site deploy by polling `curl` on the live page for the new asset.

## CI rules (user's standing instruction: minutes are limited)
- Test locally first, push once. Every push to a PR restarts its CI.
- Site-only or root `.md` changes skip app CI (`paths-ignore` in `.github/workflows/ci.yml`).
- Re-run a failed job at most once. No empty commits. Merge `main` only for a conflict.
- Do not add heavy jobs (E2E, fuzz, emulator, installers) to every PR without asking.
- Checks: typecheck/unit/build, Rust (ubuntu + windows, migration map, Kotlin bindings diff, cargo audit), Android tests/APK/emulator launch, Windows installer, fuzz, E2E (~15-20 min), CodeQL (set in GitHub settings).

## UI rules (user's standing instruction)
For any UI work use the design skills (ui-ux-pro-max, impeccable) and take ideas from React Bits, 21st.dev, uiverse, mobbin, magicui, ui.watermelon.sh, untitledui, jitter, reactbits.dev. Respect `prefers-reduced-motion`, check licences. The site shows no React Bits credit, but keep licence notices in source and `site/THIRD_PARTY.md`. Never put the user's email on the website.

## State at the end of this chat
All PRs are merged; none are open. Merging is allowed without asking (the user said so): merge when CI is green.
- Site: floating Architecture map with a page-wide animated backdrop; clicking a part opens an animated detail tile (rendered above the header, sticky × plus a bottom Close button, "Connected to" hop buttons); smooth timeline expand; no Play button; no chip rows above the map.
- Themes: both apps use the same 18 palettes (`apps/desktop/src/lib/themes.ts`, `apps/android/.../data/Themes.kt`; ids kept, Gruvbox and Latte added). `themes.test.ts` and the Android `ThemesTest` / `ThemeContrastTest` enforce contrast: Android needs text/bg >= 7, muted text >= 3.5, raw accent on bg >= 3.0. Keep the two lists identical.
- CI lessons: changing a Kotlin file changes the migration map (run `python3 docs/rust/tools/gen_migration_map.py`); run `cargo fmt --all` before pushing Rust (the storage perf test once failed fmt on main); a one-second `deploy-pages` failure is an overlap with the previous deploy, re-run it once.
- Security: `npm audit` shows 0 vulnerabilities (root, relay, site). `cargo audit` shows only unmaintained warnings (Tauri, uniffi). GitHub Security alerts cannot be read from here, so check the repo's Security tab.

## For the user to do
- Test the installer and APK on real devices (CI artifacts).
- Set the repo's Website field to the site URL.
- Install the Claude GitHub app: https://github.com/apps/claude/installations/select_target
- Consider running the heavy E2E and fuzz jobs only on `main`.
- Old "DuoCord backups" folders are not migrated (new folder is "PrivCord backups").

## Known limits and next ideas
- Windows silent-toast sound limit. No resumable file transfers.
- Engine: snapshots return all messages, so they grow with history. Next step: paging or incremental snapshots.
- An independent security audit is still advisable.
- See `docs/KNOWN_LIMITATIONS.md`.
