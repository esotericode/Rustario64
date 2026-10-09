# Rustario64 development build

This is an early Bob-omb Battlefield exploration build. Mario's non-object
movement has native decomp comparison coverage. Missions, objects (including
the cannon lid), the original camera, water/cutscene actions, warps, saves,
gamepad controls and audio are missing. The camera follows Mario; it does not
reproduce the original camera. Play stops on unsupported paths; R re-enters.

## Start

Extract the whole ZIP. Keep your own ROM outside this folder. Only the original
8 MiB US v1.0 ROM is supported, identified after byte-order normalization by
SHA-1 `9bef1128717f958171a4afac3ed78ee2bb4e86ce` (Z64, V64 or N64).
The build contains no ROM or game assets; it imports your ROM locally at launch.
A launcher with ROM selection and settings is planned; this build uses a terminal.

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

WASD is the stick; hold Shift to walk. Space jumps (A), J punches/dives (B),
K crouches/ground-pounds (Z). Left/Right arrows turn the follow camera. R resets,
M switches to the inspection camera (Mario pauses), Esc exits.

Presentation flags: `--msaa 4`, `--no-fog`, `--no-cull`, `--size 1280x960`,
`--no-interpolation`. C/P/F toggle collision, placements and fog in the window.
Simulation stays at 30 ticks per second. The settings menu is not implemented.

## Report a problem

Report the commit/build identifier from `BUILD_INFO.txt`, OS, GPU and driver, command/options,
what happened and what you expected. `--record private/runs` writes inputs
that developers can replay against the reference. Review a log before sharing;
it contains player controls and ROM identity. Do not share your ROM, extracted
assets, import caches, or ROM-derived state traces.
Original-execution traces remain necessary before claiming N64 fidelity.
