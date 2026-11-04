<div align="center">

<img src="assets/cove.svg" width="96" alt="Cove logo">

# Cove

**Every project you code, one click from a coding agent.**

Cove finds the projects on your disk and remembers every Claude Code, Codex and opencode session in them.<br>
It opens any project, new session or old one, in a single click.

[![Latest release](https://img.shields.io/github/v/release/teslaoruz/cove?style=flat-square&color=7aa2ff)](https://github.com/teslaoruz/cove/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/teslaoruz/cove/release.yml?branch=main&style=flat-square&label=build)](https://github.com/teslaoruz/cove/actions/workflows/release.yml)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows-e5603a?style=flat-square)](#install)
[![License: MIT](https://img.shields.io/badge/license-MIT-3fb950?style=flat-square)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust%20%2B%20Slint-orange?style=flat-square)](https://slint.dev)

[Install](#install) · [Setup](#setup) · [Add-ons](#what-to-install) · [Features](#features) · [Updates](#updates) · [FAQ](#faq) · [Build from source](#build-from-source)

<br>

<img src="assets/screenshot.png" alt="Cove home screen with projects, recent sessions and system widgets" width="900">

</div>

<br>

## Install

One command installs Cove for your user account. You don't need admin or root rights.

**Windows**: open **PowerShell** and run:

```powershell
irm https://github.com/teslaoruz/cove/releases/latest/download/install.ps1 | iex
```

**Linux**: open a terminal and run:

```sh
curl -fsSL https://github.com/teslaoruz/cove/releases/latest/download/install.sh | sh
```

Then start **Cove** from the Start menu or your app menu, or run `cove` in a terminal.

<details>
<summary><b>What the installer does</b></summary>

<br>

1. It downloads the newest release from this repository's [Releases page](https://github.com/teslaoruz/cove/releases).
2. It checks the download against the release's `SHA256SUMS`. If the checksum doesn't match, it stops and installs nothing.
3. It installs Cove for your user only:

| | Windows | Linux |
| --- | --- | --- |
| App | `%LOCALAPPDATA%\Programs\Cove\cove.exe` | `~/.local/bin/cove` |
| Menu entry | Start menu shortcut *Cove* | `~/.local/share/applications/cove.desktop` |
| On `PATH` | Added to your user `PATH` | `~/.local/bin` (most distros include it) |

You can read both scripts before you run them: [`install.ps1`](install.ps1) and [`install.sh`](install.sh).

To install a specific version, set `COVE_VERSION` first. On Linux:

```sh
curl -fsSL https://github.com/teslaoruz/cove/releases/latest/download/install.sh | COVE_VERSION=v0.2.0 sh
```

On Windows, run `$env:COVE_VERSION = 'v0.2.0'` before the install command.

</details>

<details>
<summary><b>Windows: using cmd instead of PowerShell</b></summary>

<br>

```bat
powershell -ExecutionPolicy Bypass -c "irm https://github.com/teslaoruz/cove/releases/latest/download/install.ps1 | iex"
```

</details>

**Requirements**

- 64-bit **Windows 10 or 11**, or 64-bit **Linux** with glibc 2.35 or newer. That covers Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch and openSUSE Tumbleweed.
- At least one coding agent: [Claude Code](https://docs.claude.com/en/docs/claude-code), [Codex](https://github.com/openai/codex) or [opencode](https://opencode.ai). Setup step 1 shows how to install one.

## Setup

Setup takes about two minutes the first time.

### 1. Install a coding agent (skip this if you already have one)

Cove starts your agent for you, so you need at least one installed and signed in. For Claude Code:

| Windows (PowerShell) | Linux |
| --- | --- |
| `irm https://claude.ai/install.ps1 \| iex` | `curl -fsSL https://claude.ai/install.sh \| bash` |

Then run `claude` once in a terminal and sign in. Cove uses that same login. For Codex or opencode, follow their own install guides.

### 2. Install Cove

Run the one-line installer from [Install](#install).

### 3. Tell Cove where your projects live

Open Cove, then open **Settings** (bottom left) and go to **Project folders**.

- Add each folder that holds your projects, for example `C:\Users\you\source\repos` or `~/Projects`.
- Give each folder a **collection** name such as *Work*, *Personal* or *University*. Collections become groups in the sidebar.
- Each folder directly inside a project folder becomes a project card.

On first start, Cove looks for common folders by itself: `~/Projects`, `~/code`, `~/dev`, `~/src`, `~/repos` and `~/source/repos`. If yours is among them, there is nothing to add.

### 4. Pick your tools

Under **Settings → Apps**, choose:

| Setting | What it does |
| --- | --- |
| **Coding agent** | Claude Code, Codex or opencode. You can switch any time from the bottom of the sidebar. |
| **Terminal** | Where sessions open. Windows uses Windows Terminal if you have it, and `cmd` if not. On Linux, Cove finds foot, kitty, Alacritty, WezTerm, Ghostty, Konsole, GNOME Terminal, Ptyxis and xterm. |
| **Code editor** | VS Code, VSCodium, Cursor, Zed, Sublime Text, Neovim or IntelliJ IDEA. |
| **Browser / File manager** | Leave them on *System default*, or pick one. |
| **Updates** | On by default. See [Updates](#updates). |
| **Inbox** | Off by default. Turn it on to see unread mail and upcoming events on the home screen. |

### 5. Open a project

Click a project and choose:

- **Open** to start a new session in that folder.
- **Continue** to pick up the last session.
- **Sessions** to resume any earlier one.

You can also type a project's name in the box and press Enter.

### 6. Add what you like

Voice, a keyboard shortcut, your inbox and more are optional. See [What to install](#what-to-install).

## What to install

Cove needs only one thing besides itself: a coding agent. Everything else is an add-on. Cove notices each one by itself and turns on the matching feature, so install only what you want.

### Required

| What | Why | Linux | Windows |
| --- | --- | --- | --- |
| **Cove** | The app | [One-line installer](#install) | [One-line installer](#install) |
| **One coding agent** | Cove opens your projects in it | [Claude Code](https://docs.claude.com/en/docs/claude-code), [Codex](https://github.com/openai/codex) or [opencode](https://opencode.ai). See [Setup, step 1](#1-install-a-coding-agent-skip-this-if-you-already-have-one). | Same |
| **A terminal** | Sessions open in it | Almost every desktop has one. Cove finds foot, kitty, Alacritty, WezTerm, Ghostty, Konsole, GNOME Terminal, Ptyxis and xterm. | Built in. [Windows Terminal](https://aka.ms/terminal) is used if you have it. |

### Optional add-ons

| Add-on | What you get | Linux | Windows | Guide |
| --- | --- | --- | --- | --- |
| **Git** | Branch and uncommitted changes on every project card | ✓ | ✓ | [Git](#git-and-github) |
| **GitHub CLI** | Your GitHub contributions in Activity, plus the GitHub button | ✓ | ✓ | [GitHub](#git-and-github) |
| **Code editor** | The *VS Code* button opens projects in your editor | ✓ | ✓ | [Editor](#code-editor) |
| **Handy** | Dictation: speak, and the words appear in Cove's box | ✓ | ✓ | [Voice input](#voice-input-handy) |
| **A voice** | Cove reads its chat replies aloud | ✓ (Kokoro, Piper or espeak-ng) | ✓ built in | [Spoken replies](#spoken-replies) |
| **Talk mode models** | A hands-free conversation: words show as you speak, and every pause sends | ✓ | — | [Talk mode](#talk-mode-live-speech-to-text) |
| **Keyboard shortcut** | One key that brings Cove up and starts listening | ✓ | via Handy | [Shortcuts](#keyboard-shortcuts) |
| **Gmail and Calendar connectors** | Unread mail and upcoming events on the home screen | ✓ | ✓ | [Inbox](#inbox) |
| **Obsidian vault** | Each project's description and open to-dos | ✓ | ✓ | [Notes](#project-notes-obsidian) |
| **A tiling compositor** | Jump straight to the terminal of a running agent | niri, Hyprland, sway | — | [Window jump](#jump-to-an-agents-window) |

## Add-on guides

### Git and GitHub

- **Git** shows each project's branch and changes.
  - Linux: `sudo apt install git`, `sudo dnf install git` or `sudo pacman -S git`.
  - Windows: `winget install Git.Git`.
- **GitHub CLI** adds your contribution heatmap to Activity.
  - Install it: [cli.github.com](https://cli.github.com). On Windows, run `winget install GitHub.cli`.
  - Then sign in once: `gh auth login`.

### Code editor

Install the editor you like, and make sure its command works in a terminal: `code`, `codium`, `cursor`, `zed`, `subl`, `nvim` or `idea`. Then pick it under **Settings → Apps → Code editor**.

The VS Code installers for Windows and Linux add `code` to your `PATH` by default.

### Voice input (Handy)

[Handy](https://handy.computer) is a free, open-source dictation app that runs on your computer. It types what you say into the window in front. Cove starts and stops it for you.

1. Install Handy from [handy.computer](https://handy.computer) or its [releases page](https://github.com/cjpais/Handy/releases/latest):
   - **Windows:** `Handy_…_x64-setup.exe`
   - **Linux:** the `.AppImage`, `.deb` or `.rpm`
2. Open Handy once and pick a speech model in its settings. Handy downloads the model itself.
3. **Linux on Wayland only:** Handy needs a typing helper. Install `wtype`, or install `ydotool` and keep its service running (`systemctl --user enable --now ydotool`).
4. In Cove, click the **microphone** next to the box, or use the [keyboard shortcut](#keyboard-shortcuts). Speak, then click again to stop.

To send dictated text without pressing Enter, switch on **Settings → Voice and chat → Dictation**.

### Spoken replies

In **Chat** mode, Cove can read its replies aloud. Turn this on with **Settings → Voice and chat → Talk back**, and choose a voice there.

- **Windows:** this works out of the box with the built-in Windows voice.
- **Linux:** Cove uses the best voice it finds, in this order.

| Voice | Sound | How to install (Linux) |
| --- | --- | --- |
| **Kokoro** | Natural, 28 voices | Run the commands below. About 360 MB to download. |
| **Piper** | Good | Install `piper-tts` (`pipx install piper-tts`, or `piper-tts-bin` from the AUR). Put one voice (`.onnx` plus `.onnx.json`) from [piper-voices](https://huggingface.co/rhasspy/piper-voices/tree/main/en) into `~/.local/share/piper/`. |
| **espeak-ng** | Robotic, tiny | `sudo apt install espeak-ng`, `sudo dnf install espeak-ng` or `sudo pacman -S espeak-ng` |

To install Kokoro, run:

```sh
mkdir -p ~/.local/share/kokoro && cd ~/.local/share/kokoro
curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/sherpa-onnx-v2.0.1-linux-x64-shared.tar.bz2 | tar xj
curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2 | tar xj
```

Then go to **Settings → Voice and chat** and click **Test voice**.

Playback uses `pw-play`, `paplay` or `aplay`, and every desktop has at least one of them.

### Talk mode (live speech to text)

Talk mode is a spoken conversation, and it needs no extra app:
- Your words appear in the box while you speak.
- Each pause sends what you said.
- Cove reads the reply aloud, then keeps listening until you stop it.

It runs on your computer with [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), the Parakeet speech model and the Silero voice detector. Nothing is recorded or uploaded. **Linux only for now.**

1. Install **Kokoro** first (see [Spoken replies](#spoken-replies)), because talk mode uses the same sherpa-onnx tools.
2. Download the listening models, about 465 MB:

```sh
mkdir -p ~/.local/share/asr && cd ~/.local/share/asr
curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 | tar xj
curl -fLO https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
```

3. Restart Cove. The microphone now starts talk mode instead of Handy. It takes a few seconds to get ready each time.

To check that Cove hears your microphone, run `cove --mic-test` in a terminal and speak. The numbers rise while you talk.

### Keyboard shortcuts

Inside Cove:

| Key | Action |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>K</kbd> or <kbd>/</kbd> | Search or ask |
| <kbd>Enter</kbd> | Send, or open the selected project |
| <kbd>↑</kbd> / <kbd>↓</kbd> | Pick a suggestion |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | Show or hide the sidebar |
| <kbd>Esc</kbd> | Clear, close, go back |

**A global shortcut (Linux).** Bind any key to `cove --listen`:
- If Cove isn't in front, the key brings it up.
- If Cove is in front, the key starts listening. Press it again to stop.

Use the full path, because some desktops don't search `~/.local/bin`. Replace `you` with your user name.

| Desktop | Add this |
| --- | --- |
| **niri** (`~/.config/niri/config.kdl`, inside `binds`) | `Mod+Shift+Space { spawn "/home/you/.local/bin/cove" "--listen"; }` |
| **Hyprland** (`~/.config/hypr/hyprland.conf`) | `bind = SUPER SHIFT, SPACE, exec, ~/.local/bin/cove --listen` |
| **sway** (`~/.config/sway/config`) | `bindsym $mod+Shift+space exec ~/.local/bin/cove --listen` |
| **GNOME** | Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts → **+**. Command: `/home/you/.local/bin/cove --listen` |
| **KDE Plasma** | System Settings → Keyboard → Shortcuts → **Add New** → **Command or Script**. Command: `/home/you/.local/bin/cove --listen` |

To bind a key that only brings Cove to the front, use `cove` without `--listen`.

**Windows.** A global Cove shortcut isn't supported yet. Use **Handy's own shortcut** (set it in Handy's settings). With Cove in front, Handy types straight into Cove's box.

### Inbox

The Inbox shows unread Gmail and the next week of Google Calendar events on the home screen. It works through Claude Code, so it needs:

1. Claude Code, signed in.
2. The **Gmail** and **Google Calendar** connectors, enabled at [claude.ai → Settings → Connectors](https://claude.ai/settings/connectors).
3. **Settings → Apps → Inbox** switched on in Cove.

Cove asks Claude Code at most once every 3 hours, using the small Haiku model and read-only tools. This counts toward your Claude plan's usage.

### Project notes (Obsidian)

If you keep one note per project in an [Obsidian](https://obsidian.md) vault, Cove shows each project's description and open to-dos.

1. Point **Settings → Notes vault** at the vault folder.
2. Give each project note a `path:` line in its frontmatter, with the project's folder, for example `path: "/home/you/Projects/myapp"`.

Without a vault, Cove uses the first lines of the project's `README.md`.

### Jump to an agent's window

On the **niri**, **Hyprland** and **sway** compositors, clicking a running agent focuses its terminal window. Other desktops don't let apps move focus like this. Cove still shows the agent there, just without the jump.

## Features

- **Every project in one place.** Each card shows the session count, when you last worked on the project, and its git state. Pin favourites and hide what you don't need.
- **One-click sessions.** Open a new session, continue the last one, or resume any past session from its first message.
- **Your agent, your data.** Switch between Claude Code, Codex and opencode at the bottom of the sidebar. Session counts, recent sessions, Activity, running agents and Chat all follow the agent you pick.
- **Live agents.** See which agents are running right now, in which project, and whether they are working or waiting for you.
- **Ask in plain words.** Type requests such as *"continue nebula-api"*, *"run quill"* or *"new project called atlas"*. Cove opens the right place, with your request already typed.
- **Chat.** Talk to Claude Code without picking a project first. When an idea grows, turn the chat into a project with one click.
- **Activity.** A heatmap, a punch card and charts of your prompts and tokens over time, plus your busiest projects.
- **Quick jumps.** Open any project in your editor, terminal, file manager or on GitHub, or copy its path.
- **Native and light.** One binary written in Rust, with no runtime to install. It opens in under a second and stays quiet in the background.
- **Fits your desktop.** It follows the system light or dark theme and accent colour, and works from the keyboard: <kbd>Ctrl</kbd>+<kbd>K</kbd> to search, <kbd>Ctrl</kbd>+<kbd>B</kbd> for the sidebar, <kbd>Esc</kbd> to go back.

## Updates

Installed copies of Cove keep themselves up to date:

1. Once a day, Cove checks this repository for a newer release.
2. If one exists, Cove runs that release's installer in the background, and the installer verifies the checksum.
3. Cove tells you when the update is ready. The new version starts the next time you open Cove.

To turn this off, switch off **Settings → Apps → Updates**. To update by hand at any time, run the install command again.

Copies you build from source never update themselves.

## Settings and privacy

| | Windows | Linux |
| --- | --- | --- |
| Settings | `%APPDATA%\cove\settings.json` | `~/.config/cove/settings.json` |
| Cache | `%LOCALAPPDATA%\cove\` | `~/.cache/cove/` |

- Cove reads session history read-only: Claude Code's from `~/.claude/projects/` (or `CLAUDE_CONFIG_DIR`), opencode's from `~/.local/share/opencode/opencode.db`, and Codex's from `~/.codex/` (or `CODEX_HOME`). It never edits or deletes your project sessions. Only Cove's own chats can be renamed or deleted from inside Cove.
- Agents run with your own login. Cove has no account, no telemetry and no server of its own.
- Cove makes one network request of its own: the daily update check, which goes to GitHub. Some widgets run your own tools in the background:
  - **Inbox** asks `claude` (Haiku model, read-only Gmail and Google Calendar connectors) for unread mail and upcoming events, at most every 3 hours. It is off until you turn it on.
  - **Activity** asks `gh` for your GitHub contributions.
  - Plain-language requests that Cove can't match by itself go to `claude` once.

## Uninstall

**Windows** (PowerShell):

```powershell
Remove-Item -Recurse "$env:LOCALAPPDATA\Programs\Cove", "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Cove.lnk"
```

**Linux**:

```sh
rm ~/.local/bin/cove ~/.local/share/applications/cove.desktop ~/.local/share/icons/hicolor/scalable/apps/cove.svg
```

To also remove your settings, delete the settings folder listed above.

## FAQ

<details>
<summary><b>Windows says "Windows protected your PC".</b></summary>

<br>

Cove isn't code-signed yet, so SmartScreen may warn the first time it runs. Click **More info**, then **Run anyway**. The installer has already checked that the file matches the published release.

</details>

<details>
<summary><b>A project is missing.</b></summary>

<br>

Cove lists the folders directly inside each project folder. Check **Settings → Project folders**, or run `cove --list` in a terminal to see what Cove finds. Projects that you started agents in from somewhere else also appear, under *Other*.

</details>

<details>
<summary><b>"cove" is not found in my terminal.</b></summary>

<br>

- On Windows, open a new terminal so it picks up the updated `PATH`.
- On Linux, add `~/.local/bin` to your `PATH`.

</details>

<details>
<summary><b>Does it work on macOS?</b></summary>

<br>

Not yet. The code has macOS paths in place, but there is no tested build. Contributions are welcome.

</details>

## Known limits

- **Jump to an agent's window** works on niri, Hyprland and sway. Elsewhere, Cove shows running agents but can't bring their window to the front.
- **Talk mode and a global shortcut are Linux-only** for now. On Windows, use Handy for dictation; spoken replies use the Windows voice.
- **Glass** (see-through window) uses Mica on Windows. On Linux, it needs a compositor that blurs, such as KDE Plasma, Hyprland or niri.
- **Codex support is new.** Cove reads Codex's sessions and can chat through `codex exec`, but this has not been tested with a signed-in Codex yet.
- **Chat extras are for Claude Code.** Renaming a chat and turning a chat into a project work with Claude Code. With opencode you can delete chats; with Codex, neither yet.
- **Windows is newer than Linux.** Each release is built and installed on Windows in CI. If something doesn't feel right, please [open an issue](https://github.com/teslaoruz/cove/issues).

## Build from source

You need [Rust](https://rustup.rs) (stable).

<details>
<summary><b>Linux build libraries</b></summary>

<br>

| Distro | Command |
| --- | --- |
| Debian / Ubuntu | `sudo apt install build-essential pkg-config libasound2-dev libfontconfig1-dev libxkbcommon-dev libwayland-dev` |
| Fedora | `sudo dnf install gcc alsa-lib-devel fontconfig-devel libxkbcommon-devel wayland-devel` |
| Arch | `sudo pacman -S base-devel alsa-lib fontconfig libxkbcommon wayland` |

</details>

On Windows, install Rust with the default MSVC toolchain. Rustup offers to install the Visual Studio Build Tools if they are missing.

```sh
git clone https://github.com/teslaoruz/cove
cd cove/app
cargo build --release
cargo test
```

The app is at `target/release/cove` (`cove.exe` on Windows). On Linux, copy `dist/install.sh`, `dist/cove.desktop` and `../assets/cove.svg` next to the binary, then run `install.sh` to add it to your app menu.

## Contributing

Issues and pull requests are welcome. Every push builds and tests on Linux and Windows through GitHub Actions. A `v*` tag publishes a release, and CI then installs it on clean Linux and Windows machines.

## License

[MIT](LICENSE) © 2026 teslaoruz
