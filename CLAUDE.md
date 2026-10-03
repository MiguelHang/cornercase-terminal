# cornercase

A terminal multiplexer TUI in Rust. A sidebar of **projects** (folders), a column with the active project's **workspaces** (lines of work, optionally each in its own git worktree) and their **tabs** (one or more shells, split like Ghostty), and the active tab's panes. Everything is driven by mouse buttons. An issues modal (GitHub, Shortcut, Linear) reads an issue and starts a coding agent on it in its own worktree. A background server owns the shells, so closing the UI leaves them running and the next `cornercase` reattaches.

## Commands

```sh
cargo run                                   # attach to the server (starts it if none is running)
cargo run -- kill-server                    # stop the server and every shell in it
cargo test --locked                         # unit + snapshot + e2e tests
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt
cargo-machete                               # unused dependencies (not `cargo machete`, which errors)
npx -y jscpd@4.3.0                          # copy-paste detector, reads .jscpd.json
```

- **Building needs Zig 0.15.2 on the PATH**: `libghostty-vt-sys` runs `zig build` on Ghostty's sources, and Ghostty pins the Zig minor version. The first build per profile clones Ghostty (network, ~100 s, ~530 MB in `target/`); `GHOSTTY_SOURCE_DIR` can point at a local checkout.
- **The toolchain is pinned** in `rust-toolchain.toml` (version and components), and CI installs exactly that one, so a new Rust release cannot break CI with new lints. Bump it by hand: change the version, then fix what clippy reports.
- `.cargo/config.toml` sets `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`; a Zig Debug build of Ghostty is too slow to use.
- **After rebuilding, run `cargo run -- kill-server`**: the client refuses to attach to a server from another build.
- Snapshots (`insta`): a changed render writes `src/snapshots/*.snap.new` and fails. Check it, then accept with `INSTA_UPDATE=always cargo test`. Delete snapshots of renamed or removed tests by hand.
- **Before calling a task done, run fmt, clippy, tests, `cargo-machete` and jscpd, and fix what fails.** If one cannot pass, say so.
- **A change that adds, removes or changes a feature updates the website in the same change** (see Website below).
- `jscpd` fails above 1 % duplication (50-token clones) in `src/` and `tests/`. It is a ratchet: extract the shared code, do not raise the threshold. `cargo-machete` is a text search; a false positive goes in `[package.metadata.cargo-machete] ignored`.
- **Every pull request that changes the app bumps `version` in `Cargo.toml` and adds a `## <version>` section to `CHANGELOG.md`** (a patch unless told otherwise; the section is written for users, it becomes the release notes and the update dialog's text). "The app" is `src/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`; docs, tests and `site/` alone need no bump. CI checks it (`.github/version.sh check`, in the guardrails job). Main requires branches to be up to date, so two pull requests cannot ship the same version.
- **Releases are automatic**: on a push to `main`, `.github/workflows/version.yml` runs `.github/version.sh release`, which tags `v<version>` if it is new and dispatches `.github/workflows/release.yml` (`dist`, `dispatch-releases`, because a tag pushed with `GITHUB_TOKEN` triggers no workflow). That builds the four targets, makes the GitHub Release (notes from `CHANGELOG.md`) with `cornercase-installer.sh`, and pushes the formula to `usecornercase/homebrew-tap` (secret `HOMEBREW_TAP_TOKEN`, a fine-grained token that expires). If the app changed but the version was already released, the job fails. `release.yml` is generated: change `dist-workspace.toml` or `.github/build-setup.yml` (the Zig step), then run `dist generate`; never edit it by hand. `site/public/install.sh` (served at `usecornercase.dev/install.sh`) runs the latest release's installer.
- **CI** (`.github/workflows/ci.yml`) runs the same checks: clippy + tests on Linux and macOS (one job per OS so clippy and tests share the Ghostty build; `target/` cached by `Swatinem/rust-cache`, saved from `main` only), and fmt + machete + jscpd on Linux. Dependabot groups action and crate updates monthly; `libghostty-vt` is pinned with `=` (pre-1.0 API), so bumping it needs a manual check.
- **Code review**: CodeRabbit (free for public repos) reviews every non-draft pull request; `.coderabbit.yaml` configures it. It reads this file as its guidelines. Its docstring check and docstring/test generation are off (the code has no comments on purpose), and so is its clippy (it cannot build Ghostty without Zig; CI runs clippy).

## Code conventions

- Everything in English: identifiers, test names, messages, UI text.
- **No comments at all**, including `///` and `//!`. Rationale goes in this file.
- Errors: `crate::error::Error` (`thiserror`) in the library; `anyhow` only in `src/main.rs`.
- No `unwrap()` outside tests (`clippy::unwrap_used`); in tests prefer `expect("what")`.
- Mutexes are `parking_lot::Mutex`.
- Lints live in `Cargo.toml` (clippy `all` deny, `pedantic` warn). Silence one only locally with `#[expect(clippy::x, reason = "...")]`.
- Imports in three groups: `std`, external crates, then `super`/`crate`.
- `rustfmt.toml`: `max_width = 120`.

## Architecture

```
src/main.rs       client, `server` or `kill-server` from argv (uses anyhow)
src/client.rs     the UI process: terminal setup/teardown, colour query, starts the server, forwards events, writes frames
src/server.rs     the daemon: owns App and every Term, accepts clients on a Unix socket, draws a ratatui frame per client
src/protocol.rs   messages, length-prefixed postcard framing, socket and lock paths, build id
src/state.rs      the saved session as JSON, migrations, and the Saver that writes it once it settles
src/config.rs     user settings (config.json), `~` expansion, validation
src/settings.rs   the settings modal's state; returns Actions for App
src/agents.rs     known coding agents, their modes and arguments, which agent takes an issue, detection, trust prompt
src/launch.rs     starting an agent in a new tab (pure state machine)
src/secrets.rs    Shortcut / Linear tokens in secrets.json (0600)
src/markdown.rs   Markdown -> wrapped ratatui Lines
src/highlight.rs  syntax highlighting of fenced code (syntect scopes -> palette colours), cached
src/issues/       issue model and clients: github.rs (gh CLI), shortcut.rs (REST), linear.rs (GraphQL), http.rs, browser.rs (modal state), cache.rs (lists on disk)
src/clipboard.rs  OSC 52
src/worktree.rs   `git worktree add`/`remove`, checkout path, `.worktreeinclude`
src/upstream.rs   `git fetch` and commits to pull per workspace (`↓n`)
src/search.rs     global search: candidates, ranking, state
src/picker.rs     folder picker state
src/process.rs    a pid's cwd, name and arguments: /proc on Linux, libproc and sysctl on macOS
src/project.rs    Project > Workspace > Tab > panes, labels, removal
src/split.rs      a tab's split tree: rects, dividers, splitting, removing, ratios
src/app.rs        App state; turns AppEvents into actions; builds the View
src/term.rs       a shell in a PTY, its Emulator, and the reader thread
src/emulator.rs   wraps libghostty-vt; takes plain Snapshots for ui
src/ui.rs         layout, hit testing and drawing from a plain View (no PTYs)
src/keys.rs       KeyEvent -> bytes (Ghostty's encoder for special keys, legacy encoder for the rest)
src/mouse.rs      MouseEvent -> bytes in the protocol the program asked for
src/host_theme.rs asks the outer terminal for its colours
src/git.rs        branch from .git/HEAD, repo roots, linked worktrees (no git process)
src/update.rs     update check against GitHub releases, download, checksum, binary swap
src/error.rs      library error type
```

**Server loop:** the accept thread, one reader thread per client, the signal thread and a forwarder for PTY output all send `ServerEvent`s over one `mpsc` channel. The loop waits for an event or a 500 ms tick, drains the queue, then draws once per client. Each client has a writer thread so a slow one never blocks the loop. **Client:** the main thread writes each `Frame` to stdout; an input thread sends every crossterm event.

`ui::draw` takes a `View`, not `App`, so rendering is testable with `TestBackend`. Geometry functions (`ui::layout`, `entry_row`, `workspace_row`, `form_buttons`, `picker_item`, …) serve both drawing and hit testing, so tests take positions from them.

## Design decisions

**Interaction**
- **Mouse buttons only, no app shortcuts.** Every key goes to the program in the active pane, except while a modal or the search is open (then `Enter` submits, `Esc` cancels). Do not add keyboard shortcuts without asking. No `Alt` shortcuts (Option is a compose key on macOS), no `Ctrl+letter` (steals shell bindings). `e2e::ctrl_b_reaches_the_shell` guards this.
- Closing the last project leaves the app open and empty. ` quit ` only detaches.
- `×` buttons only show while hovering their row. Names are cut at the end (`ui::truncate_right`), paths at the start (`truncate_left`).
- At most one overlay is open (menu, form, confirmation, settings, picker, issues, search). While it is open, no mouse event reaches the columns or the pane.
- Overlays, hover, scroll, column widths and the toast live in `App` and are shared by every attached client.

**Layout (`ui.rs`)**
- Three columns: projects (32), workspaces (26), pane (after one blank column). Columns are always present so PTY sizes do not jump. Borders drag to resize (double click resets), within `MIN_COLUMN_WIDTH` and `MIN_PANE_WIDTH`; `Widths::fit` squeezes them on small terminals. Widths are saved in the session. PTYs follow the drag live.
- Lists scroll by item with the wheel, show ` ↑ n more` / ` ↓ n more`, and keep the `+` button sticky at the bottom. The scroll follows the active item only when it changes (`App::follow`), otherwise the wheel could never move away from it.
- The active project and tab get a cyan `▌` and a surface background (palette 236, or 254 on light themes). Colours use palette indices so they work without truecolor; the brand purple is index 99.
- **Compact mode** below 90 columns (`ui::compact_layout`): a top bar (` ≡ `, `project › workspace › tab`, ` ⌕ `) and a full-screen menu showing one column at a time. Every clickable item is a 3-row band (`Areas::pitch`). Both columns' areas share one rect and `Areas::shown(nav)` blanks the hidden one, so drawing and hit testing reuse the wide-mode code.

**Hierarchy (`project.rs`)**
- Project (a canonicalized folder) > workspace (the project folder, or its own git worktree) > tab > panes. Every level has an id from one counter; menus, forms and background jobs refer to ids, never indices.
- Labels: custom name, else folder name (project), branch or `default` (workspace), foreground program (tab).
- Closing kills processes; removal waits for `Exited`, never synchronous. Closing the last tab keeps the workspace; closing the last workspace keeps the project.

**Splits (`split.rs`)**
- A binary tree (`Leaf` / `Split { dir, ratio, first, second }`). Right-click in a pane opens split/close/right-click-passthrough; this works even when the program captured the mouse, since almost no program uses the right button.
- A vertical divider is followed by a blank column so text does not touch it. Inactive panes are dimmed (configurable). A left click on an inactive pane only focuses it. Dividers drag like column borders.

**Search (`search.rs`)**
- One global search across projects, workspaces (label and branch) and tabs (label and their workspace's keys). Ranking: exact, prefix, substring; ties by kind then sidebar order. Results are recomputed on every key and draw; the query is never kept after closing.

**Folder picker (`picker.rs`)**
- Starts in the parent of the active project. `Enter` goes into the selected folder, or opens the current one when nothing is selected. Typing a path walks it; a paste goes through the same path (e2e opens projects this way). Folders are read on the main thread.

**Worktrees (`worktree.rs`)**
- Only when the project folder is a repo root. The name is the branch; the slugged name is the folder, under `<worktrees folder>/<repo>/<slug>` (`~/.cornercase/worktrees` by default). `git worktree add` runs on a thread and answers with an `AppEvent`.
- Every linked worktree of the repo shows up, also ones created outside cornercase (`App::refresh`, throttled to once a second).
- Removing asks first, runs `git worktree remove` (offering `--force` when git refuses) and never deletes the branch.
- `.worktreeinclude`: files matching it **and** ignored by git are copied into a new checkout. Matching is delegated to git (`ls-files`, `check-ignore`).
- **Commits to pull** (`↓n` at the end of a workspace row, before the `×`): every 3 s a thread per project counts `HEAD..@{upstream}` in each workspace and answers with `AppEvent::Behind`; it runs `git fetch --all` first once `fetch_minutes` (default 5, 0 turns all of it off) have passed. One thread per project at a time. Git never prompts (`GIT_TERMINAL_PROMPT=0`, empty `GIT_ASKPASS`, `SSH_ASKPASS_REQUIRE=never`); failures count as 0.

**Issues (`issues/`)**
- `Browser` is pure state that returns `Action`s; `App` does the I/O on threads. Answers carry their query and issue key so stale ones are dropped. Lists are cached in memory and in `issues.json` (titles and metadata only, never tokens).
- GitHub goes through `gh` in the project folder. Shortcut (REST v3) and Linear (GraphQL) go through `ureq`, capped at 100 issues. People filters go into each tracker's query.
- Tokens come from `SHORTCUT_API_TOKEN` / `LINEAR_API_KEY` or are typed in the app, checked, and saved to `secrets.json` (0600). `issues::Secret` hides them in `Debug`; they are never logged.
- **Starting an issue**: in a repo root, a worktree on `issue-<n>-<slug>` / `sc-<n>-<slug>` / `ENG-123-<slug>`; elsewhere a new tab. Shortcut and Linear issues ask which open project or workspace to use. Then the agent starts with the prompt.
- **Copy URL** uses OSC 52 through `App::host_writes`, because the server may run on another machine.
- Markdown renders with `pulldown-cmark`; code blocks are highlighted with syntect (pure Rust `regex-fancy`), mapping scopes to palette colours so the terminal theme applies. Highlighting is cached and warmed on the reading thread; syntect and its regex crates build with `opt-level = 3` in dev.

**Agents (`agents.rs`, `launch.rs`)**
- Config: `agent` (`auto` = the one running in the tab, else ask), `agent_args`, `agent_modes`, `agent_commands`, `prompt` (placeholders like `{url}`, `{key}`, `{title}`), `submit`, `auto_accept_trust_prompt`, `trust_prompt_pattern`.
- `launch::Launch` types the command once the shell is quiet, waits until the agent is in the foreground and the screen is quiet, answers the trust prompt, then pastes the prompt. Readiness is a heuristic.
- The trust prompt may highlight "No" by default (Claude Code does), so `answer_keys` moves the selection to the "yes" option before pressing Enter.

**Settings (`config.rs`, `settings.rs`)**
- Tabs: Worktrees, Agents, Issues, TUI. Every change is saved to `config.json` at once. File: `$XDG_CONFIG_HOME/cornercase/config.json`, or next to the socket when `CORNERCASE_SOCKET` is set (so tests never touch the real one). Missing keys take defaults; a corrupt file means all defaults.

**Terminals (`term.rs`, `emulator.rs`)**
- The emulator is libghostty-vt: it answers terminal queries (DSR, DA, DECRQM, kitty keyboard…) that programs like fzf and nvim wait for, and reflows on resize.
- Its types are `!Send`, so the emulator lives on the server's main thread; the reader thread only forwards bytes. Query replies go out from `on_pty_write`, ordered with the output that asked.
- Cells keep palette indices so the outer theme applies. The client asks the outer terminal for its colours (OSC 10/11/4, then DA1 as an end marker) before starting the input thread, and every emulator uses them as defaults.
- cwd, name and arguments of the PTY's foreground process group leader come from `process.rs`: `/proc` on Linux, `proc_pidinfo` / `proc_name` / `sysctl(KERN_PROCARGS2)` on macOS (the only `unsafe` and the only use of `libc`). In a pipeline the leader may be dead (`process::alive`), so the shell is used instead. Agent launch and detection depend on it.

**Keys and mouse (`keys.rs`, `mouse.rs`)**
- Special keys, and all keys once the program enabled kitty, go through Ghostty's encoder; a legacy encoder handles the rest. The client asks the outer terminal for kitty's disambiguate level, so `Ctrl+Enter` / `Shift+Enter` keep their modifiers.
- Pane mouse events are forwarded only when the program enabled a mouse mode, in its encoding, relative to the pane, clamped to its edges.
- **Selection:** when the program did not ask for the mouse, press-drag-release selects with Ghostty's selection and copies via OSC 52, with a toast. Programs' own OSC 52 writes are forwarded to the outer terminal too (reads and primary-clipboard writes are dropped).

**Client / server (`client.rs`, `server.rs`, `protocol.rs`)**
- The server owns the PTYs so shells outlive the UI. The client starts it if needed (`cornercase server`, logging to `server.log` next to the socket).
- One server per socket, guarded by `flock` on `server.lock`. The server calls `setsid` and ignores SIGHUP.
- The server renders, the client only writes frames. Input travels as serialized crossterm events. Several clients mirror each other; the shared size is the last used client's (attach, key, paste or mouse other than a bare move), like tmux's `window-size latest`, so a hung client (a phone whose SSH dropped) never shrinks a new one. Ping/pong would not catch that: the hung client is a healthy local process. Smaller clients get the frame cropped by `CropBackend`.
- Every shell gets `CORNERCASE=1`; a client seeing it refuses to start (no nesting).
- `Hello` carries a protocol version and build id; a server from another build rejects the client. Keep `ClientMessage::KillServer` and `ServerMessage::Rejected` as the first variants (`protocol::tests::compatibility`).
- Socket: `$XDG_RUNTIME_DIR/cornercase/server.sock` or `$TMPDIR/cornercase-<uid>/server.sock`; `CORNERCASE_SOCKET` overrides it. Paths must fit in 108 bytes.

**Updates (`update.rs`)**
- Release builds only (`debug_assertions` off), so `cargo run` and tests never call GitHub. The server asks `releases/latest` once a day (`check_updates` in config); a newer one shows ` ↑ x.y.z ` at the end of the settings row and a toast. The dialog shows the `## Release Notes` part of the release body (from `CHANGELOG.md`), scrolled by wheel or ↑/↓.
- Updating downloads `cornercase-<target>.tar.gz` and its `.sha256`, unpacks with `tar` next to the binary, runs `--version` on it (a binary that cannot run here never replaces a working one), then renames it over the old one. Homebrew installs (`/Cellar/`…), a folder we cannot write and unknown platforms get a command to copy instead.
- The running server keeps the old code. ` restart now ` saves the session, sends `ServerMessage::Restart(path)`, and each client `exec`s the new binary, which starts a new server that restores the session. A new client rejected by an older server asks `[y/N]` on the plain terminal before running `kill-server`.

**Saved session (`state.rs`)**
- Projects, workspaces, tabs, panes (cwd), split layouts, custom names, active children and column widths, in `$XDG_STATE_HOME/cornercase/session.json` (or next to the socket). Processes are not restored; each pane gets a new shell in its folder.
- Saved only once the state is stable for 2 s, so the burst of `Exited` events at logout does not save an empty session.
- Only a fresh server restores, on its first client's `Hello`. Missing folders are skipped. `VERSION` is 3; older versions are migrated.

## Tests

- Unit tests sit next to the code in `mod <unit_of_work>` with sentence-like names; tabular cases use `rstest` with named `#[case::...]`.
- UI: render a `View` into `TestBackend`; `insta` snapshots for layout, cell styles for hover.
- `term.rs` / `app.rs` tests spawn real `/bin/sh` PTYs (never the user's shell) and wait with `test_util::wait_until`, never sleeps. `/bin/sh` is `bash` on macOS, so tests check its name with `test_util::is_sh`. `TempDir` paths are canonical, because macOS' temp dir is behind a symlink (`/var` → `/private/var`).
- Helpers: `test_util::TempDir`, `git_repo`, `fake_gh`, `FakeHttp` (canned HTTP), `write_executable` (through a `/bin/sh` child to avoid `ETXTBSY`). Nothing calls real `gh`, Shortcut or Linear. App tests clear `App::env_tokens` and never use the real config.
- Agents are faked with a script (`FAKE_AGENT`) that asks a trust question and echoes what it reads.
- `tests/e2e.rs` runs the real binary in a PTY (`SHELL=/bin/sh`, `PS1='$ '`), parses output with `vt100`, sends raw bytes and SGR mouse sequences, and answers the startup colour query. Each test gets its own server through a `Session`; dropping it runs `kill-server`.
- Avoid races in e2e: wait for output that proves the previous step finished (`echo cat-""starts; cat -v`).
- A safety-net test must fail without the code it protects.

### Manual check in a real terminal

```sh
T() { tmux -L cctest "$@"; }
T new-session -d -s t -x 100 -y 20 ./target/debug/cornercase
T send-keys -t t 'echo hi' Enter
T send-keys -t t -l $'\e[<0;6;5M'         # mouse press at col 6, row 5 (1-based)
T capture-pane -p -t t
T kill-server
```

## Website (`site/`)

Astro + Starlight, deployed to GitHub Pages by `.github/workflows/pages.yml` (Pages source: GitHub Actions). `cd site && npm ci && npm run dev`; `npm run check` and `npm run build` must pass.

- **Keep it in sync with the app.** When a feature, setting, `config.json` key, message, path or click changes, update in the same change: the docs pages that describe it (`src/content/docs/docs/`, search them for the old wording), the landing page if it shows it, the simulation if the UI changed, and the screens it makes stale.
- The landing page (`src/pages/index.astro`, `src/components/landing/`) is custom; the documentation is Starlight content in `src/content/docs/docs/`. Internal doc links are relative with a trailing slash, so the site works under any base path.
- The terminal on the landing page is a simulation in TypeScript (`src/lib/demo/`) that mirrors `ui.rs`: same layout, labels and colours, with fake shells, agents and issues. When `ui.rs` changes, update the simulation too. The same code renders the feature pictures to SVG at build time (`scenes.ts`).
- Docs screenshots are real: `src/screens/*.ansi` are `tmux capture-pane -e -p -N` dumps of the app, run with a fake `HOME` and its own `XDG_RUNTIME_DIR` (a separate server that still shows the default paths), rendered to SVG at build time by `src/lib/term/`. Box-drawing, block and a few symbol characters are drawn as shapes, not font glyphs, so lines join. Re-capture them when the UI changes.
- Base URL and origin come from `actions/configure-pages` (`SITE_BASE`, `SITE_ORIGIN`). The site lives on the custom domain `usecornercase.dev` at `/` (DNS in DigitalOcean: GitHub Pages A/AAAA records on the apex, `www` CNAME to `usecornercase.github.io`); the old github.io address redirects there. Local builds default to the same origin and base.

## Known limitations

- No scrollback navigation. Shells do not survive the server: after `kill-server` or a reboot, panes come back as new shells.
- Host colours are read once; a theme switch is not seen.
- Inner programs never get key releases.
- Runs on Linux and macOS only.
