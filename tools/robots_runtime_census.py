#!/usr/bin/env python3
"""Build proof-oriented Robots XItem*/HT_Entity/DEV MAP census reports.

This tool deliberately separates class existence from behavior recovery. A valid
class descriptor proves class identity, parent, size and constructor. It does not
prove gameplay semantics by itself.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import struct
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

import pefile


CLASS_RE = re.compile(
    rb"(?:XItem(?:Handler(?:_[A-Za-z0-9]+)*|Physics(?:Sphere|(?:_[A-Za-z0-9]+)*))|EXItemAnimator(?:_[A-Za-z0-9]+)*|EXItemRender(?:_[A-Za-z0-9]+)*)\x00"
)

ENTITY_DEFINE_RE = re.compile(
    r"^#define\s+(HT_Entity_[A-Za-z0-9_]+)\s+0x([0-9A-Fa-f]{8})\s*$"
)

DEV_MAPS = (
    (55, 0x01000037, "Mechanics", "m00_demo.edb", 72),
    (15, 0x0100000F, "Enemies", "m99_enem.edb", 45),
    (160, 0x010000A0, "NPCs", "m98_npcs.edb", 22),
    (116, 0x01000074, "Ball", "m00_ball.edb", 12),
    (1, 0x01000001, "Empty Aunt Fanny House", "m00_mapt.edb", 1),
)

EXACT_VTABLES = {
    "XItemHandler_Npc_Fender": 0x005E71B8,
    "XItemHandler_Explosion": 0x005F1470,
    "XItemHandler_ElectricityExplosion": 0x005F1550,
    "XItemHandler_PlayerBall": 0x005EEDB8,
    "XItemHandler_PlayerBallChase": 0x005EF4E0,
    "XItemHandler_PlayerBallRace": 0x005EF6C8,
    "XItemPhysics": 0x005DFB20,
    "XItemPhysicsSphere": 0x005DFBA0,
    "XItemPhysics_Character": 0x005DFC60,
    "XItemPhysics_Interactive": 0x005DFCE8,
    "XItemPhysics_Platform": 0x005DFD78,
    "XItemPhysics_Projectile": 0x005DFE18,
    "XItemPhysics_ProjectileRayCast": 0x005DFE90,
    "XItemPhysics_PickupAttract": 0x005F12D8,
    "EXItemAnimator": 0x005F34E0,
    "EXItemAnimator_ForceFeedback": 0x005F3478,
    "EXItemAnimator_Map": 0x005F3548,
    "EXItemAnimator_DynLight": 0x005F4510,
    "EXItemAnimator_Collision": 0x005F5890,
    "EXItemAnimator_Camera": 0x005F5908,
}

CLASS_COVERAGE = {
    "XItemHandler_Script": ("partial", "script_timeline_and_entity_resolution"),
    "XItemHandler_Platform": ("partial", "trigger_path_rotation_and_event_preview"),
    "XItemHandler_Lift": ("partial", "trigger_path_and_event_preview"),
    "XItemHandler_Vehicle": ("partial", "path_yaw_wheels_steering_and_linear_contact_carry"),
    "XItemHandler_Fan": ("diagnostic", "native_live_axis_rotation_consumer_proven"),
    "XItemHandler_FanHorizontal": ("diagnostic", "native_live_axis_rotation_consumer_proven"),
    "XItemHandler_Pickup": ("partial", "serialized_pickup_visual_and_script_geometry"),
    "XItemHandler_ElectricityExplosion": ("partial", "effect_control_and_particle_geometry"),
    "XItemHandler_Explosion": (
        "diagnostic",
        "sphere_fragment_fixed_step_motion_and_contact_consumer",
    ),
    "XItemHandler_PlayerBall": (
        "diagnostic",
        "sphere_surface_classification_and_ball_response_parameters",
    ),
    "XItemHandler_PlayerBallRace": (
        "diagnostic",
        "slippery_floor_flag_and_ball_state_transition",
    ),
    "XItemHandler_PlayerBallChase": (
        "diagnostic",
        "path_steering_and_slippery_surface_response",
    ),
    "XItemHandler_Hazard": ("partial", "serialized_visual_and_trigger_diagnostic"),
    "XItemHandler_Camera": ("diagnostic", "camera_path_context_without_player_camera_state"),
    "XItemHandler_WatchBot": ("diagnostic", "watchbot_path_context_and_hysteresis"),
    "XItemHandler_Boss_Ratchet": ("diagnostic", "boss_path_context"),
    "XItemHandler_Transporter": ("diagnostic", "transporter_path_context"),
    "XItemHandler_Npc": ("diagnostic", "npc_mission_cutscene_and_dialogue_state_context"),
    "XItemHandler_Npc_Fender": ("diagnostic", "npc_fender_specialized_runtime_context"),
    "XItemHandler_Monster": ("diagnostic", "monster_trigger_getters_path_and_proximity_context"),
    "XItemHandler_Test_Monster": ("diagnostic", "test_monster_trigger_getter_context"),
    "XItemHandler_Monster_EF03_EvilBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "XItemHandler_Monster_EM07_PiranhaBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "XItemHandler_Monster_EW11_FatBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "XItemHandler_Monster_TestAnimBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "EXItemAnimator_Anim": ("partial", "anim_and_animskin_frame_preview"),
    "EXItemAnimator_Entity": ("partial", "entity_assembly_and_vehicle_wheel_transforms"),
    "EXItemAnimator_Map": (
        "partial",
        "map_geometry_preview_and_native_collision_query_consumers",
    ),
    "EXItemAnimator_DynLight": (
        "diagnostic",
        "dynamic_light_record_attach_transform_color_update_and_detach",
    ),
    "EXItemAnimator_Particle": ("partial", "native_particle_command_preview"),
    "EXItemAnimator_Script": ("partial", "script_timeline_and_geometry_preview"),
    "EXItemRender_SkinAnim": ("partial", "animskin_render_path"),
    "XItemPhysics": (
        "diagnostic",
        "base_fixed_step_integrator_and_collision_dispatch",
    ),
    "XItemPhysicsSphere": (
        "diagnostic",
        "fixed_step_displacement_mesh_contact_classification_and_platform_attachment",
    ),
    "XItemPhysics_Character": (
        "diagnostic",
        "base_integrator_inherited_by_character_physics",
    ),
    "XItemPhysics_Interactive": (
        "diagnostic",
        "player_or_magnetic_box_contact_gate_and_contact_owner_latch",
    ),
    "XItemPhysics_PickupAttract": (
        "diagnostic",
        "target_attraction_acceleration_and_velocity_damping",
    ),
    "XItemPhysics_Platform": ("partial", "contact_registration_and_linear_carry"),
    "XItemPhysics_Projectile": (
        "diagnostic",
        "ballistic_integrator_and_projectile_hit_owner_latch",
    ),
    "XItemPhysics_ProjectileRayCast": (
        "diagnostic",
        "swept_raycast_query_and_resolved_hit_state",
    ),
}



@dataclass
class RuntimeClass:
    family: str
    name: str
    descriptor_va: int
    parent_descriptor_va: int
    parent_name: str | None
    object_size: int
    constructor_va: int
    vtable_va: int | None
    descriptor_xrefs: int
    documented: bool
    source_referenced: bool
    coverage: str
    behavior_status: str
    proof_status: str


@dataclass
class EntityName:
    hashcode: int
    name: str
    source_line: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--project-root", type=Path, required=True)
    parser.add_argument("--hashcodes", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def section_for_rva(pe: pefile.PE, rva: int):
    for section in pe.sections:
        start = section.VirtualAddress
        end = start + max(section.Misc_VirtualSize, section.SizeOfRawData)
        if start <= rva < end:
            return section
    return None


def is_executable_rva(pe: pefile.PE, rva: int) -> bool:
    section = section_for_rva(pe, rva)
    return bool(section and section.Characteristics & 0x20000000)


def is_mapped_va(pe: pefile.PE, va: int) -> bool:
    if va == 0:
        return True
    return section_for_rva(pe, va - pe.OPTIONAL_HEADER.ImageBase) is not None


def read_u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def file_offset_to_va(pe: pefile.PE, offset: int) -> int:
    return pe.OPTIONAL_HEADER.ImageBase + pe.get_rva_from_offset(offset)


def count_text_pointers(pe: pefile.PE, data: bytes, table_va: int, count: int = 24) -> int:
    try:
        table_offset = pe.get_offset_from_rva(table_va - pe.OPTIONAL_HEADER.ImageBase)
    except Exception:
        return 0
    valid = 0
    for index in range(count):
        offset = table_offset + index * 4
        if offset + 4 > len(data):
            break
        candidate = read_u32(data, offset)
        if candidate and is_executable_rva(pe, candidate - pe.OPTIONAL_HEADER.ImageBase):
            valid += 1
    return valid


def infer_vtable(pe: pefile.PE, data: bytes, constructor_va: int) -> int | None:
    try:
        ctor_offset = pe.get_offset_from_rva(constructor_va - pe.OPTIONAL_HEADER.ImageBase)
    except Exception:
        return None
    window = data[ctor_offset : ctor_offset + 0x240]
    candidates: dict[int, int] = {}
    for offset in range(0, max(0, len(window) - 4)):
        candidate = read_u32(window, offset)
        if candidate in candidates or not is_mapped_va(pe, candidate):
            continue
        section = section_for_rva(pe, candidate - pe.OPTIONAL_HEADER.ImageBase)
        if not section or section.Characteristics & 0x20000000:
            continue
        score = count_text_pointers(pe, data, candidate)
        if score >= 6:
            candidates[candidate] = score
    if not candidates:
        return None
    return max(candidates, key=lambda candidate: (candidates[candidate], -candidate))



def collect_search_text(root: Path) -> tuple[str, str]:
    docs = []
    sources = []
    for relative in ("functions_map.md", "CURRENT_STAGE.md", "checklist_todo.md"):
        path = root / relative
        if path.is_file():
            docs.append(path.read_text(encoding="utf-8", errors="replace"))
    source_root = root / "_tools/eurochef-main_legacy/eurochef"
    for path in source_root.rglob("*.rs"):
        try:
            sources.append(path.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            pass
    return "\n".join(docs), "\n".join(sources)


def classify_runtime_class(name: str) -> tuple[str, str]:
    exact = CLASS_COVERAGE.get(name)
    if exact:
        return exact
    if name in {
        "XItemHandler",
        "XItemHandler_Hittable",
        "XItemHandler_Character",
        "XItemHandler_AI_Character",
        "XItemHandler_Interactive",
        "XItemHandler_Projectile",
    }:
        return "structural", "base_layout_hierarchy_only"
    if name.startswith("EXItemAnimator"):
        return "structural", "animator_descriptor_layout_only"
    if name.startswith("EXItemRender"):
        return "structural", "render_descriptor_layout_only"
    if name.startswith("XItemPhysics"):
        return "structural", "descriptor_layout_only"
    if name.startswith("XItemHandler_Monster") or name in {
        "XItemHandler_Monster",
        "XItemHandler_Test_Monster",
    }:
        return "unresolved", "monster_ai_gameplay_runtime"
    if name.startswith("XItemHandler_Npc"):
        return "unresolved", "npc_ai_dialogue_gameplay_runtime"
    if name.startswith("XItemHandler_Player") or name.startswith("XItemHandler_FinalBoss_Player"):
        return "unresolved", "player_input_state_and_gameplay_runtime"
    if "Projectile" in name or name.endswith("Missile") or name.endswith("Mine"):
        return "unresolved", "projectile_collision_and_damage_runtime"
    return "unresolved", "class_descriptor_only"


def scan_runtime_classes(exe: Path, root: Path) -> list[RuntimeClass]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    image_base = pe.OPTIONAL_HEADER.ImageBase
    docs, sources = collect_search_text(root)
    descriptors: dict[int, tuple[str, int, int, int]] = {}

    for match in CLASS_RE.finditer(data):
        name = match.group(0)[:-1].decode("ascii")
        string_va = file_offset_to_va(pe, match.start())
        encoded = struct.pack("<I", string_va)
        search_offset = 0
        while True:
            ref_offset = data.find(encoded, search_offset)
            if ref_offset < 0:
                break
            search_offset = ref_offset + 1
            if ref_offset < 4 or ref_offset + 12 > len(data):
                continue
            descriptor_offset = ref_offset - 4
            try:
                descriptor_va = file_offset_to_va(pe, descriptor_offset)
            except Exception:
                continue
            section = section_for_rva(pe, descriptor_va - image_base)
            if not section or section.Characteristics & 0x20000000:
                continue
            parent = read_u32(data, descriptor_offset)
            object_size = read_u32(data, descriptor_offset + 8)
            constructor = read_u32(data, descriptor_offset + 12)
            if not (4 <= object_size <= 0x10000):
                continue
            if not is_executable_rva(pe, constructor - image_base):
                continue
            if not is_mapped_va(pe, parent):
                continue
            descriptors[descriptor_va] = (name, parent, object_size, constructor)

    name_by_descriptor = {descriptor: fields[0] for descriptor, fields in descriptors.items()}
    text_sections = [
        section.get_data()
        for section in pe.sections
        if section.Characteristics & 0x20000000
    ]
    rows = []
    for descriptor_va, (name, parent, object_size, constructor) in sorted(
        descriptors.items(), key=lambda item: item[1][0]
    ):
        xref_bytes = struct.pack("<I", descriptor_va)
        xrefs = sum(section.count(xref_bytes) for section in text_sections)
        documented = name in docs
        source_referenced = name in sources
        coverage, status = classify_runtime_class(name)
        proof_status = "descriptor_and_constructor_proven"
        if not xrefs:
            proof_status = "descriptor_proven_no_direct_text_xref"
        if name.startswith("XItemPhysics"):
            family = "XItemPhysics"
        elif name.startswith("EXItemAnimator"):
            family = "EXItemAnimator"
        elif name.startswith("EXItemRender"):
            family = "EXItemRender"
        else:
            family = "XItemHandler"
        rows.append(
            RuntimeClass(
                family=family,
                name=name,
                descriptor_va=descriptor_va,
                parent_descriptor_va=parent,
                parent_name=name_by_descriptor.get(parent),
                object_size=object_size,
                constructor_va=constructor,
                vtable_va=EXACT_VTABLES.get(name, infer_vtable(pe, data, constructor)),
                descriptor_xrefs=xrefs,
                documented=documented,
                source_referenced=source_referenced,
                coverage=coverage,
                behavior_status=status,
                proof_status=proof_status,
            )
        )
    return rows


MONSTER_INHERITED_VTABLE_SLOTS = 90
MONSTER_LIFECYCLE_SLOTS = {0, 1}
MONSTER_POST_INIT_SLOT = 66

PHYSICS_STANDARD_VTABLE_SLOTS = 30
PHYSICS_VTABLE_SLOT_COUNTS = {
    "XItemPhysics_Interactive": 32,
}
PHYSICS_GENERIC_SLOT_ROLES = {
    0: "fixed_step_update",
    2: "runtime_descriptor",
    3: "destructor",
    4: "world_collision_query_dispatch",
    23: "object_contact_callback",
    25: "collision_record_handled_flag",
    26: "contact_callback_primary",
    27: "contact_callback_secondary",
}
PHYSICS_CLASS_SLOT_ROLES = {
    ("XItemPhysicsSphere", 0): "sphere_fixed_step_integrator",
    ("XItemPhysicsSphere", 2): "runtime_descriptor",
    ("XItemPhysicsSphere", 14): "fixed_step_displacement_setter",
    ("XItemPhysicsSphere", 15): "fixed_step_displacement_getter",
    ("XItemPhysicsSphere", 16): "platform_attachment_secondary_update",
    ("XItemPhysicsSphere", 22): "mesh_contact_classification_and_extent_update",
    ("XItemPhysics_Character", 0): "base_integrator_wrapper",
    ("XItemPhysics_Interactive", 0): "contact_state_update",
    ("XItemPhysics_Interactive", 12): "interactive_contact_dispatch",
    ("XItemPhysics_Interactive", 30): "contact_owner_setter",
    ("XItemPhysics_Interactive", 31): "player_or_magnetic_box_contact_filter",
    ("XItemPhysics_PickupAttract", 0): "target_attraction_update",
    ("XItemPhysics_Platform", 0): "moving_platform_update",
    ("XItemPhysics_Platform", 12): "platform_contact_dispatch",
    ("XItemPhysics_Platform", 26): "platform_contact_registration",
    ("XItemPhysics_Platform", 27): "platform_contact_point_velocity_transfer",
    ("XItemPhysics_Projectile", 0): "ballistic_projectile_update",
    ("XItemPhysics_Projectile", 23): "projectile_hit_owner_latch",
    ("XItemPhysics_ProjectileRayCast", 0): "swept_raycast_update",
    ("XItemPhysics_ProjectileRayCast", 1): "raycast_mode_enable",
    ("XItemPhysics_ProjectileRayCast", 23): "projectile_hit_owner_latch",
}


ANIMATOR_VTABLE_SLOTS = 26
ANIMATOR_GENERIC_SLOT_ROLES = {
    0: "deleting_destructor",
    1: "runtime_descriptor_getter",
    2: "runtime_class_name_getter",
    3: "object_size_getter",
}
ANIMATOR_CLASS_SLOT_ROLES = {
    ("EXItemAnimator_DynLight", 5): "dynamic_light_transform_color_update",
    ("EXItemAnimator_DynLight", 6): "bind_accept_true",
    ("EXItemAnimator_Map", 4): "map_noop_hook",
    ("EXItemAnimator_Map", 5): "map_bind_gate_false",
    ("EXItemAnimator_Map", 6): "owner_resource_bind_and_transform_cache",
    ("EXItemAnimator_Map", 14): "map_collision_query_narrowphase_0051A943",
    ("EXItemAnimator_Map", 15): "map_collision_query_narrowphase_00519CB8",
    ("EXItemAnimator_Map", 18): "map_section_query_and_cached_contact_update",
    ("EXItemAnimator_Map", 25): "owner_transform_and_timeline_evaluation",
}


PLAYER_BALL_SURFACE_ROLES = (
    {
        "owner_class": "XItemHandler_PlayerBall",
        "owner_vtable": "0x005EEDB8",
        "slot_index": 107,
        "slot_offset": "0x1AC",
        "function_va": "0x004B2D20",
        "source_field": "XItemPhysicsSphere+0x1E4",
        "source_mask": "0x01",
        "surface_role": "slippery_floor_primary",
        "ball_flag_field": "+0x7FC",
        "ball_flag_mask": "0x20",
        "response_parameters": "+0x700=mode_table[0] (0.2 for shipped modes 2/3); +0x704=0.3",
        "proof_anchor": "Slippery balls",
    },
    {
        "owner_class": "XItemHandler_PlayerBall",
        "owner_vtable": "0x005EEDB8",
        "slot_index": 107,
        "slot_offset": "0x1AC",
        "function_va": "0x004B2D20",
        "source_field": "XItemPhysicsSphere+0x1E5",
        "source_mask": "0x01",
        "surface_role": "slippery_floor_secondary",
        "ball_flag_field": "+0x7FC",
        "ball_flag_mask": "0x40",
        "response_parameters": "+0x700=mode_table[0] (0.2 for shipped modes 2/3); +0x704=0.3",
        "proof_anchor": "Slippery balls",
    },
    {
        "owner_class": "XItemHandler_PlayerBall",
        "owner_vtable": "0x005EEDB8",
        "slot_index": 107,
        "slot_offset": "0x1AC",
        "function_va": "0x004B2D20",
        "source_field": "XItemPhysicsSphere+0x1E4",
        "source_mask": "0x02",
        "surface_role": "electric_floor",
        "ball_flag_field": "",
        "ball_flag_mask": "",
        "response_parameters": "virtual +0x154(0)",
        "proof_anchor": "Electric Floor!!!",
    },
    {
        "owner_class": "XItemHandler_PlayerBall",
        "owner_vtable": "0x005EEDB8",
        "slot_index": 107,
        "slot_offset": "0x1AC",
        "function_va": "0x004B2D20",
        "source_field": "XItemPhysicsSphere+0x1E4",
        "source_mask": "0x20",
        "surface_role": "slime_floor",
        "ball_flag_field": "",
        "ball_flag_mask": "",
        "response_parameters": "virtual +0x170(0)",
        "proof_anchor": "Slime Floor!!!",
    },
    {
        "owner_class": "XItemHandler_PlayerBallRace",
        "owner_vtable": "0x005EF6C8",
        "slot_index": 107,
        "slot_offset": "0x1AC",
        "function_va": "0x004C6A80",
        "source_field": "PlayerBall+0x7FC",
        "source_mask": "0x20",
        "surface_role": "slippery_floor_state_transition",
        "ball_flag_field": "+0x7FC",
        "ball_flag_mask": "0x20 (PBF_BALL_SLIPPERYFLOOR)",
        "response_parameters": "sets latch 0x80 and state 0x1C unless already 0x1C/0x1D",
        "proof_anchor": "m_BallFlags&PBF_BALL_SLIPPERYFLOOR",
    },
    {
        "owner_class": "XItemHandler_PlayerBallChase",
        "owner_vtable": "0x005EF4E0",
        "slot_index": 101,
        "slot_offset": "0x194",
        "function_va": "0x004C1F30",
        "source_field": "PlayerBall+0x7FC",
        "source_mask": "0x20",
        "surface_role": "chase_slippery_path_response",
        "ball_flag_field": "+0x7FC",
        "ball_flag_mask": "0x20",
        "response_parameters": "path-basis response uses exact factors 3.0, 0.2 and smoothing 0.1",
        "proof_anchor": "***********Slippery Balls %f",
    },
)


def read_vtable_slots(
    pe: pefile.PE, data: bytes, vtable_va: int | None, count: int
) -> list[int]:
    if vtable_va is None:
        return []
    try:
        offset = pe.get_offset_from_rva(vtable_va - pe.OPTIONAL_HEADER.ImageBase)
    except Exception:
        return []
    if offset < 0 or offset + count * 4 > len(data):
        return []
    return [read_u32(data, offset + index * 4) for index in range(count)]


def scan_physics_vtable_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    physics_rows = sorted(
        (row for row in classes if row.family == "XItemPhysics"),
        key=lambda item: item.name,
    )
    report_rows: list[dict] = []
    overrides = 0
    semantic_roles = 0
    for row in physics_rows:
        slot_count = PHYSICS_VTABLE_SLOT_COUNTS.get(
            row.name, PHYSICS_STANDARD_VTABLE_SLOTS
        )
        own_slots = read_vtable_slots(pe, data, row.vtable_va, slot_count)
        parent = by_name.get(row.parent_name or "")
        parent_slot_count = (
            PHYSICS_VTABLE_SLOT_COUNTS.get(
                parent.name, PHYSICS_STANDARD_VTABLE_SLOTS
            )
            if parent and parent.family == "XItemPhysics"
            else 0
        )
        parent_slots = read_vtable_slots(
            pe,
            data,
            parent.vtable_va if parent and parent.family == "XItemPhysics" else None,
            parent_slot_count,
        )
        for slot_index, function_va in enumerate(own_slots):
            parent_function_va = (
                parent_slots[slot_index] if slot_index < len(parent_slots) else None
            )
            is_override = bool(
                parent_function_va is not None and function_va != parent_function_va
            )
            if is_override:
                overrides += 1
            role = PHYSICS_CLASS_SLOT_ROLES.get((row.name, slot_index), "")
            if not role and row.name != "XItemPhysicsSphere":
                role = PHYSICS_GENERIC_SLOT_ROLES.get(slot_index, "")
            if role:
                semantic_roles += 1
            report_rows.append(
                {
                    "name": row.name,
                    "parent_name": row.parent_name or "",
                    "vtable_va": hex_or_empty(row.vtable_va),
                    "slot_index": slot_index,
                    "slot_offset": f"0x{slot_index * 4:02X}",
                    "function_va": hex_or_empty(function_va),
                    "parent_function_va": hex_or_empty(parent_function_va),
                    "is_override": int(is_override),
                    "semantic_role": role,
                    "proof_status": (
                        "instruction_linked_role" if role else "address_only"
                    ),
                }
            )
    summary = {
        "physics_runtime_classes": len(physics_rows),
        "vtable_rows": len(report_rows),
        "parent_overrides": overrides,
        "instruction_linked_roles": semantic_roles,
        "standard_slots": PHYSICS_STANDARD_VTABLE_SLOTS,
        "class_slot_counts": dict(sorted(PHYSICS_VTABLE_SLOT_COUNTS.items())),
    }
    return report_rows, summary


def scan_animator_vtable_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    animator_rows = sorted(
        (row for row in classes if row.family == "EXItemAnimator"),
        key=lambda item: item.name,
    )
    report_rows: list[dict] = []
    overrides = 0
    semantic_roles = 0
    for row in animator_rows:
        own_slots = read_vtable_slots(pe, data, row.vtable_va, ANIMATOR_VTABLE_SLOTS)
        parent = by_name.get(row.parent_name or "")
        parent_slots = read_vtable_slots(
            pe,
            data,
            parent.vtable_va if parent and parent.family == "EXItemAnimator" else None,
            ANIMATOR_VTABLE_SLOTS,
        )
        for slot_index, function_va in enumerate(own_slots):
            parent_function_va = (
                parent_slots[slot_index] if slot_index < len(parent_slots) else None
            )
            is_override = bool(
                parent_function_va is not None and function_va != parent_function_va
            )
            if is_override:
                overrides += 1
            role = ANIMATOR_CLASS_SLOT_ROLES.get((row.name, slot_index), "")
            if not role:
                role = ANIMATOR_GENERIC_SLOT_ROLES.get(slot_index, "")
            if role:
                semantic_roles += 1
            report_rows.append(
                {
                    "name": row.name,
                    "parent_name": row.parent_name or "",
                    "vtable_va": hex_or_empty(row.vtable_va),
                    "slot_index": slot_index,
                    "slot_offset": f"0x{slot_index * 4:02X}",
                    "function_va": hex_or_empty(function_va),
                    "parent_function_va": hex_or_empty(parent_function_va),
                    "is_override": int(is_override),
                    "semantic_role": role,
                    "proof_status": (
                        "instruction_linked_role" if role else "address_only"
                    ),
                }
            )
    summary = {
        "animator_runtime_classes": len(animator_rows),
        "vtable_rows": len(report_rows),
        "parent_overrides": overrides,
        "instruction_linked_roles": semantic_roles,
        "standard_slots": ANIMATOR_VTABLE_SLOTS,
    }
    return report_rows, summary


def scan_monster_vtable_diffs(exe: Path, classes: list[RuntimeClass]) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    monster_rows = [
        row
        for row in classes
        if row.name == "XItemHandler_Monster"
        or row.name == "XItemHandler_Test_Monster"
        or row.name.startswith("XItemHandler_Monster_")
    ]
    report_rows: list[dict] = []
    counts: dict[str, int] = {}
    for row in sorted(monster_rows, key=lambda item: item.name):
        parent = by_name.get(row.parent_name or "")
        own_slots = read_vtable_slots(
            pe, data, row.vtable_va, MONSTER_INHERITED_VTABLE_SLOTS
        )
        parent_slots = read_vtable_slots(
            pe,
            data,
            parent.vtable_va if parent else None,
            MONSTER_INHERITED_VTABLE_SLOTS,
        )
        compared = min(len(own_slots), len(parent_slots))
        overridden = [
            index
            for index in range(compared)
            if own_slots[index] != parent_slots[index]
        ]
        non_lifecycle = [
            index for index in overridden if index not in MONSTER_LIFECYCLE_SLOTS
        ]
        shares_parent = bool(
            row.vtable_va is not None
            and parent is not None
            and row.vtable_va == parent.vtable_va
        )
        if row.name == "XItemHandler_Monster":
            classification = "base_monster"
        elif shares_parent:
            classification = "shared_parent_vtable"
        elif non_lifecycle == [MONSTER_POST_INIT_SLOT]:
            classification = "post_init_hook_only"
        else:
            classification = "specialized_virtual_overrides"
        counts[classification] = counts.get(classification, 0) + 1
        report_rows.append(
            {
                "name": row.name,
                "parent_name": row.parent_name or "",
                "object_size": f"0x{row.object_size:X}",
                "constructor_va": hex_or_empty(row.constructor_va),
                "vtable_va": hex_or_empty(row.vtable_va),
                "parent_vtable_va": hex_or_empty(parent.vtable_va if parent else None),
                "shares_parent_vtable": int(shares_parent),
                "inherited_slots_compared": compared,
                "overridden_slots": ",".join(str(index) for index in overridden),
                "non_lifecycle_override_slots": ",".join(
                    str(index) for index in non_lifecycle
                ),
                "override_targets": ",".join(
                    f"{index}:0x{own_slots[index]:08X}" for index in overridden
                ),
                "classification": classification,
                "proof_status": "first_90_inherited_slots_only",
            }
        )
    summary = {
        "monster_runtime_classes": len(report_rows),
        "inherited_slots_compared": MONSTER_INHERITED_VTABLE_SLOTS,
        "lifecycle_slots_excluded_from_behavior": sorted(MONSTER_LIFECYCLE_SLOTS),
        "post_init_hook_slot": MONSTER_POST_INIT_SLOT,
        "classification_counts": dict(sorted(counts.items())),
    }
    return report_rows, summary


def scan_xitem_subsystem_anchors(exe: Path) -> list[dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    text_sections = [
        section.get_data()
        for section in pe.sections
        if section.Characteristics & 0x20000000
    ]
    rows = []
    seen = set()
    for match in re.finditer(rb"[\x09\x0A\x0D\x20-\x7E]{4,300}\x00", data):
        raw = match.group(0)[:-1]
        if b"XItem" not in raw or b"::" not in raw:
            continue
        try:
            text = raw.decode("ascii")
            text = text.replace("\r", "\\r").replace("\n", "\\n").replace("\t", "\\t")
            string_va = file_offset_to_va(pe, match.start())
        except (UnicodeDecodeError, Exception):
            continue
        key = (string_va, text)
        if key in seen:
            continue
        seen.add(key)
        encoded = struct.pack("<I", string_va)
        rows.append(
            {
                "string_va": f"0x{string_va:08X}",
                "text_xrefs": sum(section.count(encoded) for section in text_sections),
                "text": text,
            }
        )
    rows.sort(key=lambda row: (row["text"], row["string_va"]))
    return rows


def scan_entity_names(hashcodes: Path) -> list[EntityName]:
    rows = []
    for line_number, line in enumerate(
        hashcodes.read_text(encoding="utf-8", errors="replace").splitlines(), 1
    ):
        match = ENTITY_DEFINE_RE.match(line.strip())
        if not match:
            continue
        rows.append(
            EntityName(
                hashcode=int(match.group(2), 16),
                name=match.group(1),
                source_line=line_number,
            )
        )
    return rows


def write_tsv(path: Path, fieldnames: Iterable[str], rows: Iterable[dict]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(fieldnames), delimiter="\t")
        writer.writeheader()
        writer.writerows(rows)


def hex_or_empty(value: int | None) -> str:
    return "" if value is None else f"0x{value:08X}"


def main() -> int:
    args = parse_args()
    args.output.mkdir(parents=True, exist_ok=True)

    classes = scan_runtime_classes(args.exe, args.project_root)
    physics_vtable_rows, physics_vtable_summary = scan_physics_vtable_slots(
        args.exe, classes
    )
    animator_vtable_rows, animator_vtable_summary = scan_animator_vtable_slots(
        args.exe, classes
    )
    monster_vtable_rows, monster_vtable_summary = scan_monster_vtable_diffs(
        args.exe, classes
    )
    subsystem_anchors = scan_xitem_subsystem_anchors(args.exe)
    entities = scan_entity_names(args.hashcodes)

    class_json = [asdict(row) for row in classes]
    (args.output / "xitem_runtime_classes.json").write_text(
        json.dumps(class_json, indent=2), encoding="utf-8"
    )
    write_tsv(
        args.output / "xitem_runtime_classes.tsv",
        (
            "family",
            "name",
            "descriptor_va",
            "parent_descriptor_va",
            "parent_name",
            "object_size",
            "constructor_va",
            "vtable_va",
            "descriptor_xrefs",
            "documented",
            "source_referenced",
            "coverage",
            "behavior_status",
            "proof_status",
        ),
        (
            {
                "family": row.family,
                "name": row.name,
                "descriptor_va": hex_or_empty(row.descriptor_va),
                "parent_descriptor_va": hex_or_empty(row.parent_descriptor_va),
                "parent_name": row.parent_name or "",
                "object_size": f"0x{row.object_size:X}",
                "constructor_va": hex_or_empty(row.constructor_va),
                "vtable_va": hex_or_empty(row.vtable_va),
                "descriptor_xrefs": row.descriptor_xrefs,
                "documented": int(row.documented),
                "source_referenced": int(row.source_referenced),
                "coverage": row.coverage,
                "behavior_status": row.behavior_status,
                "proof_status": row.proof_status,
            }
            for row in classes
        ),
    )

    coverage_counts: dict[tuple[str, str], int] = {}
    for row in classes:
        key = (row.family, row.coverage)
        coverage_counts[key] = coverage_counts.get(key, 0) + 1
    write_tsv(
        args.output / "xitem_runtime_coverage_summary.tsv",
        ("family", "coverage", "count"),
        (
            {"family": family, "coverage": coverage, "count": count}
            for (family, coverage), count in sorted(coverage_counts.items())
        ),
    )

    write_tsv(
        args.output / "xitem_physics_vtable_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "slot_index",
            "slot_offset",
            "function_va",
            "parent_function_va",
            "is_override",
            "semantic_role",
            "proof_status",
        ),
        physics_vtable_rows,
    )
    (args.output / "xitem_physics_vtable_summary.json").write_text(
        json.dumps(physics_vtable_summary, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "exitem_animator_vtable_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "slot_index",
            "slot_offset",
            "function_va",
            "parent_function_va",
            "is_override",
            "semantic_role",
            "proof_status",
        ),
        animator_vtable_rows,
    )
    (args.output / "exitem_animator_vtable_summary.json").write_text(
        json.dumps(animator_vtable_summary, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "player_ball_surface_roles.tsv",
        (
            "owner_class",
            "owner_vtable",
            "slot_index",
            "slot_offset",
            "function_va",
            "source_field",
            "source_mask",
            "surface_role",
            "ball_flag_field",
            "ball_flag_mask",
            "response_parameters",
            "proof_anchor",
        ),
        PLAYER_BALL_SURFACE_ROLES,
    )
    (args.output / "player_ball_surface_roles.json").write_text(
        json.dumps(PLAYER_BALL_SURFACE_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "monster_handler_vtable_diff.tsv",
        (
            "name",
            "parent_name",
            "object_size",
            "constructor_va",
            "vtable_va",
            "parent_vtable_va",
            "shares_parent_vtable",
            "inherited_slots_compared",
            "overridden_slots",
            "non_lifecycle_override_slots",
            "override_targets",
            "classification",
            "proof_status",
        ),
        monster_vtable_rows,
    )
    (args.output / "monster_handler_vtable_summary.json").write_text(
        json.dumps(monster_vtable_summary, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "xitem_subsystem_anchors.tsv",
        ("string_va", "text_xrefs", "text"),
        subsystem_anchors,
    )

    write_tsv(
        args.output / "ht_entity_hashdb.tsv",
        ("hashcode", "name", "source_line"),
        (
            {
                "hashcode": f"0x{row.hashcode:08X}",
                "name": row.name,
                "source_line": row.source_line,
            }
            for row in entities
        ),
    )

    dev_rows = [
        {
            "launcher_level_id": level_id,
            "edb_uid": f"0x{uid:08X}",
            "label": label,
            "source_edb": source,
            "trigger_count": trigger_count,
        }
        for level_id, uid, label, source, trigger_count in DEV_MAPS
    ]
    write_tsv(
        args.output / "dev_map_registry.tsv",
        ("launcher_level_id", "edb_uid", "label", "source_edb", "trigger_count"),
        dev_rows,
    )
    (args.output / "dev_map_registry.json").write_text(
        json.dumps(dev_rows, indent=2), encoding="utf-8"
    )

    summary = {
        "runtime_classes": len(classes),
        "xitem_handlers": sum(row.family == "XItemHandler" for row in classes),
        "xitem_physics": sum(row.family == "XItemPhysics" for row in classes),
        "exitem_animators": sum(row.family == "EXItemAnimator" for row in classes),
        "exitem_renderers": sum(row.family == "EXItemRender" for row in classes),
        "partial": sum(row.coverage == "partial" for row in classes),
        "diagnostic": sum(row.coverage == "diagnostic" for row in classes),
        "structural": sum(row.coverage == "structural" for row in classes),
        "unresolved": sum(row.coverage == "unresolved" for row in classes),
        "physics_vtable_rows": physics_vtable_summary["vtable_rows"],
        "physics_parent_overrides": physics_vtable_summary["parent_overrides"],
        "physics_instruction_linked_roles": physics_vtable_summary[
            "instruction_linked_roles"
        ],
        "animator_vtable_rows": animator_vtable_summary["vtable_rows"],
        "animator_parent_overrides": animator_vtable_summary["parent_overrides"],
        "animator_instruction_linked_roles": animator_vtable_summary[
            "instruction_linked_roles"
        ],
        "player_ball_surface_roles": len(PLAYER_BALL_SURFACE_ROLES),
        "monster_runtime_classes": len(monster_vtable_rows),
        "monster_shared_parent_vtable": monster_vtable_summary["classification_counts"].get(
            "shared_parent_vtable", 0
        ),
        "monster_post_init_hook_only": monster_vtable_summary["classification_counts"].get(
            "post_init_hook_only", 0
        ),
        "monster_specialized_virtual_overrides": monster_vtable_summary[
            "classification_counts"
        ].get("specialized_virtual_overrides", 0),
        "xitem_subsystem_anchors": len(subsystem_anchors),
        "ht_entity_names": len(entities),
        "dev_maps": len(DEV_MAPS),
    }
    (args.output / "runtime_census_summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
