# Security policy

## Reporting a vulnerability

Please do not open a public issue. Report it privately through [GitHub's private vulnerability reporting](https://github.com/usecornercase/cornercase-terminal/security/advisories/new).

Include what you found, how to reproduce it and what an attacker could do with it. We will acknowledge the report, keep you posted while we work on a fix, and credit you in the advisory unless you prefer otherwise.

## Scope

cornercase is a local terminal multiplexer. Things we especially care about:

- the server's Unix socket and its directory (who can attach to your shells)
- Shortcut and Linear tokens (`secrets.json`, environment variables) leaking to logs, the screen or other processes
- escape sequences from programs or issue contents that reach your outer terminal, such as clipboard reads or writes through OSC 52
- commands built from issue data (branches, worktree paths, agent prompts)

Only the latest commit on `main` is supported.
