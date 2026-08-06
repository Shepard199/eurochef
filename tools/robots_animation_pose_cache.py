from __future__ import annotations

import argparse
import json
import math
import os
import struct
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
LOCAL_DEPENDENCIES = PROJECT_ROOT / "target/python/animation-oracle"
if LOCAL_DEPENDENCIES.is_dir():
    sys.path.insert(0, str(LOCAL_DEPENDENCIES))

try:
    from robots_animation_native_oracle import (  # noqa: E402
        CACHE_HEADER,
        CACHE_MAGIC,
        CACHE_POSE,
        CONTEXT_ADDRESS,
        ORIGINAL_EDB_BASE,
        RobotsAnimationOracle,
        align_up,
        assemble_pose,
        fnv1a64,
        load_requests,
        load_script_bound_requests,
        prepare_runtime_cache_entries,
    )
except ModuleNotFoundError as exc:
    if exc.name in {"pefile", "unicorn"}:
        raise SystemExit(
            "Missing animation-oracle dependencies. Install them locally with: "
            "py -3.13 -m pip install --no-cache-dir --target "
            "target/python/animation-oracle -r tools/animation-oracle-requirements.txt"
        ) from exc
    raise


# Keep the mapped EDB inside the decoder's signed 24-bit relative-pointer window.
# The original probe base sits one page too low for records targeting the first
# bytes of an EDB (for example ep02_tur animation 2).
POSE_EDB_BASE = ORIGINAL_EDB_BASE + 0x1000


def relocate_mapped_animation(
    oracle: RobotsAnimationOracle,
    request,
    edb: bytes,
) -> None:
    relocated = bytearray(oracle.anim)
    serialized = edb[request.animation_offset : request.animation_offset + len(relocated)]
    if len(serialized) != len(relocated):
        raise ValueError("serialized Animation object is truncated")

    for field_offset in (0x30, 0x34):
        relative = struct.unpack_from("<i", serialized, field_offset)[0]
        if relative == 0:
            struct.pack_into("<i", relocated, field_offset, 0)
            continue
        target_offset = request.animation_offset + field_offset + relative
        if not 0 <= target_offset < len(edb):
            raise ValueError(
                f"relptr +0x{field_offset:02X} target 0x{target_offset:X} outside EDB"
            )
        runtime_relative = POSE_EDB_BASE + target_offset - (CONTEXT_ADDRESS + field_offset)
        struct.pack_into("<i", relocated, field_offset, runtime_relative)

    packed = struct.unpack_from("<I", serialized, 0x38)[0]
    flags = packed & 0xFF
    relative = packed >> 8
    if relative & 0x0080_0000:
        relative -= 0x0100_0000
    if relative == 0:
        struct.pack_into("<I", relocated, 0x38, flags)
    else:
        target_offset = request.animation_offset + 0x38 + relative
        if not 0 <= target_offset < len(edb):
            raise ValueError(f"packed relptr +0x38 target 0x{target_offset:X} outside EDB")
        runtime_relative = POSE_EDB_BASE + target_offset - (CONTEXT_ADDRESS + 0x38)
        if not -(1 << 23) <= runtime_relative < (1 << 23):
            raise ValueError(
                f"packed relptr +0x38 relocation {runtime_relative} exceeds signed 24-bit"
            )
        struct.pack_into(
            "<I",
            relocated,
            0x38,
            ((runtime_relative & 0x00FF_FFFF) << 8) | flags,
        )

    struct.pack_into("<I", relocated, 0x1C, 0)
    oracle.anim = bytes(relocated)


def validate_existing_cache(path: Path, request, motion_checksum: int, frames: int, bones: int) -> bool:
    try:
        expected_size = CACHE_HEADER.size + frames * bones * CACHE_POSE.size
        if path.stat().st_size != expected_size:
            return False
        with path.open("rb") as handle:
            header = handle.read(CACHE_HEADER.size)
        if len(header) != CACHE_HEADER.size:
            return False
        (
            magic,
            edb_uid,
            animation_index,
            animation_hashcode,
            animskin_hashcode,
            cached_frames,
            cached_bones,
            cached_checksum,
        ) = CACHE_HEADER.unpack(header)
        return (
            magic == CACHE_MAGIC
            and edb_uid == request.edb_uid
            and animation_index == request.animation_index
            and animation_hashcode == request.animation_hashcode
            and animskin_hashcode == request.animskin_hashcode
            and cached_frames == frames
            and cached_bones == bones
            and cached_checksum == motion_checksum
        )
    except OSError:
        return False


def validate_poses(poses: list[tuple[float, ...]], frame: int) -> None:
    for bone_index, pose in enumerate(poses):
        if not all(math.isfinite(value) for value in pose):
            raise ValueError(f"non-finite pose frame={frame} bone={bone_index}")
        length = math.sqrt(sum(value * value for value in pose[3:7]))
        if abs(length - 1.0) > 2.0e-3:
            raise ValueError(
                f"non-unit quaternion frame={frame} bone={bone_index} length={length}"
            )


def generate_clip(
    oracle: RobotsAnimationOracle,
    request,
    edb: bytes,
    output_root: Path,
    force: bool,
) -> dict[str, object]:
    started = time.perf_counter()
    motion_checksum = oracle.configure(request)
    relocate_mapped_animation(oracle, request, edb)
    frame_count = oracle.frame_count
    bone_count = oracle.bone_count
    if frame_count <= 0 or bone_count <= 0:
        raise ValueError(f"invalid clip dimensions frames={frame_count} bones={bone_count}")

    output_dir = output_root / f"{request.edb_uid:08X}"
    output_dir.mkdir(parents=True, exist_ok=True)
    output_name = (
        f"{request.animation_index:04}_[0x{request.animskin_hashcode:08X}].rapc"
        if request.variant_cache
        else f"{request.animation_index:04}.rapc"
    )
    output_path = output_dir / output_name
    if not force and output_path.is_file() and validate_existing_cache(
        output_path,
        request,
        motion_checksum,
        frame_count,
        bone_count,
    ):
        return {
            "edb_uid": f"0x{request.edb_uid:08X}",
            "animation_index": request.animation_index,
            "animation_hashcode": f"0x{request.animation_hashcode:08X}",
            "animskin_source_edb_uid": f"0x{request.animskin_source_edb_uid:08X}",
            "animskin_hashcode": f"0x{request.animskin_hashcode:08X}",
            "animskin_path": str(request.animskin_path),
            "cache_variant": request.variant_cache,
            "frames": frame_count,
            "bones": bone_count,
            "bytes": output_path.stat().st_size,
            "seconds": time.perf_counter() - started,
            "status": "reused",
            "path": str(output_path),
        }

    temporary_path = output_path.with_suffix(".rapc.tmp")
    correction_key = struct.unpack_from("<H", oracle.anim, 0x10)[0] & 0x7F00
    chunk_size = oracle.anim[0x0D]
    if chunk_size == 0:
        raise ValueError("zero native cache chunk size")

    with temporary_path.open("wb") as output:
        output.write(
            CACHE_HEADER.pack(
                CACHE_MAGIC,
                request.edb_uid,
                request.animation_index,
                request.animation_hashcode,
                request.animskin_hashcode,
                frame_count,
                bone_count,
                motion_checksum,
            )
        )

        for group_start in range(0, frame_count, chunk_size):
            group_end = min(group_start + chunk_size, frame_count)
            frames = list(range(group_start, group_end))
            decode_frames = list(frames)
            if group_end < frame_count:
                decode_frames.append(group_end)
            entries, _ = prepare_runtime_cache_entries(
                oracle,
                decode_frames,
                correction_key,
                request.motion_size,
            )
            for frame in frames:
                next_frame = min(frame + 1, frame_count - 1)
                poses = assemble_pose(
                    oracle,
                    entries[frame],
                    entries[next_frame],
                    0.0,
                )
                validate_poses(poses, frame)
                for pose in poses:
                    output.write(CACHE_POSE.pack(*pose))

    expected_size = CACHE_HEADER.size + frame_count * bone_count * CACHE_POSE.size
    if temporary_path.stat().st_size != expected_size:
        raise ValueError(
            f"cache size mismatch {temporary_path.stat().st_size} != {expected_size}"
        )
    temporary_path.replace(output_path)
    return {
        "edb_uid": f"0x{request.edb_uid:08X}",
        "animation_index": request.animation_index,
        "animation_hashcode": f"0x{request.animation_hashcode:08X}",
        "animskin_source_edb_uid": f"0x{request.animskin_source_edb_uid:08X}",
        "animskin_hashcode": f"0x{request.animskin_hashcode:08X}",
        "animskin_path": str(request.animskin_path),
        "cache_variant": request.variant_cache,
        "frames": frame_count,
        "bones": bone_count,
        "bytes": output_path.stat().st_size,
        "seconds": time.perf_counter() - started,
        "status": "generated",
        "path": str(output_path),
    }


def generate_file_job(
    executable: Path,
    edb_uid: int,
    requests,
    output_root: Path,
    force: bool,
) -> tuple[int, str, list[dict[str, object]], list[dict[str, object]]]:
    edb_path = requests[0].edb_path
    edb = edb_path.read_bytes()
    oracle = RobotsAnimationOracle(executable)
    oracle.uc.mem_map(POSE_EDB_BASE, align_up(len(edb)))
    oracle.uc.mem_write(POSE_EDB_BASE, edb)
    summaries: list[dict[str, object]] = []
    failures: list[dict[str, object]] = []
    for request in requests:
        try:
            summaries.append(generate_clip(oracle, request, edb, output_root, force))
        except Exception as exc:
            failures.append(
                {
                    "edb_uid": f"0x{request.edb_uid:08X}",
                    "animation_index": request.animation_index,
                    "animation_hashcode": f"0x{request.animation_hashcode:08X}",
                    "animskin_source_edb_uid": (
                        f"0x{request.animskin_source_edb_uid:08X}"
                    ),
                    "animskin_hashcode": f"0x{request.animskin_hashcode:08X}",
                    "animskin_path": str(request.animskin_path),
                    "cache_variant": request.variant_cache,
                    "error": f"{type(exc).__name__}: {exc}",
                }
            )
    return edb_uid, edb_path.name, summaries, failures


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate native Robots Animation pose sidecar caches for EuroChef."
    )
    parser.add_argument("executable", type=Path)
    parser.add_argument("binding_tsv", type=Path)
    parser.add_argument("animskin_tsv", type=Path)
    parser.add_argument("output_root", type=Path)
    parser.add_argument(
        "--script-health-report",
        type=Path,
        help="script_health_report.json used to add explicit cross-EDB Animation/AnimSkin pairs",
    )
    parser.add_argument(
        "--script-bound-only",
        action="store_true",
        help="generate only Script-resolved non-native Animation/AnimSkin pairs",
    )
    parser.add_argument("--edb-uid", type=lambda value: int(value, 0))
    parser.add_argument("--start-edb-uid", type=lambda value: int(value, 0))
    parser.add_argument("--max-files", type=int)
    parser.add_argument("--max-clips", type=int)
    parser.add_argument("--force", action="store_true")
    parser.add_argument(
        "--workers",
        type=int,
        default=max(1, min(12, os.cpu_count() or 1)),
        help="Parallel EDB workers. Each worker owns an isolated Unicorn runtime.",
    )
    args = parser.parse_args()

    if args.script_bound_only and args.script_health_report is None:
        parser.error("--script-bound-only requires --script-health-report")
    grouped = (
        {}
        if args.script_bound_only
        else load_requests(args.binding_tsv, args.animskin_tsv)
    )
    if args.script_health_report is not None:
        script_grouped = load_script_bound_requests(
            args.binding_tsv,
            args.animskin_tsv,
            args.script_health_report,
        )
        for edb_uid, requests in script_grouped.items():
            grouped.setdefault(edb_uid, []).extend(requests)
        for requests in grouped.values():
            requests.sort(
                key=lambda request: (
                    request.animation_index,
                    request.animskin_source_edb_uid,
                    request.animskin_hashcode,
                )
            )
    selected_uids = sorted(grouped)
    if args.edb_uid is not None:
        selected_uids = [args.edb_uid]
    elif args.start_edb_uid is not None:
        selected_uids = [uid for uid in selected_uids if uid >= args.start_edb_uid]
    if args.max_files is not None:
        selected_uids = selected_uids[: args.max_files]

    args.output_root.mkdir(parents=True, exist_ok=True)
    summaries: list[dict[str, object]] = []
    failures: list[dict[str, object]] = []
    remaining_clips = args.max_clips
    total_started = time.perf_counter()
    jobs = []
    for edb_uid in selected_uids:
        requests = grouped.get(edb_uid, [])
        if remaining_clips is not None:
            requests = requests[: max(0, remaining_clips)]
        if not requests:
            continue
        jobs.append((args.executable, edb_uid, requests, args.output_root, args.force))
        if remaining_clips is not None:
            remaining_clips -= len(requests)
            if remaining_clips <= 0:
                break

    workers = max(1, min(args.workers, len(jobs) or 1))
    if workers == 1:
        results = [generate_file_job(*job) for job in jobs]
    else:
        results = []
        with ProcessPoolExecutor(max_workers=workers) as executor:
            futures = [executor.submit(generate_file_job, *job) for job in jobs]
            for completed, future in enumerate(as_completed(futures), start=1):
                result = future.result()
                results.append(result)
                edb_uid, edb_name, file_summaries, file_failures = result
                print(
                    f"[{completed}/{len(futures)}] 0x{edb_uid:08X} "
                    f"file={edb_name} succeeded={len(file_summaries)} failed={len(file_failures)}",
                    flush=True,
                )

    for file_ordinal, result in enumerate(sorted(results, key=lambda row: row[0]), start=1):
        edb_uid, edb_name, file_summaries, file_failures = result
        if workers == 1:
            print(
                f"[{file_ordinal}/{len(results)}] 0x{edb_uid:08X} "
                f"file={edb_name} succeeded={len(file_summaries)} failed={len(file_failures)}",
                flush=True,
            )
        summaries.extend(file_summaries)
        failures.extend(file_failures)
        for summary in file_summaries:
            print(
                f"  animation={summary['animation_index']} frames={summary['frames']} "
                f"bones={summary['bones']} status={summary['status']} "
                f"seconds={summary['seconds']:.3f}",
                flush=True,
            )
        for failure in file_failures:
            print(f"  [FAIL] {failure}", flush=True)

    report = {
        "executable": str(args.executable),
        "executable_sha256": __import__("hashlib").sha256(
            args.executable.read_bytes()
        ).hexdigest(),
        "output_root": str(args.output_root),
        "generated": sum(row["status"] == "generated" for row in summaries),
        "reused": sum(row["status"] == "reused" for row in summaries),
        "succeeded": len(summaries),
        "failed": len(failures),
        "bytes": sum(int(row["bytes"]) for row in summaries),
        "elapsed_seconds": time.perf_counter() - total_started,
        "failures": failures,
        "clips": summaries,
    }
    (args.output_root / "pose_cache_summary.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                key: report[key]
                for key in (
                    "generated",
                    "reused",
                    "succeeded",
                    "failed",
                    "bytes",
                    "elapsed_seconds",
                    "output_root",
                )
            },
            indent=2,
        )
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
