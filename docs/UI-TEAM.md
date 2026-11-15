# UI team split (crates/anyfont)

Two developers work in `crates/anyfont/src` at the same time. Files are owned, never shared.

## Services UI developer owns
`updater.rs`, `bugreport.rs`, `about.rs` (+ `sysinfo.rs`, `legal.rs` if wanted), `getmore.rs`, `matching.rs`,
`help.rs`, `dlg.rs` (dialog look helpers they may need; the shell developer may reuse them), `docs/HELP.md`.
The public interfaces are fixed by the stubs committed in `c80363b`+1 (struct names, `new`, `open*`, `ui` signatures).
They may ADD public items but must not change or remove the agreed ones.

## Shell UI developer owns
Everything else in `crates/anyfont/src` (`main.rs`, `app.rs`, `theme.rs`, `i18n.rs`, `icons.rs`, title bar, menus,
toolbar, panels, font list, preview rendering, font properties, status bar, context menus, Options, confirmations,
progress, reboot prompt, splash, settings persistence, command line, single instance), `crates/anyfont/build.rs`,
`crates/anyfont/Cargo.toml`. The shell calls the services modules only through the agreed interfaces.

## Shared rules
- Look: `docs/UI-SPEC.md` (incl. §13 lead decisions), `docs/design-tokens.md`, icons in `assets/icons`.
- Every user-visible English string must be a key in `docs/strings-en.tsv` (add new ones there and tell the lead;
  translations live in `assets/i18n/*.tsv`, checked by `python -I scripts/i18n_check.py`).
- The global theme (egui `Style`/`Visuals`) is set by the shell; services dialogs use `ctx.style()` and the tokens,
  never their own global style.
