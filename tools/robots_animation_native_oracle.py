from __future__ import annotations

import csv
import struct
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_HOOK_CODE, UC_HOOK_MEM_INVALID, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

PAGE = 0x1000
HEAP_BASE = 0x02000000
HEAP_SIZE = 0x00800000
STACK_BASE = 0x0F000000
STACK_SIZE = 0x00100000
STOP_ADDRESS = 0x03000000
CONTEXT_ADDRESS = HEAP_BASE + 0x1000
MOTION_ADDRESS = HEAP_BASE + 0x10000
CACHE_ADDRESS = HEAP_BASE + 0x500000
CACHE_SIZE = 0x20000

ORIGINAL_EDB_BASE = 0x01800000
FULL_DECODE_FUNCTION = 0x00504136
POSE_ASSEMBLER_FUNCTION = 0x004FDB2A
WEAK_HANDLE_RESOLVER = 0x004FCA2B
HEAP_BOOKKEEPING = 0x004E4BF0
PACKED_CHANNEL_RESOLVER = 0x00504D0B
RUNTIME_MOTION_MANAGER = 0x008CB6E8
RUNTIME_ENTRY_ADDRESS = HEAP_BASE + 0x570000
RUNTIME_ENTRY_SIZE = 0x24

SCRATCH_BASE = HEAP_BASE + 0x580000
OUTPUT_OBJECT = SCRATCH_BASE
OUTPUT_POSES = SCRATCH_BASE + 0x1000
OWNER_OBJECT = SCRATCH_BASE + 0x2000
DESCRIPTOR = SCRATCH_BASE + 0x3000
FAKE_RESOURCE = SCRATCH_BASE + 0x4000
BIND_POSITIONS = SCRATCH_BASE + 0x5000

CACHE_MAGIC = b"RAPCV002"
CACHE_HEADER = struct.Struct("<8sIIIIIIQ")
CACHE_POSE = struct.Struct("<7f")


def align_up(value: int, alignment: int = PAGE) -> int:
    return (value + alignment - 1) & ~(alignment - 1)


def align32(value: int) -> int:
    return (value + 0x1F) & ~0x1F


def parse_hex(value: str) -> int:
    return int(value, 16)


def fnv1a64(data: bytes) -> int:
    value = 0xCBF29CE484222325
    for byte in data:
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def map_pe(uc: Uc, executable: Path) -> pefile.PE:
    pe = pefile.PE(str(executable), fast_load=False)
    uc.mem_map(pe.OPTIONAL_HEADER.ImageBase, align_up(pe.OPTIONAL_HEADER.SizeOfImage))
    raw = executable.read_bytes()
    headers_size = min(pe.OPTIONAL_HEADER.SizeOfHeaders, len(raw))
    uc.mem_write(pe.OPTIONAL_HEADER.ImageBase, raw[:headers_size])
    for section in pe.sections:
        data = section.get_data()
        if data:
            uc.mem_write(pe.OPTIONAL_HEADER.ImageBase + section.VirtualAddress, data)
    return pe


@dataclass(frozen=True)
class ClipRequest:
    edb_uid: int
    edb_path: Path
    animation_index: int
    animation_hashcode: int
    animation_offset: int
    motion_offset: int
    motion_size: int
    animskin_hashcode: int
    animskin_offset: int


class RobotsAnimationOracle:
    def __init__(self, executable: Path):
        self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        map_pe(self.uc, executable)
        self.uc.mem_map(HEAP_BASE, HEAP_SIZE)
        self.uc.mem_map(STACK_BASE, STACK_SIZE)
        self.uc.mem_map(STOP_ADDRESS, PAGE)
        self.uc.mem_write(STOP_ADDRESS, b"\x90")
        self.uc.hook_add(UC_HOOK_CODE, self._stop_hook)
        self.uc.hook_add(UC_HOOK_MEM_INVALID, self._invalid_memory)
        self.anim = bytes()
        self.frame_count = 0
        self.bone_count = 0
        self.translation_mask = (0, 0, 0, 0)
        self.relative_bind_positions: list[tuple[float, float, float]] = []

    @staticmethod
    def _stop_hook(uc: Uc, address: int, size: int, user_data: object) -> None:
        if address == STOP_ADDRESS:
            uc.emu_stop()

    @staticmethod
    def _invalid_memory(
        uc: Uc,
        access: int,
        address: int,
        size: int,
        value: int,
        user_data: object,
    ) -> bool:
        raise RuntimeError(
            f"native decoder invalid memory access={access} address=0x{address:08X} "
            f"size={size} value=0x{value:X}"
        )

    def configure(self, request: ClipRequest) -> int:
        edb = request.edb_path.read_bytes()
        anim = bytearray(edb[request.animation_offset : request.animation_offset + 0x9C])
        motion = edb[request.motion_offset : request.motion_offset + request.motion_size]
        if len(anim) != 0x9C or len(motion) != request.motion_size:
            raise ValueError("animation object or motion payload is truncated")

        self.frame_count = struct.unpack_from("<H", anim, 0x0E)[0]
        self.bone_count = anim[0x14]
        self.translation_mask = struct.unpack_from("<4I", anim, 0x60)
        if self.frame_count == 0 or self.bone_count == 0:
            raise ValueError(
                f"invalid animation dimensions frames={self.frame_count} bones={self.bone_count}"
            )

        skin_bone_count = struct.unpack_from("<I", edb, request.animskin_offset + 0x04)[0]
        if skin_bone_count != self.bone_count:
            raise ValueError(
                f"animation/AnimSkin bone mismatch {self.bone_count} != {skin_bone_count}"
            )
        relative_pointer = struct.unpack_from("<i", edb, request.animskin_offset + 0x44)[0]
        relative_address = request.animskin_offset + 0x44 + relative_pointer
        self.relative_bind_positions = [
            struct.unpack_from("<3f", edb, relative_address + bone_index * 16)
            for bone_index in range(self.bone_count)
        ]

        struct.pack_into("<i", anim, 0x04, MOTION_ADDRESS - (CONTEXT_ADDRESS + 0x04))
        self.anim = bytes(anim)
        self.uc.mem_write(CONTEXT_ADDRESS, self.anim)
        self.uc.mem_write(MOTION_ADDRESS, motion)
        return fnv1a64(motion)


def load_requests(binding_tsv: Path, animskin_tsv: Path) -> dict[int, list[ClipRequest]]:
    skin_offsets: dict[tuple[str, int], int] = {}
    with animskin_tsv.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            skin_offsets[(row["edb_path"], parse_hex(row["animskin_hashcode"]))] = parse_hex(
                row["file_offset"]
            )

    grouped: dict[int, list[ClipRequest]] = defaultdict(list)
    with binding_tsv.open(encoding="utf-8", newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            if row["skin_binding_status"] != "resolved_by_base_skin_num":
                continue
            edb_path = Path(row["edb_path"])
            animskin_hashcode = parse_hex(row["animskin_hashcode"])
            grouped[parse_hex(row["edb_uid"])].append(
                ClipRequest(
                    edb_uid=parse_hex(row["edb_uid"]),
                    edb_path=edb_path,
                    animation_index=int(row["animation_index"]),
                    animation_hashcode=parse_hex(row["animation_hashcode"]),
                    animation_offset=parse_hex(row["file_offset"]),
                    motion_offset=parse_hex(row["motiondata_info_addr"]),
                    motion_size=int(row["data_size"]),
                    animskin_hashcode=animskin_hashcode,
                    animskin_offset=skin_offsets[(str(edb_path), animskin_hashcode)],
                )
            )
    for requests in grouped.values():
        requests.sort(key=lambda request: request.animation_index)
    return grouped


def cache_entry_size(anim: bytes) -> int:
    bone_count = anim[0x14]
    auxiliary_count = anim[0x15]
    correction_count = struct.unpack_from("<H", anim, 0x16)[0]
    native_value = 4 * (correction_count + 4 * (bone_count + auxiliary_count))
    return (native_value + 0x5F) & ~0x0F


def active_bone_mask(bone_count: int) -> tuple[int, int, int, int]:
    words = [0, 0, 0, 0]
    for bone_index in range(bone_count):
        words[bone_index // 32] |= 1 << (bone_index & 31)
    return tuple(words)


def call_thiscall(oracle: RobotsAnimationOracle, function: int, ecx: int, args: list[int]) -> int:
    stack_top = STACK_BASE + STACK_SIZE - 0x400
    oracle.uc.mem_write(stack_top, struct.pack(f"<{len(args) + 1}I", STOP_ADDRESS, *args))
    oracle.uc.reg_write(UC_X86_REG_ESP, stack_top)
    oracle.uc.reg_write(UC_X86_REG_ECX, ecx)
    oracle.uc.reg_write(UC_X86_REG_EIP, function)
    oracle.uc.emu_start(function, STOP_ADDRESS + 1, count=60_000_000)
    return oracle.uc.reg_read(UC_X86_REG_EAX)


def resolve_channel_table(oracle: RobotsAnimationOracle) -> int:
    oracle.uc.mem_write(CONTEXT_ADDRESS, oracle.anim)
    return call_thiscall(
        oracle,
        PACKED_CHANNEL_RESOLVER,
        CONTEXT_ADDRESS + 0x38,
        [5],
    )


def native_block_selection(
    oracle: RobotsAnimationOracle,
    frame: int,
    channel_table: int,
) -> tuple[int, int, int, int, int]:
    frame_count = oracle.frame_count
    clamped = max(0, min(frame, frame_count - 1))
    start_frame = 0
    end_frame = frame_count - 1
    block_offset = -1
    block_stride = 0

    if channel_table:
        low_size, high_size = struct.unpack("<2H", oracle.uc.mem_read(channel_table, 4))
        block_stride = align32((high_size << 16) | low_size)
        block_index = 0
        while True:
            end_frame = struct.unpack(
                "<H",
                oracle.uc.mem_read(channel_table + 4 + block_index * 2, 2),
            )[0]
            if start_frame <= clamped <= end_frame:
                break
            start_frame = end_frame + 1
            block_index += 1
            if start_frame >= frame_count:
                raise ValueError(
                    f"native block table does not cover frame {clamped}: "
                    f"start={start_frame} frames={frame_count}"
                )
        if end_frame < frame_count - 1:
            end_frame += 1
        block_offset = block_index * block_stride

    frame_count_in_block = end_frame - start_frame + 1
    relative_frame = clamped - start_frame
    return block_offset, start_frame, frame_count_in_block, relative_frame, block_stride


def write_runtime_entry(
    oracle: RobotsAnimationOracle,
    motion_size: int,
    block_offset: int,
) -> None:
    stream_pointer = MOTION_ADDRESS if block_offset < 0 else MOTION_ADDRESS + block_offset
    entry = bytearray(RUNTIME_ENTRY_SIZE)
    struct.pack_into("<I", entry, 0x00, 1)
    struct.pack_into("<I", entry, 0x04, 1)
    struct.pack_into("<I", entry, 0x08, align32(motion_size))
    struct.pack_into("<I", entry, 0x0C, align32(motion_size))
    struct.pack_into("<I", entry, 0x10, 0)
    struct.pack_into("<i", entry, 0x14, block_offset)
    struct.pack_into("<I", entry, 0x18, stream_pointer)
    struct.pack_into("<I", entry, 0x1C, 0)
    struct.pack_into("<H", entry, 0x20, 0)
    struct.pack_into("<H", entry, 0x22, 1)
    oracle.uc.mem_write(RUNTIME_ENTRY_ADDRESS, bytes(entry))
    oracle.uc.mem_write(RUNTIME_MOTION_MANAGER, struct.pack("<I", RUNTIME_ENTRY_ADDRESS))


def prepare_runtime_cache_entries(
    oracle: RobotsAnimationOracle,
    frames: list[int],
    correction_key: int,
    motion_size: int,
) -> tuple[dict[int, int], list[dict[str, int]]]:
    anim = bytearray(oracle.anim)
    channel_table = resolve_channel_table(oracle)
    chunk_size = anim[0x0D]
    if chunk_size == 0:
        raise ValueError("zero native cache chunk size")
    entry_size = cache_entry_size(anim)
    group_size = entry_size * chunk_size
    group_indices = sorted({frame // chunk_size for frame in frames})
    required = group_size * len(group_indices)
    if required > CACHE_SIZE:
        raise ValueError(f"cache groups need {required} bytes, buffer has {CACHE_SIZE}")

    context_size = max(0x200, 0x98 + (max(group_indices) + 1) * 4)
    context = bytearray(context_size)
    context[: len(anim)] = anim
    struct.pack_into("<I", context, 0x04, RUNTIME_ENTRY_ADDRESS)

    group_addresses: dict[int, int] = {}
    for ordinal, group_index in enumerate(group_indices):
        group_address = CACHE_ADDRESS + ordinal * group_size
        group_addresses[group_index] = group_address
        struct.pack_into("<I", context, 0x98 + group_index * 4, group_address)
        cache = bytearray(group_size)
        for entry_index in range(chunk_size):
            struct.pack_into("<H", cache, entry_index * entry_size + 2, 0xFFFF)
        oracle.uc.mem_write(group_address, bytes(cache))

    oracle.uc.mem_write(HEAP_BOOKKEEPING, b"\xC3")
    # The full decoder receives the same active-skeleton mask as the native
    # caller. Animation+0x60 is only the translation-channel mask and must not
    # suppress rotation-only bones during cache decode.
    channel_mask = active_bone_mask(oracle.bone_count)
    entries: dict[int, int] = {}
    selections: list[dict[str, int]] = []

    for frame in frames:
        block_offset, start_frame, count, relative_frame, stride = native_block_selection(
            oracle,
            frame,
            channel_table,
        )
        write_runtime_entry(oracle, motion_size, block_offset)
        oracle.uc.mem_write(CONTEXT_ADDRESS, bytes(context))

        stack_top = STACK_BASE + STACK_SIZE - 0x400
        args = [frame, correction_key, *channel_mask]
        oracle.uc.mem_write(
            stack_top,
            struct.pack(f"<{len(args) + 1}I", STOP_ADDRESS, *args),
        )
        oracle.uc.reg_write(UC_X86_REG_ESP, stack_top)
        oracle.uc.reg_write(UC_X86_REG_ECX, CONTEXT_ADDRESS)
        oracle.uc.reg_write(UC_X86_REG_EIP, FULL_DECODE_FUNCTION)
        oracle.uc.emu_start(FULL_DECODE_FUNCTION, STOP_ADDRESS + 1, count=60_000_000)
        entry_address = oracle.uc.reg_read(UC_X86_REG_EAX)

        expected = group_addresses[frame // chunk_size] + (frame % chunk_size) * entry_size
        if entry_address != expected:
            raise ValueError(
                f"frame {frame} cache 0x{entry_address:08X} != expected 0x{expected:08X}"
            )
        entries[frame] = entry_address
        selections.append(
            {
                "frame": frame,
                "block_offset": block_offset,
                "block_stride": stride,
                "start_frame": start_frame,
                "frame_count": count,
                "relative_frame": relative_frame,
                "stream_pointer": MOTION_ADDRESS
                if block_offset < 0
                else MOTION_ADDRESS + block_offset,
            }
        )

    return entries, selections


def assemble_pose(
    oracle: RobotsAnimationOracle,
    current_cache: int,
    next_cache: int,
    fraction: float,
) -> list[tuple[float, float, float, float, float, float, float]]:
    oracle.uc.mem_write(SCRATCH_BASE, bytes(0x10000))

    bind_blob = bytearray(oracle.bone_count * 16)
    for bone_index, position in enumerate(oracle.relative_bind_positions):
        struct.pack_into("<4f", bind_blob, bone_index * 16, *position, 1.0)
    oracle.uc.mem_write(BIND_POSITIONS, bytes(bind_blob))

    resource = bytearray(0x100)
    struct.pack_into("<i", resource, 0x44, BIND_POSITIONS - (FAKE_RESOURCE + 0x44))
    struct.pack_into("<I", resource, 0x80, 0)
    oracle.uc.mem_write(FAKE_RESOURCE, bytes(resource))

    oracle.uc.mem_write(
        WEAK_HANDLE_RESOLVER,
        b"\xB8" + struct.pack("<I", FAKE_RESOURCE) + b"\xC3",
    )

    output_object = bytearray(0x20)
    struct.pack_into("<I", output_object, 0x10, OUTPUT_POSES)
    oracle.uc.mem_write(OUTPUT_OBJECT, bytes(output_object))

    initial_poses = bytearray(oracle.bone_count * 0x20)
    for bone_index, position in enumerate(oracle.relative_bind_positions):
        struct.pack_into("<4f", initial_poses, bone_index * 0x20, *position, 1.0)
        struct.pack_into("<4f", initial_poses, bone_index * 0x20 + 0x10, 0.0, 0.0, 0.0, 1.0)
    oracle.uc.mem_write(OUTPUT_POSES, bytes(initial_poses))

    owner = bytearray(0x40)
    struct.pack_into("<I", owner, 0x1C, 1)
    oracle.uc.mem_write(OWNER_OBJECT, bytes(owner))

    descriptor = bytearray(0x70)
    struct.pack_into("<I", descriptor, 0x00, current_cache)
    struct.pack_into("<I", descriptor, 0x04, next_cache)
    struct.pack_into("<I", descriptor, 0x10, CONTEXT_ADDRESS)
    struct.pack_into("<I", descriptor, 0x14, CONTEXT_ADDRESS)
    struct.pack_into("<f", descriptor, 0x40, fraction)
    struct.pack_into("<f", descriptor, 0x48, 1.0)
    struct.pack_into("<I", descriptor, 0x60, 1)
    oracle.uc.mem_write(DESCRIPTOR, bytes(descriptor))

    # Robots.exe caller 0x0050DD3A passes the skeleton's active-bone mask from
    # the pose request at +0x10. The four dwords at Animation+0x60 are a
    # different mask: they only identify bones with translation channels.
    # Passing that translation mask here forces every rotation-only bone to an
    # identity quaternion in 0x004FDB2A, which caused the partial/T-pose output
    # produced by the previous oracle revision.
    channel_mask = active_bone_mask(oracle.bone_count)
    result = call_thiscall(
        oracle,
        POSE_ASSEMBLER_FUNCTION,
        OWNER_OBJECT,
        [OUTPUT_OBJECT, DESCRIPTOR, *channel_mask],
    )
    if result & 0xFF != 1:
        raise ValueError(f"native pose assembler returned 0x{result:08X}")

    poses = []
    for bone_index in range(oracle.bone_count):
        values = struct.unpack(
            "<8f",
            oracle.uc.mem_read(OUTPUT_POSES + bone_index * 0x20, 0x20),
        )
        position = values[:3]
        quaternion = values[4:8]
        poses.append((*position, *quaternion))
    return poses
