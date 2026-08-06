# EuroChef Autodesk FBX character and animation exporter

This helper writes Robots character models and animation-only files through the official Autodesk FBX SDK 2020.x. The SDK is an external proprietary dependency and must not be committed to this repository.

## Build

1. Install Autodesk FBX SDK 2020.x under `_tools\FBX SDK\<version>` or set `FBX_SDK_ROOT` to another installation root.
2. Run:

```cmd
BUILD_FBX_EXPORTER.cmd
```

The launcher accepts either the exact SDK version directory or its parent and selects the newest versioned child containing `include\fbxsdk.h`. In this workspace it resolves `_tools\FBX SDK\2020.3.10` automatically, so no environment variable is required.

The helper is placed at:

```text
target\release\tools\fbx\fbx_export_helper.exe
```

The build fails explicitly when `FBX_SDK_ROOT`, `fbxsdk.h`, or the Windows x64 release library is missing. No Blender, Assimp, ASCII-FBX, or hidden fallback writer is used.

## Export models and animations

```cmd
CARGO.cmd run --package eurochef-cli -- edb fbx-characters path\character.edb output\characters --overwrite
```

Optional development-only IR validation without invoking the SDK:

```cmd
CARGO.cmd run --package eurochef-cli -- edb fbx-characters path\character.edb output\characters --ir-only --overwrite
```

## Cross-EDB Script bindings

A Script command may live in a map EDB, reference an Animation from that map or another EDB, and bind it to an AnimSkin from a character EDB. These are three independent owners and must not be collapsed into one file.

Generate a full Script resolution report from the corpus manifest, then extend the native pose cache with every non-native Animation/AnimSkin pair:

```cmd
CARGO.cmd run --package eurochef-cli -- edb script-health path\manifest.tsv target\script_health
python tools\robots_animation_pose_cache.py path\Robots.exe target\anim_binding_corpus_v1\animation_skin_bindings.tsv target\anim_binding_corpus_v1\animskin_rows.tsv target\robots_animation_pose_cache --script-health-report target\script_health\script_health_report.json --workers 32
```

Script-bound variants are stored without colliding with the Animation's native skin cache:

```text
<animation_edb_uid>\<animation_index>_[0x<animskin_uid>].rapc
```

Export the target character EDB with the same corpus manifest:

```cmd
CARGO.cmd run --package eurochef-cli -- edb fbx-characters path\character.edb output\characters --script-manifest path\manifest.tsv --overwrite
```

The exporter resolves and records the Script EDB, Animation source EDB, AnimSkin source EDB, Script FPS, command length and exact pose-cache path. External source EDB UIDs are included in output filenames so equal local Animation UIDs cannot overwrite each other.

Each AnimSkin produces one model FBX:

```text
<animskin_name>_[0xUID]_SK.fbx
<animskin_name>_[0xUID]_SK.fbx.report.json
```

Each Animation with a validated `RAPCV002` pose cache and a valid in-EDB AnimScript timing reference produces a separate animation-only FBX:

```text
<animskin_name>_[0xUID]__<animation_name>_[0xUID].fbx
<animskin_name>_[0xUID]__<animation_name>_[0xUID].fbx.report.json
```

If the same Animation is used with several distinct serialized FPS/duration pairs, one file is emitted per timing variant. The manifest records the source Script UID, command index, serialized FPS, command length, usage count, pose-cache path, frame count, duration, output file, and report file.

An Animation asset does not serialize an independent intrinsic FPS. The exact gameplay duration comes from AnimScript:

```text
duration_seconds = command.length / script.framerate
```

The FBX custom frame rate remains the serialized Script FPS. All decoded pose frames are written as keys and distributed across the exact gameplay duration, so the first pose is at time zero and the last pose is at the command end. No implicit 30 FPS, frame removal, key reduction, or dropped final frame is allowed.

Animations with no valid in-EDB timing reference are not guessed. They are listed under `skipped_animations` in the manifest. A deliberate override is available for forensic or manual use:

```cmd
--unreferenced-animation-fps 30
```

With `--keep-ir`, the canonical files are retained under `.fbx-ir`:

```text
*.ecfbx
*.fbxscene.json
```

## Model FBX contract

The model FBX contains:

- binary FBX selected through the SDK writer registry;
- centimeters;
- `MayaZUp`, `+Z` up and `-Y` front;
- component meshes without welding or topology optimization;
- source triangle/material order;
- normals, UV0, RGBA vertex colors;
- AnimSkin hierarchy and bind pose;
- normalized source skin weights through `FbxSkin` and `FbxCluster`;
- no animation stack, embedded textures, generated tangents, or generated physics assets.

## Animation FBX contract

Each animation-only FBX contains:

- the same bone names, parent hierarchy, local bind translations, axis system, units, and bind pose as its model FBX;
- no mesh, material, texture, or skin geometry;
- exactly one `FbxAnimStack` and one base `FbxAnimLayer`;
- translation X/Y/Z, rotation X/Y/Z, and scale X/Y/Z curves for every bone;
- exactly one key per decoded source pose frame on every curve;
- linear interpolation with no SDK key reduction;
- source local translations and rotations, including root tracks, without root-motion extraction;
- a custom FBX frame rate equal to the serialized AnimScript FPS;
- a time span equal to `command.length / script.framerate`.

Source coordinates are converted once as:

```text
(x, y, z) -> (-x, -z, y) * 100
```

The same basis conversion is applied to animation translations and quaternion rotations. Quaternion signs are made continuous before conversion, target Euler XYZ curves are unwrapped by whole turns, and the first and last source poses remain intact.

The reflection is matched by one model winding reversal. Mesh positions, normals, bone local positions, global bind positions, cluster matrices, and animation tracks use the same basis.

If an AnimSkin contains several source roots, one deterministic unweighted `EuroChefRoot` is inserted because Unreal requires one logical skeleton root. The same root is inserted into every animation frame. Source bone hierarchy, bind positions, rotations, and weights remain unchanged apart from the necessary one-slot index offset.

## Validation

After model export, the helper reimports the FBX through Autodesk FBX SDK and verifies bone, vertex, triangle, cluster, and bind-pose counts.

After animation export, it reimports the FBX and verifies:

- one animation stack and one animation layer;
- complete skeleton and bind pose;
- custom frame rate and exact time span;
- nine curves per bone;
- exact key count on every curve;
- first and last key time/value for translation, rotation, and scale;
- total curve and key counts.

A report is written only after the matching round-trip validation succeeds.
