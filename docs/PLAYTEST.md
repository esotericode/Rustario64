# Rustario64 development build

This is an early Bob-omb Battlefield exploration build. Mario's non-object
movement and the original camera (Lakitu, the R-button Mario camera, C-Up
first person) have native decomp comparison coverage. BOB's act-1 coins
(yellow coins and coin formations) are drawn from the ROM and can be collected.
Collection sparkles animate, and the upper-right development counter reads the
original HUD value (which counts up every other tick). Coin positions interpolate;
texture animation stays at 30 Hz. BOB's twelve act-1 Bob-ombs walk, light their
fuses and chase Mario, explode (shaking the camera and knocking Mario back),
drop a coin the first time they explode, and respawn once Mario is far away;
they are drawn with their animated ROM model, fuse smoke, explosions and smoke.
Punching or diving into a Bob-omb picks it up: Mario carries it (walking,
jumping and landing), throws it with B (also in the air), drops it with Z, or
holds it until its fuse runs out. Object shadows and original HUD typography are
pending. Missions, King Bob-omb and every other object (including the cannon
lid), camera cutscenes, original pause behavior, water/cutscene actions, warps, saves,
audio are missing. Basic controller input is available; remapping, calibration,
rumble and controller-driven menu navigation are pending. Play stops on
unsupported paths; R re-enters.

## Start

Windows and Linux ZIP builds are attached to successful runs of the
[Rust foundation workflow](https://github.com/esotericode/Rustario64/actions/workflows/rust.yml).
Choose the latest successful run for the branch under test (Bob-ombs and
holding are on `claude/jolly-noether-2tta72`, stacked on the controller and
desktop launcher changes), then
download `rustario64-windows-x86_64` or `rustario64-linux-x86_64` under Artifacts.
GitHub's artifact ZIP contains the runtime ZIP; extract both layers. The build
identifier is in the runtime folder's BUILD_INFO.txt.

The Bob-omb and holding increments enable BOB's Bob-ombs in play, including
carrying and throwing them, with per-frame decomp comparisons. The launcher,
controller, coin and Bob-omb exploration flow is the current manual test
target.

Extract the whole ZIP. Keep your own ROM outside this folder. Only the original
8 MiB US v1.0 ROM is supported, identified after byte-order normalization by
SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce` (Z64, V64 or N64).
The build contains no ROM or game assets; it imports your ROM locally at launch.
Double-click `rustario64-desktop.exe` on Windows; it opens the ROM-selection
window without a console. Open `rustario64-desktop` on Linux; if your file
manager does not execute programs, run `./rustario64-desktop` once from a terminal.
The viewer and desktop launcher must stay in the same extracted folder.
Choose Browse, enter a local path, or drag the ROM into the launcher, then select
Play. Unsupported or unreadable ROMs show an error and allow another selection.
Remembering the path is opt-in and happens after successful validation/import.
The launcher offers interpolation, fog, VSync, fullscreen, window-size choices
and controller selection. It scrolls to fit smaller windows. Pause/settings has
Choose another ROM, which returns to selection in the same window. Every selected
file is validated even when the supported ROM's imported data is reused.
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

Linux needs libudev, a compatible system Vulkan or GL driver and X11/Wayland libraries
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

Mapped controllers use physical button positions (Xbox / PlayStation labels):

| Controller control | Original input / desktop action |
| --- | --- |
| Left stick | Analog N64 stick; partial deflection walks |
| A / Cross (south face button) | A: jump |
| X / Square (west/left face button) | B: punch/dive |
| LT / L2, LB / L1, or RT / R2 | Z: crouch/ground pound |
| RB / R1 | R: Lakitu/Mario camera |
| Right stick or D-pad | C buttons: rotate, zoom and first person |
| Start / Options | Development pause/resume |

The first connected controller is selected automatically. Select another or
Keyboard only in the launcher or pause menu; this choice lasts for the session.
Keyboard and controller buttons combine; held WASD takes movement priority.
The left stick has a 10% circular center dead zone: deflections at or below 0.10
are neutral before byte rounding. Outside that circle, normalized axes map
without rescaling to a circular radius of 80 raw units, then the original
controller applies its per-axis dead zone and clamp at 30 Hz. Its raw threshold
of 8 units is nominally 10% of 80, but byte rounding alone can admit motion just
below 10%; the host gate prevents that. The original per-axis processing can
also keep small diagonal deflections neutral beyond the host circle.
Right-stick camera directions use 0.55 press / 0.4 release
thresholds; analog triggers use 0.5 / 0.4. Short button/camera taps reach one tick.
Disconnecting the selected controller releases its inputs; gameplay continues
and the keyboard remains usable. Reconnecting does not change the pause state.
After focus, pause, restart, inspection or device changes, release buttons and
center sticks before controller gameplay rearms. Start works in play/pause;
other menus currently use mouse and keyboard.
Unmapped devices may require an SDL-compatible mapping through
`SDL_GAMECONTROLLERCONFIG`. A backend error appears in the menu and keyboard
controls remain usable. Physical controller compatibility still needs testing.

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

## Coin drawing checks

In Mario mode, approach a yellow coin or a ground coin formation. The coin
vanishes on collection, sparkles briefly appear, and the displayed counter
counts up. Check at 60 Hz and higher with interpolation on/off, then pause,
resume, change focus, toggle M and restart: objects must not streak from old
positions or reappear after collection. Missing actors are listed by the
importer; this remains exploration, not a completed mission.

## Bob-omb checks

From the start, walk toward the Bob-ombs on the path ahead (or launch with
`--start X,Y,Z[,YAW]` near one in development builds). Check that a Bob-omb
walks with a smooth animation, blinks, turns to face Mario, lights its fuse
(smoke puffs) and chases at a faster walk, then explodes after about five
seconds: a bright expanding explosion, the camera shake, a puff of dark smoke
and, when Mario is close, a knockback with health loss. The first explosion
drops a yellow coin that can be collected. Jump-kick a Bob-omb (A, then B in
the air): it flies off and explodes when it lands. Walk away and return:
it respawns at its home. Compare the explosion and smoke look with the original
game at 30, 60 and 144 Hz with interpolation on and off; report any part that
pops, faces the wrong way or flickers.

## Holding checks

Punch a Bob-omb (B while standing next to it, facing it) or dive into it
(B while running). Mario lifts it over his hands; check that it stays in his
hands while he stands, walks, runs, turns, jumps and lands, with no lag or
jitter at high frame rates. Press B to throw it (it flies forward and explodes
where it lands), B in the air to throw it from a jump, Z to set it down (it
walks off again), or hold it until the fuse runs out (it explodes beside him
and knocks him back). Report where the Bob-omb sits relative to his hands
compared with the original game, especially while running far from the
camera (Mario's lower-detail body is used there).

## Controller and launcher checks

Open the desktop launcher by double-click, browse to your local ROM, and play.
Check that drift within 10% stays neutral and X/Square attacks. Test gradual
walk/run deflection, jump, dive, ground pound, C-button rotation,
zoom/C-Up and R camera. Check brief taps at 60 Hz or faster and simultaneous
keyboard/controller presses. Unplug while moving: the game should continue,
controller inputs should release, and keyboard controls should still work.
Reconnect and check for stale movement/taps. Pause with Start and hold controls
across a focus change; release/center before resuming. Try two controllers and
Keyboard only. Choose another ROM, test
an unreadable/unsupported file, then select the valid ROM again. Report controller
model, USB/Bluetooth, mapping and OS alongside the normal build/GPU details.

## Session 24 local hardware evidence

Ubuntu 26.04.1, Intel Graphics (MTL), Vulkan: the native window starts from the
owner ROM, runs 120 draw frames/61 simulation ticks and closes cleanly. Its
recorded inputs replay exactly in the native oracle. Six renderer integration
checks pass with GPU access, and a 1280×960 screenshot showing Mario, a Bob-omb
and fuse smoke is inspected. Automated keyboard injection could not retain
window focus, so interactive keyboard/controller and Windows checks remain open.
King Bob-omb is still disabled; this session adds his standard movement
prerequisites, not an encounter that a human can play yet.


## Session 25 hardware regression

The rebuilt runtime is checked on the laptop's Intel Graphics (MTL), Vulkan.
A native window closes cleanly after 120 draw frames/60 simulation ticks; all
60 recorded ticks replay exactly in native C. Six offscreen renderer checks
pass with physical device access. A private 1280×960 Mario/Bob-omb screenshot
is visually inspected. The new boss grabbing helpers have component coverage;
King Bob-omb's placement/encounter and human input-feel checks remain pending.
