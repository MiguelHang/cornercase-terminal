# Changelog

Every pull request that changes the app adds a section here for its new version. The section becomes the notes of the GitHub Release and shows up in the app's update dialog, so write it for users.

## 0.1.8

- Find out when Claude Code needs you or finishes in a tab you aren't looking at: a toast says where (`claude needs you in shop › main`), and your terminal shows a desktop notification, so you hear about it from another window too. It works over SSH, because the notification travels through your terminal.
- cornercase asks your terminal its name and sends the notification it understands: Ghostty, iTerm2, kitty, WezTerm, foot, Konsole, Warp, Rio, Contour and VS Code get a real one, other terminals a bell. Choose another kind or turn them off in settings → TUI → desktop notifications.

## 0.1.7

- Starting cornercase from inside Claude Code no longer passes that Claude session's variables to every pane, so a `claude` started in a pane is a session of its own again (it saves its prompt history, for instance) and the other session's messaging token stays out of your shells.

## 0.1.6

- See what Claude Code is doing without opening its tab: a tab running Claude shows `◐` while it works, `!` when it needs you (a permission or a question), `✓` when it finished while you were looking at something else, and `○` while it waits for your next message.
- `!` and `✓` also show at the end of the tab's workspace and project rows, and on the header of a collapsed group, so an agent waiting in another project doesn't go unnoticed. On a small screen, the `≡` button shows them. Opening the tab clears `✓`.
- Nothing to install or configure: cornercase reads the status Claude Code keeps for each of its running sessions.

## 0.1.5

- See what changed without leaving cornercase: in a git workspace, click ` changes ` (next to ` issues `, or ` ± ` on a small screen) to open a panel on the right with the diff of every changed file, new files included. It updates by itself while an agent works.
- Three tabs: uncommitted (what is not in a commit yet), commits (what your branch committed since it left the default branch) and all (both, like the pull request will look). Click `vs main ▾` to compare with another branch; cornercase remembers it for that workspace.
- Changed words stand out inside each line, code keeps its syntax colours, and lockfiles, binary and huge files start folded. Click a file to fold or unfold it, mark it as viewed with ✓, or click "N unchanged lines" to see more of the code around a change.
- Hover a block of changes to open it in your editor at that line, send its file and lines to the agent running in the workspace, or copy it.

## 0.1.4

- Project, workspace, tab and group rows get a subtle background while the mouse is over them, so you can see what a click will hit.

## 0.1.3

- Group your projects: `+ new project` → new group creates a group with a name, an icon and a colour. Right-click a project to move it into a group, and click a group's header to collapse or expand it. Right-click the header to rename it, change its icon and colour, or delete it (its projects stay open).
- Search also finds groups; picking one expands it and opens its first project.
- `+ new project` now opens a small menu: open project or new group.

## 0.1.2

- With several terminals attached, cornercase now takes the size of the one you used last (attached, typed or clicked in) instead of the smallest. Attaching from a computer after a phone whose connection hung no longer leaves the session at the phone's size. A smaller terminal shows the session cut off at its edges.

## 0.1.1

- A workspace whose branch is behind its upstream shows the commits to pull at the end of its row, such as `↓3`. cornercase fetches each project's remotes in the background every 5 minutes, and never pulls for you; change how often, or turn it off, in settings → Worktrees → fetch branches every.

## 0.1.0

- First release with prebuilt binaries for Linux and macOS: install with `curl -fsSL https://usecornercase.dev/install.sh | sh` or `brew install usecornercase/tap/cornercase`.
- cornercase checks for new versions once a day and updates itself from the sidebar; turn it off in settings → TUI.
- `cornercase --version` prints the version.
