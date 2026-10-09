# Rustario64 development build

This is an early Bob-omb Battlefield exploration build. Mario's non-object
movement and the original camera (Lakitu, the R-button Mario camera, C-Up
first person) have native decomp comparison coverage. Missions, objects
(including the cannon lid), camera cutscenes, original pause behavior, water/cutscene
actions, warps, saves, gamepad controls and audio are missing. Play stops on
unsupported paths; R re-enters.

## Start

Windows and Linux ZIP builds are attached to successful runs of the
[Rust foundation workflow](https://github.com/esotericode/Rustario64/actions/workflows/rust.yml).
Choose the latest successful run for the branch under test (the camera is now on `main`; this desktop increment is on
`codex/animation-continuity-desktop`), then
download `rustario64-windows-x86_64` or `rustario64-linux-x86_64` under Artifacts.
GitHub's artifact ZIP contains the runtime ZIP; extract both layers. The build
identifier is in the runtime folder's BUILD_INFO.txt.

Extract the whole ZIP. Keep your own ROM outside this folder. Only the original
8 MiB US v1.0 ROM is supported, identified after byte-order normalization by
SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce` (Z64, V64 or N64).
The build contains no ROM or game assets; it imports your ROM locally at launch.
Open `rustario64-viewer.exe` on Windows or run `./rustario64-viewer` on Linux.
Choose Browse, enter a local path, or drag the ROM into the launcher, then select
Play. Unsupported or unreadable ROMs show an error and allow another selection.
Remembering the path is opt-in and happens after successful validation/import.
The launcher offers interpolation, fog, VSync, fullscreen and window-size choices.
Settings are stored separately from content in `%APPDATA%/rustario64/settings.json`
or `$XDG_CONFIG_HOME/rustario64/settings.json` (default `~/.config` on Linux).
Invalid settings fall back to defaults with an error. Linux Browse needs a desktop
file portal; path entry and drag/drop work without it.

The following terminal entry points still work and enable recording:

Windows (PowerShell, in the extracted folder):

```powershell
.\rustario64.exe inspect-rom "C:\Games\sm64.z64"
.\rustario64-viewer.exe view "C:\Games\sm64.z64" --mario --record private/runs
```

Linux (terminal, in the extracted folder):

```sh
./rustario64 inspect-rom "$HOME/Games/sm64.z64"
./rustario64-viewer view "$HOME/Games/sm64.z64" --mario --record private/runs
```

Linux needs a compatible system Vulkan or GL driver and X11/Wayland libraries
(for example libxkbcommon-x11-0 for X11). Windows needs a compatible GPU driver.
The current CI uses Ubuntu 24.04 and Windows Server 2025 x86_64; older desktop
OS versions and physical GPU/controller combinations need human testing.

## Controls and options

WASD is the stick; hold Shift to walk. Space jumps (A), J punches/dives (B), K
crouches/ground-pounds (Z). The arrow keys are the C buttons: Left/Right rotate
Lakitu, Down zooms out and Up back in; Up again looks through Mario's eyes (A,
B or another C button returns). E is the R button (switch between the Lakitu
and Mario cameras). R resets, M switches to the inspection camera (Mario
pauses). Esc opens pause/settings; Esc again resumes. The menu has Resume,
Restart course and Quit. Losing focus pauses until resumed and clears held keys
and pending taps. No game frames run while paused/unfocused or inspecting; elapsed
time and catch-up backlog from those periods are discarded. A running stall keeps
its backlog, draining at most eight fixed ticks per displayed frame. The development
pause freezes the camera too; the original game’s pause-camera behavior is pending.

Presentation flags: `--msaa 4`, `--no-fog`, `--no-cull`, `--size 1280x960`,
`--no-interpolation`, `--fullscreen`, `--no-vsync`. C/P/F toggle collision, placements and fog in the window.
Simulation stays at 30 ticks per second. Pause/settings can change interpolation,
fog, VSync and fullscreen without altering game state or logs. Smooth presentation
uses completed-frame interpolation with about one simulation tick of delay. Blink
and model-detail switches, landing and other animation changes no longer snap
the entire model to 30 Hz. With interpolation on, test jump → landing → run,
jump → landing → turn, and stop → idle → run. Clip changes keep Mario's position
and joint poses on the same completed-frame interval as the camera. Level restart
and pause/resume still clear pose history.

## Report a problem

Report the commit/build identifier from `BUILD_INFO.txt`, OS, GPU and driver, command/options,
what happened and what you expected. `--record private/runs` writes inputs
that developers can replay against the reference. Review a log before sharing;
it contains player controls and ROM identity. Do not share your ROM, extracted
assets, import caches, or ROM-derived state traces.
Original-execution traces remain necessary before claiming N64 fidelity.
