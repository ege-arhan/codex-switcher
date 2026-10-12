<p align="center">
  <img src="src-tauri/icons/logo.svg" alt="Codex Switcher" width="128" height="128">
</p>

<h1 align="center">Codex Switcher</h1>

<p align="center">
  A Desktop Application for Managing Multiple OpenAI <a href="https://github.com/openai/codex">Codex</a> Accounts<br>
  Easily switch between accounts, monitor usage, schedule warm-ups, and stay in control of your quota
</p>

<div align="center">

[![](https://dcbadge.limes.pink/api/server/4QzyJTC3S)](https://discord.gg/4QzyJTC3S)

</div>

## Features

- **Multi-Account Management** – Add, rename, mask, import, export, and manage multiple Codex accounts in one place
- **Quick Switching** – Switch between accounts from the main window, native tray menu, or tray popup while preserving rotated ChatGPT sessions
- **Usage Stats** – View account usage stats for OAuth accounts, including lifetime tokens, daily buckets, streaks, activity insights, and top integrations
- **Manual Reset Credits** – See available manual reset credits beside each account plan badge, with the closest expiry highlighted as it approaches
- **Automatic Warm-Up** – Warm up one account or all accounts manually, after each 5-hour reset window, or at specific scheduled times of day
- **System Tray Controls** – Use the tray popup to switch accounts, inspect quota and active-account stats, refresh usage, open the main window, or quit the app
- **Tray Display Modes** – Choose between the app icon with session percentage, a text-only hourly/weekly percentage display, or a hidden tray icon
- **macOS Dock Control** – Keep Codex Switcher in the Dock or run it as a menu bar only app, with a first-close prompt and a tray fallback
- **Rate-Limit Monitoring** – View real-time 5-hour session and weekly usage, reset timing, credits, and subscription expiry
- **Auto-Retry on Model Capacity** – Automatically queue retries when OpenAI models are overloaded, with configurable backoff delays and optional escalation to account switching
- **Session Auto-Switch** – Detect rate limit exhaustion in running sessions, auto-redeem banked reset credits in place, or rotate accounts and continue the same CLI, macOS desktop, or Linux desktop conversation
- **Smart Rotation Strategies** – Optimize quota rotation with Smart Balanced heuristics (FIFO reset credits, burn-before-reset weekly rollover, and Pro / Pro Lite tier reserves), Most Remaining Quota, or Round Robin
- **Codex `/goal` Continuity** – Detect active or interrupted goal objectives from local SQLite state and automatically resume them using `/goal resume`
- **Blocked Switch Recovery** – Detect running Codex sessions and offer a force-close flow before retrying the account switch
- **Dual Login Mode** – Authenticate with ChatGPT OAuth or import existing `auth.json` files
- **Seven Desktop Themes** – Choose macOS (default), Windows XP, Windows 7, Terminal, Paper, Nord or Mono in Settings; provider tabs keep account lists focused while preserving advanced controls
- **Interface Language** – Choose Turkish or English in Settings; the preference also applies to dates, usage labels and native menus
- **Claude Accounts (macOS)** – Keep native Code logins and Desktop profiles separate, monitor quota, warm up accounts, and resume managed Code sessions after quota exhaustion

## Installation

### Download a Release

The easiest way to install Codex Switcher is from the latest GitHub release:

[Download the latest release](https://github.com/Lampese/codex-switcher/releases/latest)

Choose the file for your platform:

- **macOS Apple Silicon:** `Codex.Switcher_*_aarch64.dmg`
- **macOS Intel:** `Codex.Switcher_*_x64.dmg`
- **Windows:** `Codex.Switcher_*_x64-setup.exe` or `Codex.Switcher_*_x64_en-US.msi`
- **Linux Debian/Ubuntu:** `Codex.Switcher_*_amd64.deb`
- **Linux AppImage:** `Codex.Switcher_*_amd64.AppImage`
- **Linux RPM:** `Codex.Switcher-*-1.x86_64.rpm`

> **macOS:** current release builds are not Apple-notarized. If macOS says the
> app is damaged, move it to `/Applications` and remove the quarantine flag:
>
> ```bash
> sudo xattr -dr com.apple.quarantine "/Applications/Codex Switcher.app"
> open "/Applications/Codex Switcher.app"
> ```

### Auto Updates

Codex Switcher checks the latest GitHub release on startup. When a newer signed
update package is available, the app shows an update prompt and can install it
from inside the app.

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/)
- [Rust](https://rustup.rs/)

```bash
# Clone the repository
git clone https://github.com/Lampese/codex-switcher.git
cd codex-switcher

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

> **Windows:** the `pnpm tauri` script runs through a POSIX shell wrapper
> (`sh ./scripts/tauri.sh`) and will not work in PowerShell/CMD. Use the
> `tauri:win` script instead: `pnpm tauri:win dev` and `pnpm tauri:win build`.

The built application will be in `src-tauri/target/release/bundle/`.

### Run the Dashboard in a Browser

You can also serve the built dashboard over HTTP instead of opening the Tauri shell.

```bash
# Build the frontend and start the web server on 0.0.0.0:3210
pnpm lan
```

Optional environment variables:

- `CODEX_SWITCHER_WEB_HOST` to override the bind host
- `CODEX_SWITCHER_WEB_PORT` to override the port

The browser dashboard serves the same UI and backend actions through `/api/invoke/*`, which makes it usable over LAN, Tailscale, or a remote host tunnel when you expose the chosen port safely.

## Usage and Reset Credits

Codex Switcher shows two kinds of account usage information:

- **Rate limits** – the account card shows the current 5-hour and weekly limit
  windows, remaining percentage, reset timing, credit balance, and subscription
  expiry when available.
- **Usage Stats** – ChatGPT OAuth accounts can expand the **Usage
  Stats** panel to view stats such as lifetime tokens,
  today, last 7 days, last 30 days, streaks, longest task, token activity,
  reasoning/activity insights, and most-used integrations. The active account
  opens this panel by default; other accounts keep it collapsed until needed.
- **Manual reset credits** – OAuth accounts with available reset credits show a
  compact badge next to the plan badge. It includes the available count and the
  closest expiry date, hides zero-count results, and turns amber within 10 days
  or red within 3 days of expiry.

The tray popup also includes compact active-account stats for today and
the last 7 days, while keeping the normal rate-limit refresh flow separate.

## Safe Account Switching

ChatGPT can replace an OAuth refresh token after using it. Once replaced, the
older token may no longer be accepted. Before Codex Switcher writes another
account to `~/.codex/auth.json`, it now saves the latest tokens from the account
that is currently active. Switching back therefore restores the current session
instead of an older snapshot.

Token refreshes and account switches are serialized so a background refresh
cannot finish late and overwrite the account you just selected. Codex Switcher
also avoids refreshing the active account while Codex or ChatGPT is running;
close the running app before switching accounts.

If an older Codex Switcher version already saved an invalid refresh token, sign
in to that account again or remove and re-add it once. An invalidated token
cannot be recovered locally.

### Automatic session recovery

Enable **Auto-switch account on usage limit reached** in Settings to let Codex
Switcher rotate to an account with available quota after a running session hits
its limit. The feature is off by default. CLI sessions restart with `codex
resume` in a terminal. On macOS and Linux, a Codex desktop conversation is continued by
closing the desktop app gracefully, changing credentials, reopening it, and
queueing the continuation in the same thread.

The desktop handoff waits while another Codex turn or CLI session is active.
When a CLI session triggers rotation while the desktop app is idle, the desktop
app also closes and reopens so it loads the new account. If other running work
prevents a graceful handoff, switching is deferred. A failed continuation after
the account change is reported in the Switcher log. Windows desktop recovery is
not supported by this feature yet.

## Claude accounts (macOS)

The **Claude accounts** section imports the current Claude Code subscription
with **Import current account**. **Add account** starts the installed Claude CLI's
native login in a separate `CLAUDE_CONFIG_DIR` and displays its sign-in link.
Complete the login with the requested email. **Copy link** lets you paste
the original Claude authorization link into any browser. Complete the Google
steps in that browser; do not copy a Google intermediate consent URL between
browsers. If automatic completion does not return to the CLI, paste the final
localhost callback URL or Claude's complete `code#state` into **Return URL or
authorization code** and choose **Complete sign-in**. The return must match the
current login; it is sent to the native CLI without logging or persisting it.
The Switcher success screen appears only after the native CLI exits successfully
and verifies the requested account. Switcher stores profile metadata;
Claude owns the credentials in Keychain or its private credential file.

**Open Claude Code** opens a managed terminal immediately in your home directory.
**Choose project folder** optionally selects and remembers a project directory;
**Home directory** clears that choice. With
**Claude Code auto-switch** enabled, only verified Pro/Max/Team/Enterprise
subscription accounts with fresh remaining quota are eligible. A native `StopFailure` rate-limit hook
checks the saved accounts' current usage and resumes the same local session
in that terminal with an available account. Five-hour, weekly and reported
model limits must all permit work; unknown or stale usage is never assumed
to be available. Expiring logins are renewed through the native CLI. If no eligible quota is
available, the terminal keeps the failed session and waits, rechecking live
usage every 30 seconds. The Claude section shows the waiting state and plan badges. Local
projects, settings and skills are shared; logins remain separate. Ordinary
`claude` processes opened outside Switcher keep their own account.

**Warm up** sends one minimal Haiku request with tools, MCP servers, hooks and
session persistence disabled. **Claude auto warm-up** checks for an unused,
reset five-hour window every minute and starts it only with fresh verified
quota. It skips active or exhausted windows, serializes requests, waits at least
five hours between successful warm-ups, and backs off after failures. Warm-up
uses a small amount of the account's subscription quota.

**Switch Desktop** closes Claude gracefully and moves the entire local desktop
profile between account directories, preserving the outgoing profile without
copying its contents. A journal supports recovery after an interrupted move.
Sign in separately in Desktop when opening a new profile for the first time.
The Code login success screen offers explicit terminal and Desktop actions;
completing a Code authorization link alone does not open or sign into Desktop.
Cards show separate Code and Desktop profile states. Desktop state describes
the cached account identity, not a guarantee that its session is still valid.
Native Desktop account identity is matched to the imported Code accounts before
moving a profile. If Desktop is signed into another account, import that account
first. Normal cloud chats stay with their original account;
automatic continuation of those chats is not supported.

Claude usage uses the native CLI's read-only OAuth usage endpoint, which is not
a public stable API. If the endpoint or native authentication format changes,
Switcher reports the failure and keeps the current account. Accounts need a
Claude Code version supporting `auth login`, `auth status` and `StopFailure`.
Claude features are local desktop-only and are not exposed by the LAN dashboard.

## macOS Dock and Menu Bar Mode

On macOS, Codex Switcher can either stay visible in the Dock or live only in the
menu bar. The first time you close the main window, the app asks which behavior
you want and lets you choose whether to show that prompt again.

You can change the same setting later from the tray popup or from the native
tray menu under **Dock Icon**. If you choose **Menu Bar Only**, the app keeps a
visible tray item so you can always reopen the main window or switch back to
Dock mode.

## Warm-Up

A warm-up sends one minimal request to an account so its current usage window
has activity before you need it.

- **Manual** – warm up a single or all accounts, from the main window or tray menu.
- **Automatic** – when enabled (per account or for all), the app tracks the
  5-hour window when available and warms it after each reset, as long as the
  weekly limit isn't exhausted. If only the weekly window is available, it
  warms once after the weekly reset and automatically returns to the 5-hour
  schedule if that window reappears.
- **Timed** – pick specific times of day (e.g. `08:00`, `13:00`, `18:00`) from
  the **Timed** control in the main window. At each time the app warms all
  accounts (skipping any whose weekly limit is exhausted), so you control when
  your 5-hour windows start instead of letting them drift.

Timed warm-up checks the schedule every 30 seconds, runs each configured minute
only once per day, and skips missed times if the machine was asleep instead of
warming accounts late.

On macOS you can keep the machine awake with the built-in `caffeinate` command,
which stops automatically when the app quits:

```bash
caffeinate -i -w "$(pgrep -x 'Codex Switcher')"
```

## Disclaimer

This tool is designed **exclusively for individuals who personally own multiple OpenAI/ChatGPT accounts**. It is intended to help users manage their own accounts more conveniently.

**This tool is NOT intended for:**

- Sharing accounts between multiple users
- Circumventing OpenAI's terms of service
- Any form of account pooling or credential sharing

By using this software, you agree that you are the rightful owner of all accounts you add to the application. The authors are not responsible for any misuse or violations of OpenAI's terms of service.

## Versioning

Use the version bump helper to keep app versions in sync across Tauri, Cargo, and the frontend.

```bash
# Exact version
pnpm version:bump 0.2.1

# Semver bumps
pnpm version:patch
pnpm version:minor
pnpm version:major

# Prepare a release commit and tag
# This prompts for a short release note and runs the version bump first.
pnpm release patch

# Prepare and push a release
# The tag stores the release note for the in-app update prompt.
pnpm release patch -- --push

# For non-interactive use, pass the note explicitly.
pnpm release patch -- --push --note "Fixed account switching issues"
```

### Paid quota and Desktop recovery

Codex automatic switching only selects Plus, Pro/Pro Lite, Team/Business,
Enterprise or Edu accounts with verified remaining quota. Free, Go, unknown
plans and exhausted accounts are excluded, even if they have banked reset
credits. Manual account switching remains available. With no eligible account,
the app shows a waiting warning and rechecks quota every minute; it can resume
on the current account once its quota actually resets.

Desktop continuations use the installed Codex Desktop's local owner IPC,
so the Desktop process retains the conversation writer. The native IPC is
versioned but not a public stable API; discovery/communication failure is
reported instead of starting a separate writer process.
