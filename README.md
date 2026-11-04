# Project Hub

A desktop launcher for your code projects. It finds every project on your disk, shows the coding-agent sessions for each one, and opens a project with Claude Code, Codex or opencode in one click.

Built in Rust with [Slint](https://slint.dev). Runs on Linux and Windows.

## What it does

- **Projects.** Lists every folder inside the folders you choose. Each card shows the session count, when you last used it, and the git branch with its uncommitted changes.
- **One-click start.** *Open* starts a new agent session in a terminal. *Continue* picks up the last session. *Sessions* lists past sessions so you can resume any of them.
- **Live agents.** Shows which agents are running right now, in which project, and whether they are working or idle.
- **Search and ask.** Type to filter projects. You can also type a plain request, such as "progress on myapp" or "new project called foo", and the hub opens the right place.
- **Chat.** A chat with Claude Code that is not tied to a project. Turn it into a project later with one click.
- **Activity.** Heatmaps and charts of your agent use, plus your GitHub contributions if `gh` is logged in.
- **Shortcuts.** Open a project in your editor, terminal, file manager or on GitHub, or copy its path.
- **Notes (optional).** If you keep project notes in an Obsidian vault, the hub shows each project's description and open to-dos.
- Follows the system light or dark theme and accent colour. Works from the keyboard.

## Download

Get the latest build from the [Releases page](https://github.com/teslaoruz/project-hub/releases/latest).

### Windows

1. Download `project-hub-windows-x86_64.zip`.
2. Unzip it anywhere, for example to `C:\Program Files\Project Hub` or your Desktop.
3. Run `project-hub.exe`.

The build is not code-signed. SmartScreen may show "Windows protected your PC". If it does, click **More info**, then **Run anyway**.

### Linux

1. Download `project-hub-linux-x86_64.tar.gz`.
2. Unpack it and run the installer:

```sh
tar xzf project-hub-linux-x86_64.tar.gz
cd project-hub
./install.sh
```

The installer puts the binary in `~/.local/bin`, and adds a menu entry and an icon in `~/.local/share`. It needs no root access. To run the app without installing it, start `./project-hub` from the unpacked folder.

The binary needs glibc 2.35 or newer, which covers Ubuntu 22.04 and later, Debian 12, Fedora 36 and later, and Arch. It also needs a working OpenGL driver and ALSA or PipeWire, which almost every desktop already has.

To uninstall:

```sh
rm ~/.local/bin/project-hub ~/.local/share/applications/project-hub.desktop ~/.local/share/icons/hicolor/scalable/apps/project-hub.svg
```

## What you need

- At least one coding agent on your `PATH`:
  - [Claude Code](https://docs.claude.com/en/docs/claude-code) (`claude`), the default.
  - [Codex](https://github.com/openai/codex) (`codex`).
  - [opencode](https://opencode.ai) (`opencode`).
- A terminal. On Linux, the hub finds foot, kitty, Alacritty, WezTerm, Ghostty, Konsole, GNOME Terminal, Ptyxis or xterm. On Windows, it uses Windows Terminal if installed, and `cmd` if not.
- Optional:
  - `git`, for the branch and changes on each card.
  - [`gh`](https://cli.github.com) (logged in), for the GitHub chart.
  - An editor such as VS Code, VSCodium, Cursor or Zed.

## First run

1. Open **Settings**.
2. Under the scan folders, add the folders that hold your projects, for example `~/Projects` or `C:\Users\you\source\repos`. Give each one a collection name. The names become the groups in the sidebar.
3. Choose your agent, terminal and editor.

On first start, the hub guesses common folders such as `~/Projects`, `~/code`, `~/dev` and `~/source/repos`. Check that the guess is right.

To see what the hub finds without opening the window, run this in a terminal:

```sh
project-hub --list
```

## Settings and data

Settings are saved in `settings.json` in this folder:

| OS | Folder |
| --- | --- |
| Linux | `~/.config/project-hub/` |
| Windows | `%APPDATA%\project-hub\` |

The hub reads sessions from Claude Code's folder, `~/.claude/projects/`. If `CLAUDE_CONFIG_DIR` is set, it reads that folder instead. The hub reads your project sessions but does not change or upload them. Only its own chats can be renamed or deleted from the hub. It starts the agents with your own login.

## Build from source

You need [Rust](https://rustup.rs) (stable).

**Linux.** Install the build libraries first. For Debian or Ubuntu:

```sh
sudo apt install build-essential pkg-config libasound2-dev libfontconfig1-dev libxkbcommon-dev libwayland-dev
```

On Arch: `sudo pacman -S base-devel alsa-lib fontconfig libxkbcommon wayland`. On Fedora: `sudo dnf install alsa-lib-devel fontconfig-devel libxkbcommon-devel wayland-devel`.

**Windows.** Install Rust with the MSVC toolchain. The installer offers the Visual Studio Build Tools if they are missing.

Then build and run:

```sh
git clone https://github.com/teslaoruz/project-hub
cd project-hub/app
cargo build --release
./target/release/project-hub
```

On Linux, run `dist/install.sh` from the same folder as the binary to install it. Run `cargo test` to run the tests.

## Known limits

Most of the app works the same on Linux and Windows. Some extras depend on tools that only some Linux setups have:

- **Jump to an agent's window** works on the niri compositor. On other desktops, the hub shows the agent but cannot focus its window.
- **Voice input** uses [Handy](https://handy.computer). **Spoken replies** use sherpa-onnx with a Kokoro model in `~/.local/share/kokoro`, or Piper or espeak-ng if those are installed. Without these tools, the voice features stay off.
- **Glass (blur behind the window)** needs a compositor that blurs, such as niri, KDE Plasma or Hyprland. On Windows, it uses Mica.
- **Single window.** On Linux, starting the hub a second time brings the open window to the front. On Windows, each start opens a new window.
- **Windows has had little testing.** Every change is built for Windows, but please [open an issue](https://github.com/teslaoruz/project-hub/issues) if something does not work.
- **macOS** has not been built or tested.

## License

[MIT](LICENSE)
