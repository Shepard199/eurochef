# 👨‍🍳 Eurochef

_Cooking up some EDBs_

> **Robots fork.** This is the maintained PC fork of
> [eurotools/eurochef](https://github.com/eurotools/eurochef), focused on
> *Robots* (EDB version 248).

Eurochef provides tools and Rust crates for working with Eurocom EngineX(T)
files; including filelist, `.edb`, `.sfx` and `.elx` files.

See [docs/CLI_FAQ.md](docs/CLI_FAQ.md) for supported CLI commands and common
Robots workflows.

<p align="center">
  <img src="docs/Img/img1.png" width="32%" alt="Native Maps: lighting, NavMesh, particles and the Map Controls panel" />
  <img src="docs/Img/img2.png" width="32%" alt="Scripts: native AnimScript/Sound timeline with resolved local entity names" />
  <img src="docs/Img/img3.png" width="32%" alt="Animations tab: native full-bone skeletal playback with pose diagnostics" />
</p>
<p align="center">
  <sub><b>Maps</b> — native lighting & particles &nbsp;•&nbsp; <b>Scripts</b> — AnimScript/Sound timeline &nbsp;•&nbsp; <b>Animations</b> — native skeletal playback</sub>
</p>

## Features

* [x] Easy to use CLI Tool
* [x] Texture extractor
  * Supported output formats: png, qoi, tga
* [x] Entity extractor
* [x] Map extractor
  * [x] Blender plugin
* [x] Filelist re-packer
* [x] GUI viewer tool
  * [x] Native Robots map runtime (triggers, scripts, entities, vehicles, skies)
  * [x] Native Robots particle renderer (`EXParticleSys`)
  * [x] Native MapZone lighting + apparent-sun global lighting
  * [x] Native, spatialized PC MUSX sound preview, mixer and persistent cache
  * [x] Native skeletal animation playback (full-bone, CPU skinning)
  * [x] Native Script Animation/Sound timeline playback
  * [x] Trigger diagnostics, path-context visualization and mouse picking
  * [x] Runtime-created Monster/NPC character models (`MonsterDatabase` bridge)
* [x] Autodesk FBX character-model and animation export (`edb fbx-characters`)
* [x] Cross-EDB resource atlas & human-readable, deduplicated glTF resource library
* [ ] Filelist VFS
* [ ] Intermediate representation of EDB files
* [ ] EDB to Euroland 4 decompiler
* [ ] And more?

## Robots PC additions

This fork keeps the upstream Rust workspace and adds evidence-backed, reverse
engineered support for the PC release of *Robots*. Every feature below is
tied to instruction-level proof against the verified executable
`Robots.exe` (`SHA-256 8fefaa09767d9d1e76ca8c023e4e60720808cc529fc3abe1ff6d863d93f668bc`)
and validated against the full shipped 179/180-EDB corpus wherever practical.
Serialized EDB decoding itself does not depend on executable addresses.

### 🗺️ Maps, triggers, scripts & entities

<img src="docs/Img/img1.png" align="right" width="360" alt="Maps viewer with lighting, NavMesh and particle controls" />

* Maps are split into logical modules: native trigger behavior, specialised
  entity resolution and script behavior live in their own modules instead of
  one large map frame.
* AnimScript composition resolves local entities, external script
  dependencies, recursive `SubScript` and serialized timing. Cyclic
  `SubScript` branches are skipped without aborting the parent script.
* Controller slot tables are resolved directly from serialized command
  indices — never `thread_controller_count` or an assumed `index - 1` — so
  composite objects (vehicles, mechanisms, characters) keep every part.
* Every raw `AnimScript` opcode is now classified by native role: Controller
  Init, Dynamic Light, Camera, External Callback, Collision, Force Feedback,
  Loop, Controller Fan-out and Terminator. Payload lanes for the Dynamic
  Light, Camera and Collision commands are decoded from the exact native
  consumer, while every raw byte is preserved for provenance.
* Native `XTrigger_Platform`, `XTrigger_Lift` and `XTrigger_Vehicle` motion is
  reproduced from serialized path/speed/acceleration data: exact map speed,
  fixed-60 Hz acceleration ramp, ping-pong/loop behavior, tangent-based
  Vehicle steering yaw, and a manual/automatic `Native Event Gate` that now
  **defaults on** (matching the native constructor), so moving triggers wait
  at their serialized start position until a real activation instead of
  free-running from map load.
* Vehicle assemblies resolve body and wheel components through the original
  controller slots; road wheels roll and both drive/passive wheels steer
  using the exact native heading-delta/smoothing formula.
* **Runtime-created Monster/NPC geometry** is now rendered even though the
  game never serializes a `visual_object` for AI characters. The native
  `XTrigger_AI_Character` → `HT_SpreadSheet_MonsterDatabase` chain is fully
  reproduced: runtime family, `data[0]` row and character EDB are resolved,
  the matching local Animation Script is selected, and the implicit
  `skin_hashcode = 0xFFFFFFFF` sentinel ("use the Animation's own bound
  AnimSkin") is now honoured by static Map rendering, not just the dynamic
  Animation viewer. Proven end-to-end on the real `m02_city.edb` corpus:
  `77/77` character triggers resolve to an existing external EDB, Script and
  mesh closure (DogBot, SawBot, JailBot, TurretBot, Spider, Ticket Clerk,
  PiranhaBot, and more).
* **`EXGeoMap.skies` zone-selected sky assemblies** are rendered natively:
  the exact per-`MapZone` AABB selector (serialized order, zone-0 fallback,
  `-1` = no assembly) picks the active sky Script, camera-relative members
  keep the inherited camera root while map-space facade members keep their
  serialized transform, and a persistent background-only base sky now fills
  zones that explicitly opt out of a foreground assembly (e.g. Hub1 zones
  `29`/`30`). Sky geometry renders in a dedicated early unlit pass (with
  depth writes preserved for map-space assemblies) so it neither fights
  ordinary scene lighting nor gets overdrawn by its own children.
* Triangle-strip flag `0x10` no longer hides geometry by default; visibility
  is controlled explicitly through the **`Geometry with strip flag 0x10`**
  Maps toggle, independent from the unrelated object-level sky flag.
* `XTrigger_Platform`/gear rotation now composes `base * delta` in the
  body-local axis instead of world space, so serialized local-Z gear spins
  stay on their physical rotation axis; Script-driven gears use their own
  quaternion channels, and native infinite-loop opcode `16` timelines
  (`repeat = -1`) jump to their exact serialized target frame instead of
  wrapping the full clip duration.
* Path-context is recovered (without inventing traversal) for Camera, Camera
  Marker, Watchbot, BossRatchet, Monster Transporter and Monster; a false
  BossSewer path match was instruction-proven and explicitly rejected.
* NPC (mission/tutorial/cutscene context) and Monster/Test/Fish
  (proximity/path/flag context) native trigger getters are recovered and
  shown as diagnostics.
* `XItemPhysics_Platform` native contact-carry (linear velocity transfer) is
  reproduced for Platform/Lift/Vehicle; full contact-point/manifold physics
  remains a separate task.
* A `146`-class runtime census (`XItemHandler*`, `XItemPhysics*`,
  `EXItemAnimator*`, `EXItemRender*`) with exact descriptors/vtables/xrefs
  drives ongoing behavior recovery; `XItemPhysicsSphere` and PlayerBall
  slippery/electric/slime surface reactions are instruction-proven.
* **`XTrigger_ObjectAudio`** object sound profiles are fully decoded as a
  four-slot data profile (Activate/Deactivate one-shots, Active/Inactive
  loops) with the exact native event choreography for Hazard, Fan,
  FanHorizontal, Platform, Lift, Clock and Vehicle consumers, including each
  consumer's class-specific fallback table and enable-bit gate.
* Dedicated, correctly-scaled trigger and sound icons (Player, Camera,
  Script, ChangeLevel, Door, NPC, Mission, ObjectAudio and more) are bound
  through the serialized trigger typemap.
* Trigger mouse picking uses a reusable, cleared, isolated-state RGBA8
  pick framebuffer.
* NavMesh (`0x606`/`0x607`) geometry is preserved and independently
  toggleable with a world-space UV scale.

### 💡 Lighting

* `EXGeoLight` MapZone lighting reproduces the exact native `ltype` feature
  mask, range/cone falloff and the true `byte/128` RGB scale (not `/255`).
* A separate `Global Lighting` stage reproduces the apparent outdoor sun:
  15 known level-UID coefficient tables, three live smoothed directional
  slots, spatial per-position world-light sampling from MapZone vertex
  colours, and the executable fallback for unmapped positions.
* `Vertex Lighting` toggles the full contribution without rebuilding map
  geometry.

### ✨ Particles

* Native `EXParticleSys` is fully reproduced: fixed-step (`1/60` or `1/30`)
  simulation, emission rate, pool limits, lifetime, box emitter, velocity
  distribution, damping/acceleration, material/blend selection and the
  appended curve table (rotation, scale, RGBA, speed-offset, initial age).
* Validated against the full shipped corpus: 179/179 EDB, 321 particle
  systems, 10,804 curve records, 0 out-of-range material selectors.
* Remaining boundary: deterministic per-emitter preview seeding vs. the
  game's shared global RNG schedule.

### 🔊 Sound

* The GUI decodes PC MUSX data in-process through a native Eurocom IMA
  ADPCM decoder: soundbank waves, streams, music and SubSfx dependencies.
  No external `.NET EuroSoundBridge` process is required at runtime for
  playback (a thin headless `.NET 8` bridge is used only to source the
  original bank-parsing logic).
* Playable map and external-script sound references are preloaded into a
  persistent, worker-backed WAV cache when an EDB is opened; the cache now
  also carries a JSON metadata sidecar so legacy WAV-only entries recover
  their soundbank properties instead of silently losing them.
* **Native SFX spatial radius** is now applied uniformly to ObjectAudio
  one-shots/loops and Map Script Sound events: the shipped
  `usa__sounddetails.sfx` table (inner/outer radius, loop state, Is3D,
  streamed state) plus MusX v5 bank fields (master volume, priority,
  TrackingType) drive `master volume × linear inner/outer radius gain`,
  listener-local pan and correct voice creation/teardown at the audible
  boundary. 2D SFX stay centered at fixed master volume; map ambient sounds
  keep their own authoritative serialized volume/radius/pan/fade behavior.
* Maps automatically plays ambient `EXGeoMapZone` sounds from the fly camera
  listener with volume/radius attenuation, panning and fades.
* **Script Sound playback parity**: Maps now enumerates the *same* animated
  Script instance families the renderer draws — direct placements, trigger
  visual Scripts, assembly Scripts resolved from visual Entities and
  recursive `SubScript` — instead of only direct placements, and prefetches
  every referenced sound ahead of its (often 5–12 frame) window so short
  cues are no longer dropped by asynchronous decode races. Map Script audio
  now lives in its own `SoundVoiceGroup::MapScript`, so pausing or seeking
  the standalone Scripts panel can no longer affect Maps playback.
* Validated on the real game corpus: 13 soundbanks, 2487 SFX, 4218
  sample-pool entries, 3057 waves, 0 failures.

### 🎬 Animation

<img src="docs/Img/img3.png" align="right" width="360" alt="Animations tab with native full-bone playback and pose diagnostics" />

* Full native skeletal playback: complete active-bone decoding, hierarchy,
  bind poses, serialized one-to-four bone weights, position/quaternion
  interpolation and CPU skinning, driven by a reproducible offline pose
  cache built from the verified executable.
* `Animation.skin_num == AnimSkinHeader.base_skin_num` is the proven
  Animation-to-AnimSkin binding rule (not an array index); `0xFFFFFFFF` is
  the explicit no-skin / "use the asset's own bound skin" sentinel, now
  honoured consistently by the `Animations` viewer, `Scripts` timeline
  playback **and** static Map rendering of runtime-created characters.
* Full corpus binding census: 179/179 EDB, 1744 Animation records, 234
  AnimSkin records, 1390 exact native bindings, 354 no-skin sentinels,
  0 unresolved.
* A dedicated `Animations` tab exposes searchable clips, timeline scrub,
  Play/Pause/Loop/Speed, frame stepping and full diagnostic metadata.
* `Scripts` plays native Animation commands on the real AnimScript timeline
  (including nested `SubScript` and the implicit-skin sentinel), as seen
  in the Scripts preview above.

### 🎭 FBX export

* `eurochef-cli edb fbx-characters` exports native Robots characters to
  Autodesk FBX SDK 2020.x, driven by the same proven `AnimSkin`/`Animation`/
  `AnimScript`/pose-cache pipeline as the GUI: one Skeletal Mesh FBX per
  AnimSkin (original triangles, materials, normals, UV0, vertex colours) and
  one animation-only FBX per bound Animation clip, preserving bone names,
  hierarchy, bind pose and every decoded source pose frame as a linear key.
* Exact gameplay timing (`duration = command.length / script.framerate`) is
  recovered from the consuming AnimScript, not assumed from a fixed FPS;
  the Animation, its Script and its target AnimSkin may each live in a
  different EDB, resolved independently via `--script-manifest`.
* Multi-root AnimSkin assets receive one deterministic `EuroChefRoot` for
  Unreal-style single-skeleton import; single-root assets are untouched.
* Rebuilt native pose corpus: `1750/1750` Animation/AnimSkin pairs resolved
  with `0` failures (the previous `1390` native bindings plus `360`
  Script-resolved non-native bindings across EDB boundaries).

### 🗂️ Resource tooling & pipeline

* **Canonical resource labels** (`decoded name [0xUID]`) are used
  consistently across Texture, Animation/AnimSkin, Script and Entity lists,
  selection headers, tooltips and previews, resolving known names through
  the shipped Robots HashDB while always retaining the exact UID.
* `eurochef-cli edb resource-atlas` scans Textures, Animations, AnimSkins,
  AnimScripts and Entities across the whole game and writes
  [`docs/ROBOTS_RESOURCE_ATLAS.md`](docs/ROBOTS_RESOURCE_ATLAS.md): a
  cross-EDB table of shared global resources plus complete per-kind tables,
  correctly keeping same-numbered local resources (`0x82/83/84/86...`)
  separate per owning EDB. Current real corpus: 180 EDB files, 9,634 unique
  scoped resources, 10,467 occurrences, 0 scan errors.
* A **human-readable, deduplicated glTF resource library** replaces the old
  SHA-only content store: exported resources live under
  `_eurotools_out/edb/_shared/gltf_library/by_edb/<n>_<source.edb>/`, named
  with the decoded HashDB label plus UID (e.g.
  `HT_Entity_Vehicle_Taxi_Collision_[0x020001B4].gltf`); a byte-identical
  resource is stored exactly once, with no hardlinks/symlinks — only index
  files track aliases. Real corpus results: 3,829 aliases deduplicated down
  to 2,651 unique canonical `.gltf` documents and 8,341 unique canonical
  files overall, cutting referenced resource bytes from ~1.27 GB to ~322 MB.

## Current status

### ✅ Working

* Robots PC EDB textures, entities, maps, scripts and vehicle assemblies.
* Native map rendering: trigger/entity/script composition, GPU global
  lighting, MapZone lighting, zone-selected sky assemblies, NavMesh and
  mouse-pick trigger selection.
* Native `EXParticleSys` particle rendering (see boundary note above).
* Native, spatialized PC MUSX decoding, mixing and preview for soundbanks,
  streams, music and SubSfx, including automatic Map ambience, ObjectAudio
  and Script timeline playback with correct radius-based voice gating.
* Full native AnimSkin skeletal playback (hierarchy, weights, interpolation,
  CPU skinning) for the proven Animation-to-AnimSkin binding set, in the
  standalone `Animations` tab, `Scripts` timeline playback **and** static
  Map rendering of runtime-created Monster/NPC characters.
* Runtime-created Monster/NPC character geometry resolved live from the
  shipped `MonsterDatabase` spreadsheet, matching the game's own
  data-driven model instantiation.
* Native Platform/Lift/Vehicle path motion (now gated off by default at
  spawn, matching the native constructor), event gating, contact carry and
  body-local gear rotation; Camera/Camera Marker/Watchbot/BossRatchet/
  Transporter/Monster/NPC path and trigger-context diagnostics.
* Autodesk FBX character-model and animation export for the full proven
  binding corpus (native + Script-resolved cross-EDB pairs).
* Cross-EDB resource atlas and deduplicated, human-readable glTF resource
  library for the exported asset pipeline.
* CLI extraction, reports and Filelist v7 operations listed in
  [CLI FAQ](docs/CLI_FAQ.md), including `edb trigger-report`,
  `edb script-health`, `edb entity-report`, `edb anim-binding-report`,
  `edb fbx-characters` and `edb resource-atlas`.

### 🚧 Work in progress

* Animation records outside the proven Animation-to-AnimSkin binding set
  remain on the diagnostic (non-deforming) path where no Script timing
  reference exists; cross-clip blending, layered controller state, exact
  root-motion ownership and runtime event coupling are not yet implemented.
* Physics/collision response beyond linear contact carry — real contact
  point, manifolds, impulses, friction and damage handoff — is not yet
  reproduced.
* The majority of `XItemHandler` gameplay subclasses (Monster AI/combat,
  Player/input, Projectile damage, class-specific boss/interactive state
  machines) remain unresolved beyond structural descriptors, resolved
  runtime models and path/trigger context diagnostics.
* Native Camera event ownership and controller plans are exposed, but exact
  player-state interpolation/projection, portal/load transitions and native
  zone fog/camera/effect composition are not yet simulated.
* Audio voice limits, priority replacement, exact PCAUDIO RNG, signed
  millisecond delays, random sample selection, MultiSample/Shuffled and
  Polyphonic scheduling are implemented. Cross-cycle negative-delay overlap,
  process-global RNG interleaving, occlusion, random-position tracking and
  DirectSound reverb DSP remain open; shipped Doppler profiles are all zero
  even though its backend consumer is instruction-proven.
* Many trigger `data[]` fields remain preserved as raw, unproven values even
  where the trigger type itself has a typemap entry — for example a real
  City audit found only 644 of 1,821 populated occurrences currently have a
  named/proven meaning; the rest are structurally parsed but semantically
  unrecovered.
* The GUI remains a work in progress; not every EngineX trigger, entity,
  script or platform has a native runtime implementation.

## Support Matrix

### Robots (EDB version 248)

| Textures <sup>[1]</sup> | Maps | Scripts | Entities | Animations <sup>[2]</sup> | Sounds <sup>[3]</sup> | Particles <sup>[4]</sup> | Spreadsheets |
| :----------------------: | :--: | :-----: | :------: | :-------------------------: | :----------------------: | :-------------------------: | :-----------: |
| ✅/❌                     | ✅/❌  | ✅/❌     | ✅/❌      | ✅/❌                        | ✅/❌                    | ✅/❌                        | ✅/❌           |

<sup>[1]</sup> Texture/entity support indicates the ability to read headers
and frame data.

<sup>[2]</sup> Native animation support covers the proven
Animation-to-AnimSkin binding set (`1390/1744` shipped clips natively, plus
`360` additional Script-resolved cross-EDB clips available through FBX
export) with full-bone decode, interpolation and CPU skinning. Unbound
clips remain diagnostic.

<sup>[3]</sup> Native sound support covers PC MUSX decoding, mixing and
persistent cache generation for soundbanks, streams, music and SubSfx, plus
automatic Map ambience, ObjectAudio and Script timeline playback with native
3D radius attenuation.

<sup>[4]</sup> Native `EXParticleSys` rendering covers timing, emission,
lifetime, materials and curve channels; individual particle placement uses a
deterministic per-emitter seed rather than the game's shared RNG schedule.

_Each field is formatted as R/W. For example, if a feature can be read but not
written, the field is shown as ✅/❌._

### PC platform

| Platform | Endian | Textures | Sounds <sup>[1]</sup> | Mesh | Support status |
| :------: | :----: | :------: | :---------------------: | :--: | :-------------: |
| PC       | LE     | ✅/❌      | ✅/❌                    | ✅/❌ | ✅             |

<sup>[1]</sup> Sound support is currently specific to the PC release of
*Robots* and the GUI preview/cache path.

### Filelists

| Game   | Version | Read | Write |
| :----- | :-----: | :--: | :---: |
| Robots | v7      | ✅    | ✅     |

<!-- ## Map extracting -->
<!-- TODO(cohae): Write this out into a guide on how to build/use CLI/GUI, not just for maps but also everything else -->
