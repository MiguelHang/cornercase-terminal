# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the app's update dialog, so write it for users.

## 0.1.1

- A workspace whose branch is behind its upstream shows the commits to pull at the end of its row, such as `↓3`. cornercase fetches each project's remotes in the background every 5 minutes, and never pulls for you; change how often, or turn it off, in settings → Worktrees → fetch branches every.

## 0.1.0

- First release with prebuilt binaries for Linux and macOS: install with `curl -fsSL https://usecornercase.dev/install.sh | sh` or `brew install usecornercase/tap/cornercase`.
- cornercase checks for new versions once a day and updates itself from the sidebar; turn it off in settings → TUI.
- `cornercase --version` prints the version.
