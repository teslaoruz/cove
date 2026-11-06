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
| **Talk mode models** | A hands-free conversation: words show as you speak, and every pause sends | ✓ | ✓ (new) | [Talk mode](#talk-mode-live-speech-to-text) |
| **Keyboard shortcut** | One key that brings Cove up and starts listening | ✓ | ✓ set up by the installer | [Shortcuts](#keyboard-shortcuts) |
| **More agents** | Gemini CLI, Copilot CLI, Cursor CLI, Qwen Code, Aider, Goose | ✓ | ✓ | [Agents](#coding-agents) |
| **Ollama** | Run your agent on a local model, offline and free | ✓ | ✓ | [Local models](#local-models-ollama) |
| **Gmail and Calendar connectors** | Unread mail and upcoming events on the home screen | ✓ | ✓ | [Inbox](#inbox) |
| **Obsidian vault** | Each project's description and open to-dos | ✓ | ✓ | [Notes](#project-notes-obsidian) |
| **A tiling compositor** | Jump straight to the terminal of a running agent | niri, Hyprland, sway | — | [Window jump](#jump-to-an-agents-window) |

## Add-on guides

### Coding agents

Cove works with these agents. Install any of them, and they appear in the picker at the bottom of the sidebar, each with its own colour.

| Agent | Install | Cove reads its sessions | Chat in Cove |
| --- | --- | --- | --- |
| <img src="assets/agents/d97757.svg" alt=""> **Claude Code** | [docs.claude.com](https://docs.claude.com/en/docs/claude-code) | ✓ | ✓ |
| <img src="assets/agents/10a37f.svg" alt=""> **Codex** | `npm install -g @openai/codex`, then `codex login` | ✓ | ✓ |
| <img src="assets/agents/e8b33b.svg" alt=""> **opencode** | [opencode.ai](https://opencode.ai) | ✓ | ✓ |
| <img src="assets/agents/4285f4.svg" alt=""> **Gemini CLI** | `npm install -g @google/gemini-cli` ([guide](https://github.com/google-gemini/gemini-cli)) | — | — |
| <img src="assets/agents/8957e5.svg" alt=""> **GitHub Copilot CLI** | `npm install -g @github/copilot` ([guide](https://github.com/github/copilot-cli)) | — | — |
| <img src="assets/agents/2fc6d8.svg" alt=""> **Cursor CLI** | [cursor.com/cli](https://cursor.com/cli) | — | — |
| <img src="assets/agents/615ced.svg" alt=""> **Qwen Code** | `npm install -g @qwen-code/qwen-code` ([guide](https://github.com/QwenLM/qwen-code)) | — | — |
| <img src="assets/agents/e5484d.svg" alt=""> **Aider** | [aider.chat](https://aider.chat/docs/install.html) | — | — |
| <img src="assets/agents/d94fa8.svg" alt=""> **Goose** | [block.github.io/goose](https://block.github.io/goose/docs/getting-started/installation) | — | — |

All agents open, continue and resume from Cove and show up as running agents. Session counts, Activity and Chat need an agent whose history Cove can read: Claude Code, Codex or opencode.

### Local models (Ollama)

Run your agent on a model on your own computer instead of the cloud. It's free, private, and works offline. You need a fairly strong computer, ideally with 16 GB of memory or a good graphics card.

1. Install [Ollama](https://ollama.com/download):
   - **Linux:** `curl -fsSL https://ollama.com/install.sh | sh`
   - **Windows:** `winget install Ollama.Ollama`
2. Download a coding model, for example `ollama pull qwen3-coder`.
3. In Cove, type the model name into **Settings → Apps → Local model** and press Enter.

This works with **Claude Code**, **Codex**, **opencode** and **Aider**:
- Each new session, and Cove's Chat, then runs on that model.
- Clear the field to go back to the agent's own models.
- Ollama must be running. It starts by itself after installing.

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

Cove can read its chat replies aloud. Turn this on with **Settings → Voice and chat → Talk back**, and choose a voice there. Every reply also has a **Read aloud** button above it.

- **Windows:** this works out of the box with the built-in Windows voice.
- **Linux:** Cove uses the best voice it finds, in this order.

| Voice | Sound | How to install (Linux) |
| --- | --- | --- |
| **Kokoro** | Natural, 28 voices | Run the command below. About 330 MB to download. |
| **Piper** | Good | Install `piper-tts` (`pipx install piper-tts`, or `piper-tts-bin` from the AUR). Put one voice (`.onnx` plus `.onnx.json`) from [piper-voices](https://huggingface.co/rhasspy/piper-voices/tree/main/en) into `~/.local/share/piper/`. |
| **espeak-ng** | Robotic, tiny | `sudo apt install espeak-ng`, `sudo dnf install espeak-ng` or `sudo pacman -S espeak-ng` |

To install Kokoro, download the voice model. Cove has the engine built in:

```sh
mkdir -p ~/.local/share/kokoro && cd ~/.local/share/kokoro
curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2 | tar xj
```

Then go to **Settings → Voice and chat** and click **Test voice**. Cove loads the voice only when it is about to speak, and lets it go after five quiet minutes. While it gets ready, three dots move on the microphone button.

Playback uses `pw-play`, `paplay` or `aplay`, and every desktop has at least one of them.

### Talk mode (live speech to text)

Talk mode is a spoken conversation, and it needs no extra app:
- Your words appear in the box while you speak.
- Each pause sends what you said.
- Cove reads the reply aloud, then keeps listening until you stop it.

It runs inside Cove with [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), the Parakeet speech model and the Silero voice detector. Nothing is recorded or uploaded, and the microphone is open only while you talk.

1. Download the listening models, about 465 MB:

   **Linux**

   ```sh
   mkdir -p ~/.local/share/asr && cd ~/.local/share/asr
   curl -fL https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2 | tar xj
   curl -fLO https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
   ```

   **Windows** (PowerShell, Windows 10 and 11 include `tar`)

   ```powershell
   $d = "$env:APPDATA\asr"; New-Item -ItemType Directory -Force $d | Out-Null; cd $d
   curl.exe -fL -o m.tar.bz2 https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2; tar xf m.tar.bz2; del m.tar.bz2
   curl.exe -fLO https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx
   ```

2. Restart Cove. The microphone now starts talk mode instead of Handy.

Cove loads the speech model in the background when it starts, so it is ready when you are. If you don't talk for a few minutes, Cove lets it go to free about 1 GB of memory, and loads it again the next time you press the microphone. While it loads, the box glows softly and three dots move on the microphone button. When it turns red, speak.

To check talk mode from a terminal, run `cove --talk-test` and say something: it prints what it hears for 20 seconds. `cove --mic-test` shows only the microphone level.

### Keyboard shortcuts

Inside Cove:

| Key | Action |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>K</kbd> or <kbd>/</kbd> | Search or ask |
| <kbd>Ctrl</kbd>+<kbd>1</kbd> … <kbd>4</kbd> | Home, Chat, Activity, Projects |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | Settings |
| <kbd>Ctrl</kbd>+<kbd>=</kbd> / <kbd>-</kbd> / <kbd>0</kbd>, or <kbd>Ctrl</kbd>+wheel | Zoom in, out, back to 100 % |
| <kbd>Enter</kbd> | Send, or open the selected project |
| <kbd>↑</kbd> / <kbd>↓</kbd> | Pick a suggestion |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | Show or hide the sidebar (drag its edge to resize it) |
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

**Windows.** The installer adds two Start menu shortcuts with keys that work anywhere in Windows:

| Key | Action |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>F9</kbd> | Open Cove, or bring it to the front |
| <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>F10</kbd> | Bring up Cove; with Cove in front, start listening; press again to stop |

To change a key:
1. Open the Start menu and find **Cove** or **Cove - Listen**.
2. Right-click it, then choose **More → Open file location**.
3. Right-click the shortcut, choose **Properties**, click in **Shortcut key**, and press the new keys.

Updates keep the key you chose.

To use a single key such as the **Copilot key**, install [PowerToys](https://learn.microsoft.com/windows/powertoys/install). In **Keyboard Manager → Remap a shortcut**, map that key to <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>F10</kbd>.

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
- **Your agent, your data.** Switch between Claude Code, Codex, opencode, Gemini CLI, Copilot CLI, Cursor CLI, Qwen Code, Aider and Goose at the bottom of the sidebar. Each has its own colour, and local models through Ollama work too. Session counts, recent sessions, Activity, running agents and Chat all follow the agent you pick.
- **Live agents.** See which agents are running right now, in which project, and whether they are working or waiting for you.
- **Ask in plain words.** Type requests such as *"continue nebula-api"*, *"run quill"* or *"new project called atlas"*. Cove opens the right place, with your request already typed.
- **Code and Chat.** The box on Home is for your projects: open or continue one, ask how one is going, or start a new one. **Chat** (<kbd>Ctrl</kbd>+<kbd>2</kbd>) is for longer conversations that aren't about a project, like a chatbot. Replies appear as they are written, with the agent, model and time above each one.
- **Chat with any agent.** Pick the chat agent in Settings or with the button above the chat. When an agent hits its limit, hand the conversation to another one and carry on. **Open in terminal** continues a chat as a full agent session, and **Make it a project** turns it into a project.
- **Activity.** A heatmap, a punch card and charts of your prompts and tokens over time, plus your busiest projects.
- **Quick jumps.** Open any project in your editor, terminal, file manager or on GitHub, or copy its path.
- **Native and light.** One binary written in Rust, with no runtime to install. It opens in under a second and stays quiet in the background.
- **Fits your desktop.** It follows the system light or dark theme and accent colour, zooms like a browser, and works from the keyboard (see [Keyboard shortcuts](#keyboard-shortcuts)).

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
Remove-Item -Recurse "$env:LOCALAPPDATA\Programs\Cove", "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Cove*.lnk"
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

Cove lists the folders directly inside each project folder. Check **Settings → Project folders**, or, on Linux, run `cove --list` in a terminal to see what Cove finds. Projects that you started agents in from somewhere else also appear, under *Other*.

</details>

<details>
<summary><b>"cove" is not found in my terminal.</b></summary>

<br>

- On Windows, open a new terminal so it picks up the updated `PATH`.
- On Linux, add `~/.local/bin` to your `PATH`.

</details>

<details>
<summary><b>Does it work with the Claude, Codex or opencode desktop apps?</b></summary>

<br>

Partly. Cove reads each agent's history from where the agent keeps it: `~/.claude/projects`, `~/.codex` and opencode's `opencode.db`. Sessions stored there appear in Cove, so a desktop app that keeps its history in the same place shows up too. But Cove's buttons open the command-line agents in a terminal, not the desktop apps, so install the command-line version too (see [Coding agents](#coding-agents)).

</details>

<details>
<summary><b>Does it work on macOS?</b></summary>

<br>

Not yet. The code has macOS paths in place, but there is no tested build. Contributions are welcome.

</details>

## Known limits

- **Jump to an agent's window** works on niri, Hyprland and sway. Elsewhere, Cove shows running agents but can't bring their window to the front.
- **Talk mode on Windows is new and untested.** Spoken replies on Windows use the built-in Windows voice, not Kokoro.
- **Linux touchpad pinch** doesn't zoom (the window toolkit doesn't report it); use <kbd>Ctrl</kbd>+wheel or <kbd>Ctrl</kbd>+<kbd>=</kbd>.
- **Glass** (see-through window) uses Mica on Windows. On Linux, it needs a compositor that blurs, such as KDE Plasma, Hyprland or niri.
- **Chat extras are for Claude Code.** Renaming a chat and turning a chat into a project work with Claude Code. With opencode you can also delete chats. Chat needs Claude Code, Codex or opencode.
- **Not yet tried on a real machine:**
  - The flags for Gemini CLI, Copilot CLI, Cursor CLI, Qwen Code, Aider and Goose come from their docs.
  - Local models through Ollama.
  - The Windows shortcut keys, and Windows self-update.
  - The rule that keeps opencode chat to web tools only.
  
  Please [open an issue](https://github.com/teslaoruz/cove/issues) if one misbehaves.
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
