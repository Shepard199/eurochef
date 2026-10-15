# Robots 2005 Windows: Game Mode frontend resource audit

## Source and known IDs (not screenshot artwork)
Original corpus: `_eurotools_out/extracted_main/robots/binary/_bin_pc/h04_fron.edb`,
EngineX EDB v248, UID `0x0100000A`. Font EDB `h07_font.edb` UID
`0x01000042`. Another frontend/map asset `m00_fron.edb` UID
`0x0100000D`. The inspection helpers extract assets into
`target/robots_menu_assets/`; this folder is diagnostic, **not** a runtime
dependency. The current runtime decodes textures directly from `h04_fron.edb`
with `UXGeoTexture::read_hashcodes` whenever the indexed file is accessible.

| Resource | EDB-local UID | Format | Usage |
|---|---|---|---|
| Robots metallic logo | `0x86000035` | RGBA 512x256 | Title Screen |
| Left/top chrome rail and pipe | `0x8600003F` | RGBA 256x256 | Menu chrome |
| Left/bottom teal rail and pipe | `0x8600003E` | RGBA 256x256 | Menu chrome |
| Highlight bar | `0x8600001E` | RGBA 64x32 | Focus/hover |
| Factory/room still | `0x8600001A` | RGBA 256x256 | Fallback original frontend art behind menu, **not proven** to equal native live 3D background |

Original scripts exported with canonical naming by `eurochef-cli edb scripts`:

- `HudScript_FE_MainMenu_PC` `0x0400027D` (161-frame command route).
- `HudScript_FE_MainMenu` `0x04000009`.
- `HudScript_FE_TitleScreen` `0x04000035`.
- `HudScript_FE_TitleScreenRoot` `0x040002AE`.
- `HudScript_FE_NewGameMenu` `0x04000010`.
- `HudScript_FE_OptionsMenu_PC` `0x040002EC`.
- `HudScript_PauseMenu_ButtonPrompt` `0x04000285`.
- `HudScript_FE_QuitAppPrompt` `0x040002F8`.

Native menu is a HUD **Script + SubScript + Entity + Event** graph, not a
flat static screenshot. The current `app/menu_assets.rs` is an evidence-backed
first rendering pass: actual source textures/teal chromes and hover bar,
but approximate layout (mirrored halves), temporary font metrics,
and a static H04 scene still. It must not be tagged `pixel-exact`
until script transforms, native bitmap font glyph metrics, selection transition,
and 3D animated background are restored.

## Screenshot-driven frontend composition correction (2026-10-14)

The user screenshots show a M00 map fragment through the title logo, black
rectangles behind the menu and controls overlapping the two chrome panels.
The original H04 art decoder is **not** responsible for the black matte:
direct decoded RGBA tests confirm transparent corners/large transparent
areas for `0x86000035` (logo), `0x8600003E/3F` (chrome), partial alpha
for `0x8600001E` (selection), and an opaque still `0x8600001A`.
The host painted a black rectangle over the chrome with
`Color32::from_black_alpha(190)`; that overlay has been removed.

The provisional H04 chrome now uses a shared centered square stage at
**82% of the shorter viewport dimension**; the five native Main Menu
rows start at 31.5% of that stage, use reduced spacing and remain inside
the upper enamel panel, above the exposed teeth. The extraneous sixth
`Open Editor` entry is no longer drawn over the original UI; **F2**
invokes Editor (or EDB data selection). The Title Screen retains its
wider clickable Press Enter highlight.

**F4** toggles the M00 3D scene preview. It defaults OFF because its
bounds-based orbit camera is not the original front-end camera and made
an unrelated map chunk float through the original transparent logo.
M00 loading/script clocks remain intact. **F3** retains the H04
inspector. In normal mode the Title Screen uses a clean dark stage,
and Main Menu uses the subdued original H04 still under alpha chrome.
These are explicit intermediate choices, **not claimed pixel-exact**.

Tests verify sprite alpha and chrome/button bounds at 960×540,
1280×1024, Full HD and 4K. Windows paths
`D:\GitHubHome\x64dbg\x32\x32dbg.exe` and
`D:\Games\Robots\Robots.exe` were verified to exist; actually
capturing the original camera/controller state under x32dbg and visual
GPU parity are still pending.

## Widescreen background and exact-hitbox update from screenshot (2026-10-14)

The second pair of user screenshots confirmed the intrusive M00 fragment was
gone, but exposed a hard-edged *square* factory still centered inside a wider
window and five H07 text rows running into the upper chrome's teeth.

`menu_assets.rs` now draws the 512×512 opaque H04 fallback still as a
full-viewport **aspect-preserving cover**: UVs crop the source vertically on
wide windows or horizontally on narrow ones. This removes black borders
without distorting original content or modifying the EDB decoder. The
separate 82%-shorter-side 640×640 HUD chrome stage is unchanged.
The background is still a **low-resolution original bitmap**; this does not
increase real source detail or recreate the correct live M00 camera.

`game_mode.rs` draws original H07 menu labels using exact
`Ui::allocate_response` hitboxes instead of invisible `egui::Button`
widgets, which silently inserted ordinary button text padding. The
H04-specific child layout now has zero automatic item spacing and the
five-row origin is moved from 31.5% to 29.5% of the HUD stage. Mouse and
keyboard navigation continue to use the same hitboxes. Any running
session no longer adds an extra `Resume Session` row across the chrome:
**F6** resumes from Main Menu if a session exists, while the native visual
menu stays five rows. **F2** Editor, **F3** HUD diagnostics and **F4**
M00 3D preview remain available.

Tests: `app::menu_assets::tests` 10/10, including cover UV
coordinates for square/16:9/9:16 images, and `app::game_mode::tests`
8/8. `cargo build -p eurochef-gui` succeeded and updated the debug EXE
used by `RUN_GAME_MODE_FAST.cmd`. Visual parity on an actual GPU
still awaits a user screenshot and a verified original camera/controller
trace; no claim of finished animation fidelity.

## Original frontend Camera capture bridge (2026-10-14)

Ghidra now identifies the **runtime camera state and original default lens**:

- During M00/frontend initialization, `Robots.exe 0x004D9070` calls the
  native Camera controller `0x004729D0` with **camera mode 0**.
- The actual update `0x00472B70` reads camera eye from
  `0x007B2BE0/2BE4/2BE8`, target from
  `0x007B2BF0/2BF4/2BF8`, and bank/roll input
  `0x007B2C00`, in floating-point units. It propagates position,
  target and world-up to the backend view object.
- The original `DAT_00619AFC` is **45.0f**, passed through
  `DAT_005DE538 = 0.017453292f` (degrees → radians) during camera
  updates. This establishes the default lens constant for the native
  update path, although dynamic changes to FOV remain possible.

A **new module** `app/frontend_native_camera.rs` accepts measured,
non-fabricated poses from the original PC runtime. The frontend OpenGL
callback now sets its existing `NativeViewCamera` from those samples
*before* calculating view/projection matrices and drawing M00. Both formats
are optional and expected beside the selected `m00_fron.edb`:

1. **`frontend_camera_dump.bin`**: an exact **0x24-byte memory dump** of
   original `Robots.exe` address `0x007B2BE0` while the frontend
   camera is active. The importer takes eye xyz at offsets 0/4/8,
   target xyz at 0x10/0x14/0x18, roll at 0x20, and native default FOV
   45°. It is a single pose at background time zero.
2. **`frontend_camera_capture.txt`**: a measured, time-ordered camera
   track, one line per observation with **nine float values**:
   `seconds eyeX eyeY eyeZ targetX targetY targetZ verticalFovDegrees rollDegrees`.
   Lines beginning with `#` and trailing `#` comments are allowed.
   The importer validates each frame, retains original measurements, and
   linearly interpolates between samples; that interpolation is a host
   approximation, **not yet the native camera controller easing function**.

To collect the binary sample using the user's installed x32dbg: load the
original `D:\Games\Robots\Robots.exe` in
`D:\GitHubHome\x64dbg\x32\x32dbg.exe`, run to the frontend scene
(or break in Camera update `0x00472B70` while the menu is visible),
and save **36 bytes (0x24)** beginning at `0x007B2BE0` from the
Dump/Memory view. If the executable loads at a different image base,
use `Robots.exe + 0x003B2BE0` instead of the absolute address.
Copy that capture alongside `m00_fron.edb` as
`frontend_camera_dump.bin` and restart Game Mode. If a valid text
track and binary dump both exist, the binary dump takes priority.

**Behavior:** a validated native capture auto-enables M00 rendering in
Game Mode with the captured Camera; **F4** can still toggle it. No file
or invalid/corrupt data retains the safe 2D fallback; F3 reports the
camera source. The gameplay camera and MapFrame are unaffected.

**Evidence boundary:** an original x32dbg runtime dump has not yet
been obtained in this remote pass, and the real original controller
path for per-frame movements/roll and UI animation remains outstanding.
The module is implemented and covered by unit tests, but no captured
pose or screenshot comparison is claimed.

## Main Menu native H04 Entity controller motion (2026-10-14)

A shipped PC `h04_fron.edb` command/controller inventory proves that
`MainMenu_PC` (`0x0400027D`, 30 FPS) has **two parallel Entity
`0x82000031` commands** (indices 1 and 2). Both use five native
`vector_0` position keys with identical Y coordinates and different X,
strong evidence of mirrored mechanical members:

| Script frame | Native Entity position Y |
| ---: | ---: |
| 0 | -32.314280 |
| 16 | -17.623365 |
| 36 | -3.643300 |
| 39 | -3.643300 |
| 54 | -12.729241 |

The values establish the original **rise → overshoot → settle** profile.
This behavior is not the previously guessed one-way fade/translation.
`FrontendScriptNode::from_native` now extracts the pair **from the
original controller data** by UID and controller header index, validates
matching/sorted/finite Y lanes, and retains the actual frame keys.
`main_menu_chrome_motion` samples them with the **same sine interpolator
used by the shared Script renderer**, expressing motion relative to the
final frame-54 pose. Other screens are unaffected.

`game_mode.rs` advances this native H04 curve on the screen-local
playhead, feeding the same normalized result to the H04 chrome sprites
and their corresponding text/click hitboxes, so the labels track the
metal panels instead of floating while the art moves. F3 diagnostics
show the live normalized controller offset. This is live GPU-presented
animation in Game Mode, not merely a log of keyframes.

**Projection boundary:** the original HUD Entity-to-screen projection
and exact per-Entity texture mapping are not yet recovered. For the
temporary 2D H04 sprites, normalized world Y currently maps to
`0.5 × logical chrome stage height` pixels. That scale is explicit
and tested at 960×540/1080p/4K; **do not claim pixel-exact native HUD
transforms**. Both menu chrome halves currently share the paired Y
motion; direct native 3D HUD geometry rendering and individual HUD
text/highlight controllers remain future work.

Native serialized-H04 motion tests and layout regression all pass.
The one-off detailed controller dump remains in
`target/h04-controller-audit.log` but its ignored probe test was
removed to keep the normal test suite focused. Gameplay NPC simulation,
camera capture and independent M00 clock are unchanged.

## Correction: H04 Entity controllers are not automatically HUD sprite transforms (2026-10-14)

Real-world screenshots show the last chrome animation still moves incorrectly.
The shipped `MainMenu_PC` Entity `0x82000031` Y keys
`0,16,36,39,54` are real, but **applying one normalized Y curve to all
four separate H04 chrome textures and the H07 labels was not proven by
the original renderer**. In particular, `0.5 * chrome-stage-height`
was a guessed screen-pixel conversion and should not run automatically.

`game_mode.rs` now leaves this approximate whole-panel movement **OFF
by default**. Pressing **F5** enables/disables its debug preview, while
the underlying H04 script still samples real controller values at all
times. F3 identifies that this is an unverified projection. The M00
camera capture and game/NPC simulation are unchanged.

To discover the correct mapping from runtime Entity controllers to each
metal-panel sprite, HUD text, and final projected matrices, a read-only
x32dbg logging script is now in `.ai-bridge/h04_menu_x32dbg_trace.txt`,
with `.ai-bridge/h04_x32dbg_capture_GUIDE.md` describing use. Its
breakpoints cover `0x00440640` (HUD option selection),
`0x0043D2C0` (JumpToMarker), and `0x0043D730`
(0xA4 record allocation), using log-only conditions. This first pass
correlates original script instances; *final per-widget matrix values are
not captured yet*, and no debugger session was executed in this turn.
Do not claim visual animation parity pending a runtime trace and GPU
comparison.

## Windows one-click Game Mode launcher

The workspace root contains **`RUN_GAME_MODE.cmd`**. Double-click it to
immediately start the original-style Title Screen in the existing **debug**
`eurochef.exe --play`, with `m01_vill.edb` preloaded from the configured
`GAME_ROOT` (workspace `.env`) or the existing local corpus. If no compiled EXE
exists, it automatically builds one before launching.

Use `/build` after changing Rust code to compile the current debug version
before running it; this routes through the canonical
`_tools/eurochef-main_legacy/RUN_GUI.cmd -DebugBuild`. Use `/release` to
select the optimized release EXE (and `/build /release` to rebuild it).
`/fast` explicitly selects the default immediate launch. `/trace` enables
verbose debug logging and can be combined with `/build`; `/choose` requests
another map EDB path, including drag-and-drop; `/dry-run` validates the paths
and environment without compiling or launching; `/help` displays usage.
A path to a different `.edb` may also be passed as the positional argument
(or dragged directly onto the CMD). The wrapper reuses the game's EDB
manifest and refuses to launch with an absent map.

The regression helper `tools/Test-GameModeStartup.ps1` attempts a bounded
4-second process startup then terminates its child. On the current Moon97
remote session it **fails** at eframe/egui_glow initialization:
`egui_glow requires opengl 2.0+`. The remote graphics session exposes no
adequate OpenGL context. This is an environment/hardware limitation in the
remote smoke run, not proof that the local interactive Windows GPU run succeeds
or fails; the UI still needs a real graphical smoke test on the target desktop.

The CMD selects a **file**, not a playable Player spawn; a level still needs
a valid native context-0 Player for **New Game** to succeed. Choosing a map
does not bypass that runtime gate.

## Windows one-click Game Mode launcher (2026-10-13)

The top-level workspace file `RUN_GAME_MODE.cmd` launches `eurochef.exe` with
`--play` and a real EDB path, bypassing the editor and entering the Robots
Title Screen. Double-clicking the CMD uses the existing `RUN_GUI.cmd` /
`RUN_GUI.ps1` build environment to build the current **release** code before
launch; the debug build is available with `/trace`. No parallel compiler or
secondary custom Rust entrypoint is introduced.

- `RUN_GAME_MODE.cmd` starts from `GAME_ROOT` in the project `.env` (fallback
  `D:\Games\Robots`), preferring its `m01_vill.edb` and then the test corpus in
  `target/attachment_game_root/.../_bin_pc/m01_vill.edb`.
- `RUN_GAME_MODE.cmd /fast` runs the **existing** release executable; it does
  **not** rebuild and can therefore be stale. `/trace /fast` runs the existing
  debug executable. `/trace` builds and runs debug. `/choose` prompts for a
  different EDB. Drag-dropping an EDB file onto the CMD also works. `/dry-run`
  validates the selected paths without building or opening the game.
- The script preserves the EDB manifest environment needed for multi-file
  resource lookup and reports missing files or nonzero exit codes instead of
  silently returning to the desktop. Game saves remain experimental checkpoint
  v1, not original PC saves.

For convenient one-click rapid testing, the top-level
`RUN_GAME_MODE_FAST.cmd` delegates to `/fast /trace`, launching the last
successfully built **debug** EXE without recompiling. This build has been
verified but can become stale as development continues.

`tools/Test-GameModeLauncher.ps1` checks both launchers' contracts and the real
EDB fixture statically. A headless native CMD smoke test was not available in
this remote runner; interactive click-through remains outstanding. The current
**debug** binary was built successfully via `RUN_GUI.ps1 -Trace -BuildOnly`.
A release build attempt exceeded the remote command's 180-second timeout, so
there is **no claim** that the existing release binary includes these changes.

## H04 script-timed frontend presentation and missing-resource sentinel (2026-10-13)

Observed debug message `Checked load for unknown object type 0x7fff0000 (hc ffffffff)`
originates in `RenderStore::is_object_loaded` when a valid missing-resource
sentinel `0xFFFFFFFF` reaches type classification. It is **not** itself evidence
of a failed geometry or animation load. `render/mod.rs` now treats exactly
`u32::MAX` as already-satisfied/absent before classifying object type, with
regression `missing_resource_sentinel_never_becomes_unknown_render_object`.

The frontend HUD was previously entirely static despite script sampling. An
incremental **2D presentation animation**, not native controller parity, is now
visible: `frontend_script.rs` uses the first two distinct Entity activation
frames from each original H04 script as an entrance interval. In the shipped
Windows corpus, `FE_TitleScreenRoot` uses `0 -> 190` at 30 FPS, while
`FE_MainMenu_PC` uses `0 -> 40` at 30 FPS. A bounded eased fraction now drives
the original H04 title-logo entrance and movement of existing chrome sprites;
other frontend screens retain static rendering. The actual H04 script controls
the **timing only**; fade amount and slide distance are provisional host art,
not recovered original transforms. Regression coverage checks exact shipped
boundary/halfway times and the marker handling.

**Still missing for original animation parity:** 3D frontend scene (including
`m00_fron.edb`), native 2D/3D controller matrices, script event dispatch,
subscript transitions, and animated Entity rendering into the game window.
This incremental entrance must not be mistaken for a reproduction of the
original cinematic main menu.

## M00 native 3D frontend layer and H04 Event edges (2026-10-13)

Implemented `app/frontend_scene3d.rs` as a **separate GPU/frontend resource
owner**, not a second gameplay MapFrame. It opens the shipped adjacent
`m00_fron.edb` (UID `0x0100000D`) and loads its 5 Entity resources,
4 native Script resources, native particle resources, and 98 EDB textures.
This EDB has 1 Map and no standalone Animation or AnimSkin entries.
Renderable Entity/Particle Script roots run using the existing
`render/script.rs` controller interpolation, EntityRenderer draw passes,
and ParticleRenderer, with the original serialized FPS and repeat opcode
(where present). Their OpenGL callback is painted beneath the H04 chrome
and H07 fonts on Title/Main Menu; if M00 is absent/invalid, the original
H04 still-image fallback remains. The 3D store is independent of gameplay
resource purges when changing levels. The provisional orbit camera focuses
on decoded mesh centers; it is **not yet the original ViewCamera**.
The MapZone static geometry and placements were added in the subsequent
increment below; exact H04-controlled scene activation remains open.

The H04 frontend script module now schedules each Event **start edge**
across bounded time intervals, recursively including SubScripts and
respecting clip ends; zero-time starts are handled once per screen.
Game Mode retains the last 16 scheduled native Event UIDs in the F3
diagnostic and traces starts. Unknown event payloads do **not yet have
side effects**; this is intentional until original semantics are decoded.
Both H04 and M00 original-resource regression tests and the four
Game Mode navigation tests have passed. OpenGL visual parity must still
be tested interactively against the original Windows main menu.

**Next work:** map zone 3D static scene presentation, true frontend
camera and controller root/event binding, correct script-scene ownership
per screen, and integration of native menu sounds/scene transitions.

## M00 static zone and placed Entity geometry (2026-10-13)

The shipped Windows `m00_fron.edb` map `0x85000000` has **one MapZone,
two Entity placements, and zero EXGeoCamera records**. The single MapZone's
refpointer `0` resolves to a real mesh of **32,789 vertices / 787 strips**.
The placements are local Entity index `0x82000001` at the origin and
`0x82000000` with serialized translation/rotation. These concrete facts
are asserted by `shipped_m00_entity_scripts_produce_real_native_render_queue`.

`frontend_scene3d.rs` now loads the native `ProcessedMap` through the
existing `maps::read_from_file` parser and retains the MapZone refpointer
mesh as a separate static `EntityRenderer`. It draws both opaque and
transparent zone passes before existing script-controlled objects; map
placements are queued through `QueuedEntityRender` using the same
`EulerRot::ZXY` transform convention as `map_frame/canvas.rs`. Local
`0x82` references are resolved by the existing renderer, not fabricated.

There is no camera record to copy from M00. Until the *actual frontend
camera owner* is identified in the original binary/H04 transition path,
the dedicated `BaseViewer` uses a **provisional geometric framing** from
the decoded MapZone bounds instead of a fixed world-zero orbit. This is not
the original camera position, lens or animation.

The original EDB regression and `cargo check -p eurochef-gui` passed.
The canonical `RUN_GUI.ps1 -Trace -BuildOnly` finished successfully and
updated `target/debug/eurochef.exe` for `RUN_GAME_MODE_FAST.cmd`.
An actual Windows OpenGL screenshot parity check is still pending.

## Widescreen/HiDPI Game Mode and native 0x46 presenter discovery (2026-10-13)

The previous `--play` window was hard-coded to **1280×1024**; the original
menu buttons were fixed at 250×28 logical points. The startup viewport is now
**maximized on the actual Windows desktop** (1920×1080 requested initial size,
960×540 minimum), with the Editor's old viewport unchanged. H07 glyph sizes,
button hitboxes and gaps scale with the *logical* viewport and preserve the
GPU's monitor DPI framebuffer. H04 still uses its original textures, sampled
linearly. The original H07 256×256 bitmap atlas also uses linear filtering at
high scaling; this improves interpolation but **does not** create higher
resolution source art. GUI tests cover compact, 1080p, and 4K viewport
dimensions.

M00 3D time now continues between Title Screen, Main Menu and Options instead
of restarting on every H04 screen transition. H04 screen-local scripts still
reset on entry. This removes a host-level animation discontinuity; it is not
a claim that every native controller has been recovered.

New Ghidra observations:
- `Robots.exe 0x00436150` loads M00 map `0x85000000` then invokes
  `0x00425140` (XPathManager path creation) and `0x0044B510`
  (XTriggerManager creation). Neither is the camera setter.
- `0x004D9070` invokes native UI `SetClipSize(0x280,0x280)` (640×640),
  establishing the *virtual HUD clipping size*, NOT a limit on Windows
  monitor/framebuffer resolution.
- `0x00499960` dispatches a distinct presentation resource through
  `0x00440600` according to menu state:
  `0x0400027D -> 0x46000001` (Main),
  `0x04000010 -> 0x4600000C` (New),
  `0x04000011 -> 0x4600000D` (Load),
  `0x040002EC -> 0x46000022` (Options).
  These `0x46` resources are **not** the same identifiers as the
  `0x04` EXGeoScript roots currently sampled from H04. Game Mode records
  the proven mapping and shows it in F3 diagnostics; the `0x46` loader,
  controls, and exact 2D/3D animation playback remain OPEN.
- `0x0049A6C0` contains a scoped timer for particular bonus/loading menu
  transitions; it must not be applied indiscriminately to every menu state.

Native camera creation/pose was **not** demonstrated by these functions.
The current M00 orbit remains a fallback and real GPU screenshot comparison
remains pending. This preserves honest boundaries while targeting the actual
missing frontend animation subsystem.

## Native 0x46 marker payload and script jump bridge (2026-10-14)

The previous hypothesis that each 0x46 frontend identifier must be loaded as
an independent EDB resource was **too strong**. New exact-binary Ghidra
analysis of the native HUD owner established a genuine *marker-selection*
mechanism:

- `0x00440600` resolves the supplied 0x46 UID in an array of 12-byte
  descriptors at owner `+0x11C`, count `+0x118`. Each descriptor contains
  a source record index, marker UID (entry `+4`), and a numeric script-frame
  marker lane.
- Owner vslot `+0xB4` resolves to `0x00440640` in the primary frontend
  vtables, including `0x005ED478`. This method checks selection bounds.
  Depending on the third argument, it queues selection in `+0x144/+0x148`
  or immediately exits the previous selection, sets `+0x120`, changes
  state `+0x10C=2`, and seeks the script via `0x0043D2C0`.
- `0x00440020` services the pending selection and calls virtual selected
  item handlers `+0x9C/+0xA0`. `0x0043FD40` builds the native marker
  descriptors by scanning the `0xA4`-byte runtime status entries with
  marker flag bit 2. These steps explain why menu animations cannot be
  reconstructed by independently restarting each H04 script at frame 0.

A separate shipped PC H04 corpus inspection revealed the original marker
UIDs in **opcode 11, Event type `0x16000008`**. Its exact observed
20-byte payload is five little-endian u32 words
`[4, 4, 3, marker_uid, 0]`:

| H04 script | Event frame | Embedded native marker |
| --- | ---: | --- |
| `0x0400027D` MainMenu_PC | 56 | `0x46000001` |
| `0x04000010` NewGame | 25 | `0x4600000C` |
| `0x04000011` LoadGame | 24 | `0x4600000D` |
| `0x040002EC` Options_PC | 75 | `0x46000022` |

`frontend_script.rs` now parses this event layout *fail closed* into
`presentation_marker`, propagates it through active Script samples and
Event start-edge scheduling, and preserves the event clock time. Game Mode
tracks the last observed native marker and exposes it beside the independently
verified expected selection in F3 diagnostics. This code avoids treating
unknown Event payloads as commands. The regression checks all four marker
identities on the real shipped H04 data, the MainMenu frame-56 start edge,
and synthetic malformed/truncated payloads. The one-time raw-byte audit
fixture was removed after promotion to durable tests.

**Still open:** exact event-to-owner routing, per-marker native renderer
controller poses/geometry, animation completion callbacks and original menu
camera. The recovered marker-selection code does not by itself mean that
the host now visually reproduces those controller transitions.

## Native +0x9C/+0xA0 handlers and marker-selector state (2026-10-14)

The generic callback handles `0x00440750` are **no-ops** (`RET 0xC`),
not mesh/opacity animators. Concrete derived frontend vtable targets:

| Class/vtable | +0x9C | +0xA0 | Proven behavior |
| --- | --- | --- | --- |
| Base `0x005E0C08` | `0x00440750` | `0x00440750` | no-op |
| Menu `0x005ED3B0` | `0x004980C0` | `0x00498080` | option UID dispatch; completion/memcard |
| Settings `0x005ED478` | `0x0049E910` | `0x00440750` | game/input options and defaults |
| Derived `0x005EE980` | `0x004A8BE0` | `0x00440750` | +0x9C tail-jumps to settings dispatcher |

The original `0x0049E910` options handler indexes `0x005EDCA0`
(HudOption UIDs) and `0x005EDE7C` (SaveData option UIDs). For left/right
adjustments it accepts only music volume `0x5800000E` and effects volume
`0x5800000D` (input codes `0x2A/0x2B`). Supported 14-row table values
are retained in `frontend_hud_dispatch.rs::native_option_binding`;
sentinel `0xAABBCCDD` is not exposed as a saved setting. The original
SaveData writer has not yet been connected to the host's Settings widgets.

The `0x004980C0` dispatcher includes an important *different screen*:
`0x0049A5B0` cycles the original credits HUD text elements
`HT_HudText_Credits_Names01…16` (UIDs `0x49000064`,
`0x49000082…89`, `0x490000B1…B7`) via its `0x49000064`
animator and `0x00439630`. **These are not Main Menu logo transforms.**
The callback functions do not yield generic per-widget pose/opacity data.

A modular `NativeHudMarkerSelector` now models the proven part of
`0x00440640` and `0x00440020`: validates 0x46 identifiers, selects
the corresponding decoded H04 Event marker, supports deferred selection,
consumes it on tick, records native selection state 2, and rejects missing
or malformed descriptors. `Game Mode` creates this selection from the
actual loaded H04 per-screen markers and shows selected UID, state,
and Event frame alongside the original Event clock in F3. The later
Ghidra producer trace below **proves the distinction** between
Event.start and target frame from the Event's fifth payload word.
The selector now seeks that native target, but genuine HUD controller
poses and visual parity remain open. Gameplay MapFrame is not touched.

Native animation fidelity is still blocked on the H04-owned widget/Entity
presentation pipeline, native camera and status frame-to-pose link. Do
not apply credits animation logic to the Main Menu.

## H04 Entity instance records and local pose sampling (2026-10-15)

Ghidra Robots.exe routes opcode-3 through 0x0043B800 -> 0x0043B880. It appends a distinct 0x30-byte runtime Entity record for each Script command, retaining resource UID and start/end frames. Opcode-4 SubScript uses a separate 0x0043B4D0 -> 0x0043B590 path with 0x40-byte records. Native 0x0043E8E0 loops Entity records at owner +0x64 (count at +0x60), tests per-instance start/end against the script clock, and resolves matching instances through 0x0043A910 and 0x0043A990.

EuroChef now preserves the shipped UXGeoScript command/controller source inside FrontendScriptNode. Its entity_poses method returns local position/rotation/scale independently per serialized Entity command index using the same interpolation routine as the existing 3D Script renderer. Original keyframes are decoded; identical easing on Windows is not yet established. The F3 diagnostics list up to ten active instance poses and never collapse repeated Entity UIDs.

The H04 corpus regression confirms MainMenu_PC commands 1 and 2 both target Entity 0x82000031 yet have different initial X, matching initial Y, and nonzero Y travel by frame 30. This is not evidence that all four 2D chrome sprites share one transform.

UNKNOWN: downstream parent/world matrix, HUD projection, per-sprite material binding, alpha. F5 provisional whole-panel motion remains OFF by default. M00 camera and NPC simulation unchanged.

Further native matrix/visual-param route: 0x0043E8E0 optionally invokes 0x004E672D, which stores a full 4x4 float matrix at Entity instance +0x90, incorporating local instance translation at +0xD0..+0xD8 and multiplying a passed-in matrix under the native composition flags (+0x7C). It prepares a 0x30-byte visual packet through 0x0052BBA2 and invokes the resolved runtime object's vslot +0x14. The packet merger includes a signed-16 field +0x1C and a flag 0x200 path multiplying two lanes /32767. The exact HUD mesh submission, projection, per-sprite binding and final opacity still require following the actual virtual target; the opcode alone is not proof of the final pixel color. Ghidra evidence .ai-bridge/ghidra-h04-entity-vslot14-inputs-20261015.txt.

H04 `+0x14` dispatch refinement (2026-10-15): `0x0043E8E0` calls `0x0043A410` to acquire the renderer-side receiver, then invokes `(**(code **)(*receiver + 0x14))(instance+0x90, visual_packet, flags)`. `0x0043A410` calls `0x004FB1B7`; the latter **looks up** an existing object from the owner `+0x110` binding table (0x20-byte entries) by resolved resource index/object field `+0x20`, or via fallback identifier matching, and returns that object's pointer. Therefore `+0x14` is not yet a proven single implementation: identify the concrete receiver's vtable at runtime, then follow its actual slot function to geometry submission / Direct3D. Do not equate the Script Entity UID with a particular render class without this pointer provenance. Additional Ghidra evidence: `.ai-bridge/ghidra-h04-vslot14-resolver-20261015.txt`.

Global renderer + Direct3D9 draw boundary (2026-10-15): XREF to renderer setter `0x0052AFB4` identifies two initialization paths. `0x004321A0` creates a frontend-derived renderer whose constructor ends with vtable `0x005E0788`; the alternative initialization around `0x004F8BAA` calls `0x0052E9AB`, which initializes vtable `0x005F4738`. Independently dumped vtables agree: both `+0x20` slots resolve to **`0x0052F6E6`**. That method prepares composite world/view/projection matrices (global `DAT_008C96C0` etc), not a direct GPU draw. The independent render pass path `0x00525118 / 0x0052552D → 0x00572128 → 0x00571C7E → 0x00571E32` issues D3D9 draw calls. `0x005325D8` obtains actual IDirect3DDevice9 from `IDirect3D9::CreateDevice` (vslot `+0x40`) into **`DAT_008C9E34`**. `0x00571E32` calls that device's `+0x148` with the seven-parameter `DrawIndexedPrimitive` signature, or `+0x144` with `DrawPrimitive` signatures. Thus the backend draw boundary is **PROVEN_STATIC**, including device-pointer provenance and exact slots. However, a specific H04 `0x82000031` instance's mesh/batch submission to this draw path is still **UNKNOWN**: no live MainMenu_PC capture at `0x0043EB78`, and no per-panel texture mapping/projection/alpha confirmation. It is incorrect to equate the static device-wide draw route with proof of H04 chrome geometry. Source logs `.ai-bridge/ghidra-h04-backend-interface-setter-xrefs-20261015.txt`, `ghidra-h04-render-backend-binding-calls-20261015.txt`, `ghidra-h04-render-backend-vtable-20261015.txt`, `ghidra-h04-secondary-global-render-vtable-20261015.txt`, `ghidra-h04-d3d9-device-and-draw-dispatch-20261015.txt`, `ghidra-h04-mesh-draw-queue-20261015.txt`.

Native 0x82 Entity renderer dispatch recovery (2026-10-15): the H04 call is at **0x0043EB78** (`MOV EAX,[EBX]`; `MOV ECX,EBX`; `CALL dword ptr [EAX+0x14]`). Ghidra independently traced the runtime object's lookup and construction: `0x004FB1B7` resolves an existing owner +0x110 / +0x124 binding; `0x004F9925` allocates those 0x20-byte entries; `0x004FA74C → 0x004FA332 → 0x004F9C6B` creates entries by serialized type; `0x004F9D48` uses `0x004E91F3` to instantiate the bound resource type. For resource UID class `0x82`, `0x004E91F3` masks the top-bit and selects class `0x02`, constructor `0x0050696E`, which installs **vtable 0x005F40A8**. An independent vtable pointer read fixes **slot +0x14 = 0x00506B84**. This is PROVEN_STATIC for a successful bound class-0x82 instance; H04's actual receiver at a specific runtime frame has not yet been captured.

`0x00506B84` conditionally recomputes the Entity instance matrix (`0x004E672D`), merges a 0x30-byte visual block (`0x0052BBA2`), commits the 4x4 combined matrix via `0x004EC482` (global output `DAT_008C9540`), and propagates state via `0x0052BDE2` (global `DAT_008C9B3C`). It dispatches into an injected global render interface `DAT_008C98A8` vslot `+0x20` and through per-mesh/per-render-component vslot `+0x0C`. The global interface is assigned by `0x0052AFB4`; the concrete backend and its Direct3D calls are NOT proven. Nested hierarchy traversal `0x004E5008` invokes child vslot `+0x14`. No pixel-exact HUD projection, per-panel texture assignment, or final alpha is claimed; retain F5 unverified chrome motion OFF by default. Evidence: `.ai-bridge/ghidra-h04-indirect-callsite-insns.txt`, `ghidra-h04-binding-producers-20261015.txt`, `ghidra-h04-resource-object-factories.txt`, `ghidra-h04-82-receiver-ctor-20261015.txt`, `ghidra-h04-82-receiver-vtable-20261015.txt`, `ghidra-h04-concrete-entity-vslot14-20261015.txt`, `ghidra-h04-entity-render-submit-route-20261015.txt`.

Ghidra evidence: .ai-bridge/ghidra-h04-entity-transform-route-20261014.txt; .ai-bridge/ghidra-h04-render-entity-resolve-20261014.txt; .ai-bridge/ghidra-h04-sprite-instance-consumers-20261014.txt; .ai-bridge/ghidra-h04-entity-instance-stage-20261014.txt.

## H04 runtime record indices and Script frames: corrected with x32dbg (2026-10-14)

**Correction to prior claim:** the last float of an opcode-11 H04 Event
payload is descriptor metadata, NOT the direct JumpToMarker playhead.
Likewise, JumpToMarker(float) receives a **runtime record index**, NOT
an absolute animation frame number.

Static Ghidra and the user's live x32dbg trace of
`R_widescreen_devmenu_windowed.exe` confirm:

1. `0x0043BE90` visits every opcode-11 Event `0x16000008`, including
   nonselectable sentinels. `0x0043D730` appends a 0xA4-byte runtime
   record whose first fields are the Event's source `start` and end.
2. `0x0043FD40` builds a 12-byte descriptor with **record index +0**,
   marker UID +4, and `__ftol` of the payload float +8.
3. `0x004406CD FILD [descriptor]` reads **index +0**, then
   `0x004406D6` calls `0x0043D2C0`. This routine selects
   `runtime_records + index*0xA4` and jumps to the record's source
   `start` on immediate selection. Its fractional argument supports
   interpolation within/among records; it is not seconds.

| Menu | Native marker UID | Record index | H04 start frame (30 FPS) |
| --- | --- | ---: | ---: |
| Main | 0x46000001 | 1 | 56 |
| New | 0x4600000C | 1 | 25 |
| Load | 0x4600000D | 1 | 24 |
| Options | 0x46000022 | 1 | 75 |

The main script has a preceding nonselectable record 0 at frame 39;
the next selectable UIDs 0x46000002/03/04/3C use record indices
2/3/4/5 and frames 66/76/86/96 respectively. This independently
agrees with the live `HUD_SELECT 0 -> HUD_JUMP 1` sequence.

**EuroChef fix:** `FrontendScriptCatalog::marker_evidence`
enumerates every native event record BEFORE filtering markers.
`NativeHudMarkerSelector` now carries its runtime record ordinal
separately from the Event.start and the final metadata float. Game
Mode's screen-local H04 playhead begins from Event.start/FPS. The M00
3D background, camera snapshot and gameplay MapFrame are unchanged.

**Remaining:** the Entity 0x82000031 position channel does not
by itself establish a native mapping to the four metal HUD sprites,
text positions, opacity, or GPU projection. The unverified
whole-panel 2D translation remains disabled by default (F5 debug).
No claim of full native HUD motion or final GPU parity is made.

## Runtime boundaries

`eurochef/gui/src/app/game_mode.rs`: Title Screen -> Main Menu ->
New Game slots/confirmation -> Launching -> Running; Pause / Options /
Load Game, with Editor as a deliberate developer exit. `--play` opens the
Title Screen; menu art is resolved from the current EDB index or an adjacent
`h04_fron.edb`. `app/checkpoint.rs` stores only versioned persistent
Player/inventory/mission groups and a shipped EDB UID. It does **not** persist
per-NPC/XItem world state or emulate the original platform SaveGame format.

## 2026-10-13 frontend Script interpreter stage

Added `eurochef/gui/src/app/frontend_script.rs`, parsed **directly from
h04_fron.edb at runtime** (no dependency on the exported JSON dump).
`FrontendScriptCatalog` loads bounded local subscript closure for
`FE_TitleScreenRoot`, `FE_MainMenu_PC`, `FE_NewGameMenu`,
`FE_LoadGameMenu`, `FE_OptionsMenu_PC` and `FE_QuitAppPrompt`.
It retains original command start/length, native serialized FPS,
controller index, event type and Entity/SubScript/Sound reference UID.
`sample` evaluates active timeline commands and child SubScript
frames using each script's own FPS. It fails closed on cycles,
unresolved references, invalid elapsed time or >24 nested scripts.

`game_mode.rs` now has `FrontendMenuNavigation`: per-screen cursor
selection, Up/Down wrap, Enter activation, Escape back/pause and mouse
selection that does not steal the keyboard cursor while stationary.
The F3 script inspector shows currently active commands, controller
indices, resource UIDs and unknown subscript/cycle counts; hidden
during normal gameplay. The frontend clock restarts upon each screen
transition and the GUI uses the real Script catalog for its F3 sample.

**Not yet native renderer parity.** `UXGeoScript` exposes animation
controller records, but the FE-specific controller matrices,
2D placement, HUD event dispatch, 3D backdrop, native font glyph
mapping and script loop/end semantics are not yet implemented.
The original chrome/bitmap textures remain actual EDB-decompressed
resources; their prior mirrored layout is still provisional.

For the font, `h07_font.edb` actual textures are available:
`0x86000002` (256×256 white variable glyph atlas),
`0x86000003` (256×32 green numerals), `0x86000004`
(256×512 atlas), `0x86000005/06/07` (controller icon strips).
Without proven EXGeoFont layout and glyph advances, do not invent a
monospace UV grid or label it the shipped bitmap renderer.

## 2026-10-13 original font atlas decoding

`menu_assets.rs` now has a bounded `read_original_font_atlas` decoder for
`h07_font.edb` (EDB `0x01000042`, texture `0x86000002`). It validates the
256×256 RGBA atlas and keeps the native glyph UV/advance layout explicitly
unresolved. The optional real-corpus regression test
`shipped_windows_font_atlas_decodes_when_corpus_present` passed using the
local original Windows resources. This is a decode seam, **not** a bitmap
font renderer; the menu still uses egui font rendering.

## EXGeoFont bounded binary inspection

The live H07 diagnostic now lists each font-directory entry's UID, section,
raw address and (only if the raw address is within the file) the first 32 bytes
at that literal offset. The byte prefix is deliberately uninterpreted: the
section-relative address resolution and glyph struct field boundaries remain
UNKNOWN. No bitmap glyph metrics or rendering parity are claimed.
The real-corpus font regression and GUI build passed after this change.

## Native Ghidra font path (2026-10-13)

The existing Robots.exe Ghidra project has now exposed a concrete Windows font
consumer at `0x00430620`: 16-bit text indexes a two-level glyph lookup through
relative pointers (`character >> 8` then `character & 0xff`). Glyph records have
consumed 16-bit metric fields at `+0x10`, `+0x12`, `+0x14`; measured width uses
`word[+0x10] - word[+0x14]`, and vertical extent `word[+0x12] - 3`.
The HUD Script text route at `0x0043be90` calls `0x0043db80` with
`HT_Font_Main` / `h07_font.edb`. Exact directory section address resolution,
glyph UV fields and glyph rendering pipeline are still unknown; no guessed
bitmap renderer is enabled. Native function identities are in `functions_map.md`.

## Native two-level glyph lookup implementation

`menu_assets.rs::native_font_glyph_offset` implements the native lookup shape
observed in `Robots.exe:0x00430620`: relative 32-bit pointers, high-byte page,
low-byte glyph and bounded record access. Synthetic regression covers present,
missing and invalid relative pointers; all five `menu_assets` tests pass.
The caller-supplied table base is **not yet resolved** from H07 in the live
resource path. Do not use the helper for game text presentation until native
resource base resolution and glyph UV fields are confirmed.

## Native glyph metric decode seam

`menu_assets.rs` now models the three glyph u16 fields consumed by the native
text layout at `0x00430620`. Its fail-closed accessor calculates a candidate
width as `u16[+0x10]-u16[+0x14]` and height as `u16[+0x12]-3`, with bounds and
underflow checks. This operates on an already-resolved glyph record; resolving
an EXGeoFont resource's serialized base offset and the texture UV fields remains
open. All six `app::menu_assets::tests` passed after this change.

## Native glyph render-record extraction (2026-10-13)

Ghidra `0x00430DD0` routes text glyphs into callbacks `0x00431390` and
`0x00431700`. At `0x004314F5..0x0043150A` the native code copies four
consecutive 32-bit words from the resolved glyph object (`+0,+4,+8,+12`)
into its render element, and scales 16-bit dimensions at `+0x10/+0x12`.
`menu_assets.rs` now has `native_font_glyph_render_record`, preserving these
four opaque words and validated metrics without inventing their UV encoding.
The native font resource base and four word interpretation must be confirmed
before replacing the egui menu glyph renderer. Targeted compilation began but
was interrupted by the command timeout; do not claim the new test passed.

## Native texture binding frontier (2026-10-13)

Further Ghidra traversal of `0x00430DD0` established callbacks `0x00431390`
and `0x00431700`. They copy the four opaque glyph dwords verbatim to draw
items and scale glyph dimensions separately. Render-resource binding
`0x0052E854` obtains a packed handle from `0x0052E8A8` and resolves
`(*handle & 0x3F) * 0x38 + page[(*handle >> 6)]` before placing it into
render context `+0xB4`. Neither this call nor the glyph copy establishes
which of the four dwords encode UVs. The original atlas cannot yet replace
egui text safely. See `functions_map.md` for exact source addresses.

## Original bitmap text rendered in Game Mode (2026-10-13)

**Implemented, real PC EDB verified.** `HT_Font_Main` (`0x07000001`)
at serialized H07 offset `0x4B1C` uses the two-level relative glyph lookup
from the native code. Its first u32 is a zero-based EDB texture-list index:
`2` -> `0x86000002`, the real 256×256 glyph atlas. Native glyph words
`+0x00,+0x04,+0x08,+0x0C` are normalized float32 UV bounds
`(u0,v0,u1,v1)`, independently verified on actual glyphs. For `'A'` the
UV bounds are `(144/255,28/255,160/255,56/255)`, draw size 16×28 and
advance 15. The binary parser retains the bounds and guards malformed
relative pointers, missing glyphs, invalid UVs and incorrect atlas selectors.

`RobotsMenuAssets::paint_original_label` now submits one original atlas
quad per supported glyph via `egui::Painter`; `game_mode::menu_button`
uses the native bitmap text while preserving original clickable button and
keyboard navigation semantics. For labels lacking glyphs, or unavailable H07
data, only the fallback egui label is drawn. The Main Menu's **font art is
original**; spacing, baseline alignment and global 2D placement remain
a provisional host layout rather than pixel-exact native FE HUD transforms.

The real-corpus GUI regression verifies that a `New Game` label emits
image meshes with the H07 atlas texture ID. All 7 active
`app::menu_assets::tests` passed (0 failures).

## Next verified priorities

1. Run **graphical** Windows smoke test of Title Screen/Main Menu/options/pause
   and missing/valid EDB; compare text baseline and highlight against the
   original Windows Robots 2005 reference.
2. Recover FE_MainMenu_PC/FE_TitleScreenRoot exact sprite transforms, event
   timing, subscripts and animated 3D backdrop; current frame remains provisional.
3. Recover native keyboard/controller selection transition/animation state,
   and audit Pause Menu separately.
4. Reconstruct native save/profile and chapter-start semantics; never relabel
   checkpoint v1 as native saves.
