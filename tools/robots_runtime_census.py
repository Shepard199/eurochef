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

ENTITY_HASHDB_TUPLE_RE = re.compile(
    r'^\(\s*0x([0-9A-Fa-f]{8})\s*,\s*"(HT_Entity_[A-Za-z0-9_]+)"\s*\),?\s*$'
)

DEV_MAPS = (
    (55, 0x01000037, "Mechanics", "m00_demo.edb", 72, False, "mechanics regression corpus"),
    (15, 0x0100000F, "Enemies", "m99_enem.edb", 45, False, "enemy regression corpus"),
    (160, 0x010000A0, "NPCs", "m98_npcs.edb", 22, True, "canonical clean NPC logic corpus"),
    (116, 0x01000074, "Ball", "m00_ball.edb", 12, False, "ball mechanics regression corpus"),
    (1, 0x01000001, "Empty Aunt Fanny House", "m00_mapt.edb", 1, False, "empty-map/player baseline"),
)

EXACT_VTABLES = {
    "XItemHandler": 0x005DD2B8,
    "XItemHandler_BossExec": 0x005EFC20,
    "XItemHandler_Player": 0x005EEC08,
    "XItemHandler_Track": 0x005F32D8,
    "XItemHandler_Vehicle": 0x005DFA28,
    "XItemHandler_Fluid": 0x005DDEB8,
    "XItemHandler_FixSwitch": 0x005DEBF0,
    "XItemHandler_Hittable": 0x005DE650,
    "XItemHandler_Projectile": 0x005DF2B0,
    "XItemHandler_Mine": 0x005DF390,
    "XItemHandler_RefractionProjectile": 0x005DF470,
    "XItemHandler_HomingProjectile": 0x005DF550,
    "XItemHandler_ProjectileRaycast": 0x005DF630,
    "XItemHandler_ProjectileRicochet": 0x005DF710,
    "XItemHandler_ProjectileHomingScrap": 0x005DF7F0,
    "XItemHandler_MonsterProjectile": 0x005F0308,
    "XItemHandler_SlimeProjectile": 0x005F03E8,
    "XItemHandler_ScramblerMissile": 0x005F2CC0,
    "XItemHandler_ElectroBombMissile": 0x005F2DA0,
    "XItemHandler_Npc_Fender": 0x005E71B8,
    "XExplosionFragment": 0x005F1388,
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
    "EXItemRender": 0x005F3D40,
    "EXItemRender_SkinAnim": 0x005F3D38,
}

CLASS_COVERAGE = {
    "XItemHandler": (
        "diagnostic",
        "base_contact_callback_interface_with_native_noop_default",
    ),
    "XItemHandler_Script": ("partial", "script_timeline_and_entity_resolution"),
    "XItemHandler_Platform": ("partial", "trigger_path_rotation_and_event_preview"),
    "XItemHandler_Lift": ("partial", "trigger_path_and_event_preview"),
    "XItemHandler_Vehicle": ("partial", "path_yaw_wheels_steering_and_linear_contact_carry"),
    "XItemHandler_Fan": ("diagnostic", "native_live_axis_rotation_consumer_proven"),
    "XItemHandler_FanHorizontal": ("diagnostic", "native_live_axis_rotation_consumer_proven"),
    "XItemHandler_Pickup": ("partial", "native_handler_lifetime_proximity_periodic_scheduler_and_script_contract"),
    "XItemHandler_ElectricityExplosion": ("partial", "effect_control_and_particle_geometry"),
    "XExplosionFragment": (
        "partial",
        "fx03_fragment_rng_projectile_physics_contact_fade_staggered_teardown_and_dynamic_pickup",
    ),
    "XItemHandler_Explosion": (
        "partial",
        "fx03_definition_scheduler_animated_fragment_generation_and_common_hitquery_init",
    ),
    "XItemHandler_Fluid": (
        "diagnostic",
        "water_grid_wave_simulation_and_secondary_linear_motion_accumulator_response",
    ),
    "XItemHandler_FixSwitch": (
        "partial",
        "native_trigger_progress_dirty_latch_and_handler_owned_progress_contract",
    ),
    "XItemHandler_Projectile": (
        "diagnostic",
        "projectile_state_machine_and_exact_generate_explosion_dispatch",
    ),
    "XItemHandler_SlimeProjectile": (
        "diagnostic",
        "projectile_hit_owner_latch_consumer_and_slime_specific_explosion_generation",
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
    "XItemHandler_Npc": ("production", "live_behavior_focus_mission_cutscene_dialogue_and_character_motion_host"),
    "XItemHandler_Npc_Fender": ("production", "base_npc_behavior_identical_0x170_vtable_destructors_and_setup_thunk_only"),
    "XItemHandler_Monster": ("diagnostic", "monster_trigger_getters_path_and_proximity_context"),
    "XItemHandler_Test_Monster": ("diagnostic", "test_monster_trigger_getter_context"),
    "XItemHandler_Monster_EF03_EvilBot": ("production", "common_monster_builder_0x00466FB0_side_effects_vtable_0x005E68D8"),
    "XItemHandler_Monster_EM07_PiranhaBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "XItemHandler_Monster_EW11_FatBot": ("production", "common_monster_builder_0x00466A80_vtable_0x005E6770"),
    "XItemHandler_Monster_TestAnimBot": ("diagnostic", "exact_base_monster_vtable_context"),
    "EXItemAnimator_Anim": ("partial", "anim_and_animskin_frame_preview"),
    "EXItemAnimator_AnimModifier": (
        "diagnostic",
        "spine_and_head_tracking_helpers_fp_idle_gate_and_left_right_pose_mirror",
    ),
    "EXItemAnimator_Entity": ("partial", "entity_assembly_and_vehicle_wheel_transforms"),
    "EXItemAnimator_Map": (
        "partial",
        "map_geometry_preview_and_native_collision_query_consumers",
    ),
    "EXItemAnimator_ForceFeedback": (
        "diagnostic",
        "forcefeedback_hash_factory_exact_vtable_with_base_animator_behavior",
    ),
    "EXItemAnimator_Collision": (
        "diagnostic",
        "single_collision_datum_init_owner_pose_refresh_append_and_spatial_query_dispatch",
    ),
    "EXItemAnimator_DynLight": (
        "diagnostic",
        "dynamic_light_record_attach_transform_color_update_and_detach",
    ),
    "EXItemAnimator_Sound": ("diagnostic", "sound_resource_bind_reset_and_audio_manager_update"),
    "EXItemAnimator_Particle": ("partial", "native_particle_command_preview"),
    "EXItemAnimator_Script": ("partial", "script_timeline_and_geometry_preview"),
    "EXItemRender": ("diagnostic", "two_slot_descriptor_and_deleting_destructor_base"),
    "EXItemRender_SkinAnim": ("partial", "animskin_render_path"),
    "XItemPhysics": (
        "diagnostic",
        "base_fixed_step_integrator_and_collision_dispatch",
    ),
    "XItemPhysicsSphere": (
        "diagnostic",
        "fixed_step_displacement_mesh_contact_classification_platform_attachment_and_mass_restitution_sphere_impulse",
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
    "XItemPhysics_Platform": (
        "partial",
        "peer_registration_linear_and_angular_carry_with_temporal_script_entity_pose_cache",
    ),
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
    4: "world_geometry_query_contact_face_build_classify_and_three_pass_projection",
    12: "peer_exitemphysics_contact_callback",
    16: "platform_relative_script_entity_pose_delta_consumer",
    23: "current_physics_contact_callback",
    25: "collision_record_handled_flag",
    26: "contact_callback_primary",
    27: "contact_callback_secondary",
}
HANDLER_EVENT_SLOT = 5
HANDLER_SCRIPT_COMMAND_SLOT = 23
HANDLER_EVENT_FAMILY_CASES = (
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000001",
        "event_name": "HT_ScriptEvents_SetupIdle",
        "payload_contract": "none",
        "native_action": "clear AI idle/status latches and dispatch state helper value 3",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000007",
        "event_name": "HT_ScriptEvents_SetScriptValue",
        "payload_contract": "f32@event+0x14 -> native ftol -> Handler+0x45C low byte",
        "native_action": "store script-value byte",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x1600000A",
        "event_name": "HT_ScriptEvents_HitCheck",
        "payload_contract": "full Event command consumed by 0x00425990",
        "native_action": "build hit-check context and update Handler hit-check record list",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000021",
        "event_name": "HT_ScriptEvents_Attach_Swoosh",
        "payload_contract": "u32@event+0x14, u32@event+0x18",
        "native_action": "delegate to Handler vslot +0xA4",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000022",
        "event_name": "HT_ScriptEvents_Attach_Particle",
        "payload_contract": "u32@event+0x14, u32@event+0x18",
        "native_action": "delegate to Handler vslot +0xA4",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000026",
        "event_name": "HT_ScriptEvents_CreateExplosion",
        "payload_contract": "none",
        "native_action": "call common AI explosion path 0x004550A0(handler,0,1)",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x16000035",
        "event_name": "HT_ScriptEvents_Footstep",
        "payload_contract": "u32@event+0x14",
        "native_action": "delegate to Handler vslot +0xF4",
    },
    {
        "handler_target": "0x00453160",
        "event_uid": "0x1600003F",
        "event_name": "HT_ScriptEvents_Dettach_Swoosh",
        "payload_contract": "u32@event+0x14",
        "native_action": "delegate to Handler vslot +0xB0",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000001",
        "event_name": "HT_ScriptEvents_SetupIdle",
        "payload_contract": "none",
        "native_action": "call Player setup-idle path 0x004BBC10",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000004",
        "event_name": "HT_ScriptEvents_ChangeAnimMode",
        "payload_contract": "u32@event+0x14; sentinel 0/-1 ignored",
        "native_action": "dispatch Handler vslot +0xDC with requested AnimMode",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000006",
        "event_name": "HT_ScriptEvents_ForceIdleMode",
        "payload_contract": "none",
        "native_action": "dispatch state helper value 3 and normalize Player state byte +0x6DE",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000007",
        "event_name": "HT_ScriptEvents_SetScriptValue",
        "payload_contract": "f32@event+0x14 -> native ftol -> Handler+0x45C low byte",
        "native_action": "store script-value byte",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x1600000A",
        "event_name": "HT_ScriptEvents_HitCheck",
        "payload_contract": "full Event command plus owner EXItem",
        "native_action": "set Handler+0x6D4 bit0x20 and update embedded hit-check state through 0x00425990",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x1600001E",
        "event_name": "HT_ScriptEvents_SetWeaponValue",
        "payload_contract": "full Event command and Script animator",
        "native_action": "delegate to optional Handler+0x6EC component vslot +0x10",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x1600001F",
        "event_name": "HT_ScriptEvents_CreateProjectile",
        "payload_contract": "full Event command and Script animator",
        "native_action": "delegate to optional Handler+0x6EC component vslot +0x10",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000021",
        "event_name": "HT_ScriptEvents_Attach_Swoosh",
        "payload_contract": "u32@event+0x14, u32@event+0x18",
        "native_action": "delegate to Handler vslot +0xA4",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000022",
        "event_name": "HT_ScriptEvents_Attach_Particle",
        "payload_contract": "u32@event+0x14, u32@event+0x18",
        "native_action": "delegate to Handler vslot +0xA4",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000023",
        "event_name": "HT_ScriptEvents_Attach_ParticlesToSkeleton",
        "payload_contract": "u32@event+0x14/+0x18/+0x1C/+0x20",
        "native_action": "delegate four payload words to Handler vslot +0xA8",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000028",
        "event_name": "HT_ScriptEvents_PlayerAttackBranch",
        "payload_contract": "f32@event+0x14 threshold, u32@event+0x18 branch AnimMode",
        "native_action": "on native threshold pass clear Handler+0x6D4 bit0x80000 and dispatch vslot +0xDC with branch AnimMode",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x16000035",
        "event_name": "HT_ScriptEvents_Footstep",
        "payload_contract": "u32@event+0x14",
        "native_action": "delegate to Handler vslot +0xF4",
    },
    {
        "handler_target": "0x004AF9C0",
        "event_uid": "0x1600003F",
        "event_name": "HT_ScriptEvents_Dettach_Swoosh",
        "payload_contract": "u32@event+0x14",
        "native_action": "delegate to Handler vslot +0xB0",
    },
    {
        "handler_target": "0x00415460",
        "event_uid": "0xFFFFFFFF",
        "event_name": "native_internal_sentinel",
        "payload_contract": "no Event payload; requires current Script animator identity match",
        "native_action": "dispatch Handler vslot +0xD4 only for matching owner Script animator",
    },
    {
        "handler_target": "0x004B6F50",
        "event_uid": "0x16000001",
        "event_name": "HT_ScriptEvents_SetupIdle",
        "payload_contract": "none",
        "native_action": "run first-person state machine keyed by Handler+0x6F8; states 1/2 dispatch HT_AnimMode_FP_Move, state 5 dispatches HT_AnimMode_FP_Activate or idle fallback, state 3 runs cleanup/re-entry, state 4 is quiet",
    },
    {
        "handler_target": "0x004B6F50",
        "event_uid": "0x1600001E",
        "event_name": "HT_ScriptEvents_SetWeaponValue",
        "payload_contract": "full Event command and Script animator",
        "native_action": "delegate to optional Handler+0x6EC component vslot +0x10",
    },
    {
        "handler_target": "0x00456EF0",
        "event_uid": "*",
        "event_name": "all_script_events",
        "payload_contract": "ignored",
        "native_action": "native no-op: xor eax,eax; ret 8",
    },
    {
        "handler_target": "0x00464EA0",
        "event_uid": "0x16000001",
        "event_name": "HT_ScriptEvents_SetupIdle",
        "payload_contract": "none",
        "native_action": "if Handler+0x640 state is 1 set 3; if state is 2 set 0; otherwise fall through to shared AI target 0x00453160",
    },
    {
        "handler_target": "0x00464EA0",
        "event_uid": "0x16000004",
        "event_name": "HT_ScriptEvents_ChangeAnimMode",
        "payload_contract": "u32@event+0x14",
        "native_action": "set Handler+0x648=1 and Handler+0x64C=requested AnimMode",
    },
    {
        "handler_target": "0x00464EA0",
        "event_uid": "*",
        "event_name": "fallback",
        "payload_contract": "preserve Event command and Script animator",
        "native_action": "delegate all other events to shared AI/Monster/NPC target 0x00453160",
    },
    {
        "handler_target": "0x00490B40",
        "event_uid": "*",
        "event_name": "all_script_events",
        "payload_contract": "full Event command and Script animator",
        "native_action": "delegate to current WatchBot component selected by Handler+0x4A0 from Handler+0x490[] via component vslot +0x30",
    },
    {
        "handler_target": "0x004CAA30",
        "event_uid": "0x16000034",
        "event_name": "HT_ScriptEvents_WaitForState",
        "payload_contract": "u32@event+0x14; specialized predicate only when value == 12",
        "native_action": "for value 12 return whether Handler+0x2EC byte is zero; otherwise delegate to base Handler target 0x00402ED0",
    },
    {
        "handler_target": "0x004CAA30",
        "event_uid": "*",
        "event_name": "fallback",
        "payload_contract": "preserve Event command and Script animator",
        "native_action": "delegate all non-specialized cases to base Handler target 0x00402ED0",
    },
    {
        "handler_target": "0x004CBCE0",
        "event_uid": "0x16000001",
        "event_name": "HT_ScriptEvents_SetupIdle",
        "payload_contract": "none",
        "native_action": "when Handler+0x6A0 state is 6 clear linked runtime fields, set state 2, then delegate SetupIdle through shared AI target 0x00453160; otherwise use AI fallback",
    },
    {
        "handler_target": "0x004CBCE0",
        "event_uid": "0x1600000A",
        "event_name": "HT_ScriptEvents_HitCheck",
        "payload_contract": "full Event command plus Script animator",
        "native_action": "when Handler+0x6A0 state is 3 consume one global RNG draw, select native pattern modulo 5 and update linked hit-check record/state 4; state 5 runs 0x004CBAE0; other states fall through to shared AI target 0x00453160",
    },
    {
        "handler_target": "0x004CBCE0",
        "event_uid": "*",
        "event_name": "fallback",
        "payload_contract": "preserve Event command and Script animator",
        "native_action": "delegate remaining events to shared AI/Monster/NPC target 0x00453160",
    },
    {
        "handler_target": "0x004CE940",
        "event_uid": "0x16000007",
        "event_name": "HT_ScriptEvents_SetScriptValue",
        "payload_contract": "f32@event+0x14 -> native ftol; Script animator identity selects destination",
        "native_action": "write converted value to Handler+0x3D0 or +0x3D4 for the two tracked Script animators; otherwise write Handler+0x3CC",
    },
    {
        "handler_target": "0x004CE940",
        "event_uid": "*",
        "event_name": "unhandled",
        "payload_contract": "ignored",
        "native_action": "return 0 without side effect",
    },
    {
        "handler_target": "0x004CFB00",
        "event_uid": "0x16000007",
        "event_name": "HT_ScriptEvents_SetScriptValue",
        "payload_contract": "f32@event+0x14 -> native ftol -> Handler+0x45C low byte",
        "native_action": "store Ratchet script-value byte and return handled=1",
    },
    {
        "handler_target": "0x004CFB00",
        "event_uid": "0x1600001F",
        "event_name": "HT_ScriptEvents_CreateProjectile",
        "payload_contract": "uses gameplay Player global and Ratchet owner EXItem",
        "native_action": "call native Ratchet missile creation path 0x004D01F0 and return 0",
    },
    {
        "handler_target": "0x004CFB00",
        "event_uid": "*",
        "event_name": "unhandled",
        "payload_contract": "ignored",
        "native_action": "return handled=1 without additional side effect",
    },
    {
        "handler_target": "0x004D0480",
        "event_uid": "0x1600001D",
        "event_name": "HT_ScriptEvents_WaitForHit",
        "payload_contract": "full Event command and Script animator",
        "native_action": "while 10-second lifetime remains active delegate to proven wait-for-hit latch consumer 0x004172C0; expired pickup returns 0",
    },
    {
        "handler_target": "0x004D0480",
        "event_uid": "0x1600002E",
        "event_name": "HT_ScriptEvents_InventoryAdd",
        "payload_contract": "full InventoryAdd Event command",
        "native_action": "clear linked pickup Script owner +0x10C; while lifetime remains active delegate to base Handler inventory/full-heal path, expired pickup returns 0 without granting inventory",
    },
    {
        "handler_target": "0x004D0480",
        "event_uid": "*",
        "event_name": "fallback",
        "payload_contract": "preserve Event command and Script animator",
        "native_action": "delegate remaining or expired cases to base Handler target 0x00402ED0",
    },
)

HANDLER_CONTACT_SLOT = 26
HANDLER_CONTACT_NOOPS = {0x0041CEC0, 0x004190C0}
HANDLER_HIT_CALLBACK_SLOT = 50

PHYSICS_CLASS_SLOT_ROLES = {
    ("XItemPhysicsSphere", 0): "sphere_fixed_step_integrator",
    ("XItemPhysicsSphere", 2): "runtime_descriptor",
    ("XItemPhysicsSphere", 6): "mass_and_restitution_weighted_sphere_sphere_impulse_response",
    ("XItemPhysicsSphere", 14): "fixed_step_displacement_setter",
    ("XItemPhysicsSphere", 15): "fixed_step_displacement_getter",
    ("XItemPhysicsSphere", 16): "platform_attachment_secondary_update",
    ("XItemPhysicsSphere", 22): "static_face_metadata_surface_classification_extent_update_and_contact_response",
    ("XItemPhysicsSphere", 23): "sphere_contact_motion_state_correction",
    ("XItemPhysics_Character", 0): "base_integrator_wrapper",
    ("XItemPhysics_Interactive", 0): "contact_state_update",
    ("XItemPhysics_Interactive", 12): "interactive_contact_dispatch",
    ("XItemPhysics_Interactive", 30): "contact_owner_setter",
    ("XItemPhysics_Interactive", 31): "player_or_magnetic_box_contact_filter",
    ("XItemPhysics_PickupAttract", 0): "target_attraction_update",
    ("XItemPhysics_Platform", 0): "moving_platform_update",
    ("XItemPhysics_Platform", 12): "platform_contact_dispatch",
    ("XItemPhysics_Platform", 26): "platform_peer_attachment_registration",
    ("XItemPhysics_Platform", 27): "platform_peer_body_origin_velocity_transfer_and_reference_pose_cache_seed",
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
    ("EXItemAnimator_AnimModifier", 4): "spine_modifier_head_tracking_fp_idle_gate_and_left_right_pose_mirror_update",
    ("EXItemAnimator_Collision", 8): "collision_datum_runtime_shape_scale_refresh_and_base_update",
    ("EXItemAnimator_Collision", 11): "collision_datum_append_and_base_dispatch",
    ("EXItemAnimator_Collision", 12): "collision_datum_transform_and_spatial_query_dispatch",
    ("EXItemAnimator_DynLight", 5): "dynamic_light_transform_color_update",
    ("EXItemAnimator_DynLight", 6): "bind_accept_true",
    ("EXItemAnimator_Sound", 4): "sound_transition_and_audio_state_reset",
    ("EXItemAnimator_Sound", 5): "sound_runtime_audio_manager_update",
    ("EXItemAnimator_Sound", 6): "sound_resource_bind_and_runtime_state_init",
    ("EXItemAnimator_Sound", 8): "sound_audio_state_reset",
    ("EXItemAnimator_Map", 4): "map_noop_hook",
    ("EXItemAnimator_Map", 5): "map_bind_gate_false",
    ("EXItemAnimator_Map", 6): "owner_resource_bind_and_transform_cache",
    ("EXItemAnimator_Map", 14): "map_geometry_query_variant_a_narrowphase_0051A943",
    ("EXItemAnimator_Map", 15): "map_geometry_query_variant_b_narrowphase_00519CB8",
    ("EXItemAnimator_Map", 16): "map_transformed_query_geometry_dispatch",
    ("EXItemAnimator_Map", 17): "map_single_mutable_query_record_dispatch",
    ("EXItemAnimator_Map", 18): "map_transformed_world_geometry_query_dispatch",
    ("EXItemAnimator_Entity", 14): "entity_geometry_query_variant_a_dispatch",
    ("EXItemAnimator_Entity", 15): "entity_geometry_query_variant_b_dispatch",
    ("EXItemAnimator_Entity", 16): "entity_transformed_query_geometry_dispatch",
    ("EXItemAnimator_Entity", 17): "entity_single_mutable_query_record_dispatch",
    ("EXItemAnimator_Entity", 18): "entity_transformed_world_geometry_query_dispatch",
    ("EXItemAnimator_Script", 14): "script_child_geometry_query_variant_a_forwarding",
    ("EXItemAnimator_Script", 15): "script_child_geometry_query_variant_b_forwarding",
    ("EXItemAnimator_Script", 16): "script_child_transformed_query_geometry_forwarding",
    ("EXItemAnimator_Script", 18): "script_child_transformed_world_geometry_query_forwarding",
    ("EXItemAnimator_Map", 25): "owner_transform_and_timeline_evaluation",
}


SPHERE_RESPONSE_PARAMETER_ROLES = (
    {
        "field_offset": "+0xE4",
        "runtime_role": "collision_sphere_radius",
        "default_raw": "0.0 (must be supplied before normalized runtime use)",
        "player_ball_raw": "0.98",
        "initializer": "0x00509E2D -> 0x00509DBC",
        "consumer": "0x0050A912; 0x0050B4A4; 0x00509DBC",
        "proof_anchor": "contact lever is constructed as -contact_normal * radius; angular tangential coupling is normalized by radius squared",
    },
    {
        "field_offset": "+0xEC",
        "runtime_role": "mass",
        "default_raw": "1.0",
        "player_ball_raw": "1.0",
        "initializer": "0x00509E2D -> 0x00509DBC",
        "consumer": "0x0050BD80",
        "proof_anchor": "effective mass = 1 / (1 / mass_a + 1 / mass_b); velocity deltas are +/- impulse / own_mass",
    },
    {
        "field_offset": "+0xF0",
        "runtime_role": "one_plus_restitution_normal_response_multiplier",
        "default_raw": "0.0 -> runtime 1.0",
        "player_ball_raw": "0.1 -> runtime 1.1",
        "initializer": "0x00509DBC adds exact 1.0",
        "consumer": "0x0050BD80; 0x0050B4A4",
        "proof_anchor": "sphere-sphere solve averages both runtime factors then multiplies closing normal velocity before the effective-mass solve",
    },
    {
        "field_offset": "+0xF4",
        "runtime_role": "linear_tangential_contact_response_scale",
        "default_raw": "0.5 -> runtime 0.25",
        "player_ball_raw": "1.0 normal / 0.2 slippery -> runtime 0.5 / 0.1",
        "initializer": "0x00509DBC: runtime = min(raw * 0.5, 0.5)",
        "consumer": "0x0050A130",
        "proof_anchor": "multiplies the already-normal-removed tangential contact velocity before adding it to linear motion",
    },
    {
        "field_offset": "+0xF8",
        "runtime_role": "angular_tangential_slip_coupling_per_radius_squared",
        "default_raw": "0.5 -> runtime (1 - 0.5) / radius^2",
        "player_ball_raw": "0.7 normal / 0.3 slippery",
        "initializer": "0x00509DBC: runtime = (1 - raw) / radius^2",
        "consumer": "0x0050A130",
        "proof_anchor": "multiplies cross(tangential_slip, contact_lever) before updating rotational state",
    },
)


SPHERE_SURFACE_CATEGORY_ROLES = (
    {"face_metadata_masked": "0x08", "runtime_category": "0x0001", "runtime_role": "slippery_floor_primary", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBall 0x004B2D20", "proof_anchor": "Slippery balls"},
    {"face_metadata_masked": "0x10", "runtime_category": "0x0002", "runtime_role": "electric_floor", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBall 0x004B2D20", "proof_anchor": "Electric Floor!!!"},
    {"face_metadata_masked": "0x38", "runtime_category": "0x0004", "runtime_role": "playerball_death_surface", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBall slot74 0x004B4F10", "proof_anchor": "HT_Sound_SFX_RODNEY_VOX_SCREAM_START; state byte -> 0x30; 0x004C09B0/0x004C0A20 classify state 0x30 as Player death"},
    {"face_metadata_masked": "0x28", "runtime_category": "0x0020", "runtime_role": "slime_floor", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBall 0x004B2D20", "proof_anchor": "Slime Floor!!!"},
    {"face_metadata_masked": "fallback with source bit 0x40", "runtime_category": "0x0040", "runtime_role": "playerball_chase_oily_surface", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBallChase slot107 0x004C4F20", "proof_anchor": "PBF_BALL_CHASE_OILY"},
    {"face_metadata_masked": "0x60", "runtime_category": "0x0080", "runtime_role": "unresolved_surface_category_dormant_in_shipped_face_info", "producer": "0x0041C2C0", "consumer": "no direct consumer proven", "proof_anchor": "0x0041C2C0 is reached only from Sphere slot22; its 0x44 records are built only by 0x004EFF1C -> 0x004F4EFC from Mesh +0x68 face_info, and the full 1,732,091-face shipped corpus OR-closure cannot produce exact 0x60"},
    {"face_metadata_masked": "0x68", "runtime_category": "0x0100", "runtime_role": "slippery_floor_secondary", "producer": "0x0041C2C0", "consumer": "XItemHandler_PlayerBall 0x004B2D20 (+0x1E5 bit0)", "proof_anchor": "Slippery balls"},
)


SPHERE_STATIC_CONTACT_RESPONSE_ROLES = (
    {"role": "near_horizontal_floor_normal_snap", "function_va": "0x0050B4A4", "condition": "Sphere+0x158 bit0x08 clear and normalized contact normal Y > 0.995", "response": "replace contact normal with exact (0,1,0)", "proof_anchor": "constant 0x005F41BC = 0.995"},
    {"role": "downward_impact_restitution_gate", "function_va": "0x0050B4A4", "condition": "fixed-step Y motion <= -0.1 and runtime (1 + restitution) >= 1.01", "response": "multiply closing normal response by runtime (1 + restitution)", "proof_anchor": "constants 0x005E18EC = -0.1 and 0x005F12C4 = 1.01"},
    {"role": "wall_or_non_downward_projection_response", "function_va": "0x0050B4A4", "condition": "fixed-step Y motion > -0.1 or runtime (1 + restitution) < 1.01", "response": "plain normal projection/correction without restitution amplification", "proof_anchor": "same exact branch as downward impact gate"},
)


EXITEM_XITEM_OWNERSHIP_ROLES = (
    {
        "owner_class": "EXItem",
        "descriptor_va": "0x005F33F0",
        "object_size": "0x170",
        "field": "+0x14C",
        "role": "handler_owner_link",
        "attach_function": "0x004E8028",
        "reverse_link": "Handler+0x04 = EXItem",
        "teardown": "0x004E7E9A virtual Handler teardown",
        "proof_anchor": "0x004E8028 writes EXItem+0x14C and Handler+0x04; base destructor clears/destroys +0x14C",
    },
    {
        "owner_class": "EXItem",
        "descriptor_va": "0x005F33F0",
        "object_size": "0x170",
        "field": "+0x150",
        "role": "physics_owner_link",
        "attach_function": "0x004E803C",
        "reverse_link": "Physics+0x04 = EXItem",
        "teardown": "0x004E7E9A virtual Physics teardown",
        "proof_anchor": "0x004E803C writes EXItem+0x150 and Physics+0x04; base destructor clears/destroys +0x150",
    },
    {
        "owner_class": "XItem",
        "descriptor_va": "0x005E18A8",
        "object_size": "0x270",
        "field": "inherits EXItem +0x14C/+0x150",
        "role": "gameplay_item_owner",
        "attach_function": "0x00443BD0 -> EXItem base construction",
        "reverse_link": "inherits Handler/Physics reverse-owner links",
        "teardown": "0x00443CC0 -> 0x004E7E9A",
        "proof_anchor": "descriptor string XItem at 0x006183D0; parent descriptor 0x005F33F0 string EXItem at 0x0061F960",
    },
)


PHYSICS_SCHEDULER_STAGE_ROLES = (
    {
        "stage": 1,
        "function_va": "0x00444C20 -> XItem slot11 0x004E831D",
        "selector": "main XItem list; XItem+0x150 != null; Physics+0x08 bit0x20 clear",
        "dispatch": "Physics slot0",
        "role": "physics_fixed_step_update",
        "proof_anchor": "0x004E831D loads XItem+0x150, tests Physics+0x08 & 0x20, then JMP [Physics.vtable+0x00]",
    },
    {
        "stage": 2,
        "function_va": "0x00444C20",
        "selector": "physics list; XItem+0x150 != null; Physics+0x08 bit0x02 clear",
        "dispatch": "Physics slot4",
        "role": "world_geometry_query_contact_solve",
        "proof_anchor": "0x00444D2C loads XItem+0x150 and 0x00444D3D calls [Physics.vtable+0x10]",
    },
    {
        "stage": 3,
        "function_va": "0x00444C20",
        "selector": "XItem+0x26D bit0x02 set; attached object derives from XItemPhysics or XItemPhysicsSphere",
        "dispatch": "Physics slot16",
        "role": "physics_secondary_platform_relative_update",
        "proof_anchor": "0x00444D7F loads XItem+0x150, descriptor-walk checks 0x005DFB00/0x005DFB10, then 0x00444DC1 calls [Physics.vtable+0x40]",
    },
    {
        "stage": 4,
        "function_va": "0x00444E30 -> EXItem slot10 0x004E8188",
        "selector": "EXItem state flags +0x10/+0x11",
        "dispatch": "Handler slots +0x0C/+0x10 through EXItem+0x14C",
        "role": "handler_state_phase",
        "proof_anchor": "0x004E8188 reads EXItem+0x14C and conditionally calls Handler virtual +0x0C/+0x10; this phase is separate from Physics ownership/scheduling",
    },
)


XITEM_MANAGER_REGISTRATION_ROLES = (
    {
        "role": "registry_layout",
        "function_va": "0x004E9779; derived ctor 0x004446A0",
        "field": "manager+0x84 / manager+0x88",
        "value": "+0x84 has 1 slot; +0x88 has 5 slots in shipped derived manager",
        "proof_anchor": "base ctor allocates (arg1+1) and (arg2+5) arrays of 8-byte list slots; 0x004446A0 calls it with arg1=0,arg2=0",
    },
    {
        "role": "main_xitem_registry_sorted_insert",
        "function_va": "0x004E9A1C",
        "field": "manager+0x84[0]; XItem+0x162 priority",
        "value": "sorted XItem registration",
        "proof_anchor": "inserts through 0x0053878D/0x0053876C, notifies XItem+0x144 components, applies mask through 0x004E7F46 and stores factory priority",
    },
    {
        "role": "secondary_mask_registry_transform",
        "function_va": "0x004E7F46",
        "field": "XItem+0x100 mask; manager+0x88[index]; XItem+0x104+12*index nodes",
        "value": "one intrusive registry channel per changed mask bit",
        "proof_anchor": "changed bits add/remove 12-byte payload nodes through 0x00538904; node payload is the XItem; new mask is written back to +0x100",
    },
    {
        "role": "collision_contact_registry_channel",
        "function_va": "0x00444C20",
        "field": "registration mask bit0 -> manager+0x88[0]",
        "value": "bit0 required for scheduler Physics slot4 pass",
        "proof_anchor": "0x00444D23 walks manager+0x88[0], extracts node XItem, loads XItem+0x150 and dispatches Physics vtable+0x10",
    },
    {
        "role": "tball_traffic_registry_channel",
        "function_va": "0x004E3E00",
        "field": "registration mask bit2 (0x04)",
        "value": "representative TBall traffic/race/joyride/traffic-pattern factory channel; no Physics attachment",
        "proof_anchor": "factory selects XItemHandler_TBall_Traffic/Race/JoyRide/TrafficPattern, registers mask=0x04 and never calls 0x004E803C",
    },
    {
        "role": "registry_remove",
        "function_va": "0x004E9AAB",
        "field": "main + secondary registries",
        "value": "mask cleared before main-registry unlink",
        "proof_anchor": "calls 0x004E7F46 with zero mask, then unlinks main manager+0x84 registration",
    },
)


GAMEPLAY_BODY_BOOTSTRAP_ROLES = (
    {
        "role": "owner_pose_is_fixed_step_source_of_truth",
        "function_va": "XItemPhysics slot0 0x00419400",
        "source": "owner XItem+0xD0..0xDC and +0xE0..0xEC via Physics+0x04",
        "destination": "Physics+0x8C..0x98 and +0x9C..0xA8",
        "ordering": "at the start of each base fixed-step update",
        "proof_anchor": "0x00419472..0x004194C2 copies both owner pose blocks into Physics snapshots",
    },
    {
        "role": "physics_integrates_back_into_owner_pose",
        "function_va": "XItemPhysics slot0 0x00419400",
        "source": "Physics accumulated linear/angular state",
        "destination": "owner XItem+0xD0..0xDC and +0xE0..0xEC",
        "ordering": "during the same fixed-step update",
        "proof_anchor": "0x00419815..0x00419905 and later update owner transform fields and clear owner+0x7E pose-dirty byte",
    },
    {
        "role": "pre_registration_pose_cache_seed_is_factory_specific",
        "function_va": "0x00482850; 0x00409300",
        "source": "XItem pose",
        "destination": "Physics+0x8C..0x98",
        "ordering": "before 0x004E803C/0x004E9A1C in these mask=0x05 factories",
        "proof_anchor": "Character/Cutscene-style factories explicitly seed Physics pose before attach/register; this is not universal",
    },
    {
        "role": "registration_pose_order_is_not_universal",
        "function_va": "CreateRatchetMissile 0x004D01F0",
        "source": "projectile XItem + Physics",
        "destination": "mask=0x01 registration followed by later launch pose and ballistic motion initializer 0x0041EC80",
        "ordering": "attach/register occurs before final launch pose in this factory",
        "proof_anchor": "0x004D0311 attach, 0x004D0320 register mask1/priority0x14, 0x004D0373..0x004D0391 writes XItem pose, then 0x004D03D0 calls 0x0041EC80",
    },
    {
        "role": "minimal_collision_registration_requirement",
        "function_va": "0x004E7F46 + 0x00444C20",
        "source": "XItem registration mask",
        "destination": "manager+0x88[0] Physics work list",
        "ordering": "after XItem/Physics bidirectional attach and before scheduler contact phase",
        "proof_anchor": "Physics factories use mask 0x01 or 0x05; both include bit0, while mask0 bodies skip the slot4 registry and mask0x04 TBall factory has no Physics",
    },
)


MAP_COLLISION_DATUM_ROLES = (
    {
        "role": "physics_map_collision_query",
        "function_va": "XItemPhysics slot0 0x00419400",
        "source": "owner XItem component graph",
        "behavior": "queries hash 0x10000004 HT_AnimDatum_MapCollisionCapsule through XItem virtual +0x1C and caches the converted shape at Physics+0x1D8",
        "status": "proven_static",
        "proof_anchor": "0x00419400 pushes 0x10000004 before owner virtual +0x1C and calls 0x004D64C0 on a successful datum result",
    },
    {
        "role": "xitem_component_datum_dispatch",
        "function_va": "XItem virtual +0x1C 0x004E850E",
        "source": "XItem+0x144 component list",
        "behavior": "iterates components and calls component slot12 (+0x30) for the requested AnimDatum hash",
        "status": "proven_static",
        "proof_anchor": "component-provider chain used by the base Physics HT_AnimDatum_MapCollisionCapsule request",
    },
    {
        "role": "script_collision_provider",
        "function_va": "EXItemAnimator_Collision slot12 0x00567861; slot8 0x005678FA",
        "source": "Collision animator 0x30-byte datum at object+0x110",
        "behavior": "serves the map-collision datum; slot8 refreshes datum +0x20/+0x24/+0x28 from animator runtime scale XYZ before normal transform/update",
        "status": "proven_static+corpus",
        "proof_anchor": "all six shipped opcode13 Collision commands are fx03_exp.edb mode1 sphere records with serialized seed [0.5,0,0]",
    },
    {
        "role": "entity_static_datum_provider",
        "function_va": "EXItemAnimator_Entity slot12 0x00507219",
        "source": "resolved Entity optional directory ID5",
        "behavior": "resolves directory through 0x00504D0B(5), scans 0x30-byte records by first-u32 AnimDatum hash and transforms the hit through 0x005391E8/0x0053914F",
        "status": "proven_static+implemented_parser",
        "proof_anchor": "Robots PC v248 Entity directory ID5 parser matches native mask/relative-pointer layout and passes the full shipped Entity corpus",
    },
    {
        "role": "entity_animdatum_serialized_layout",
        "function_va": "0x0053914F / 0x005391E8",
        "source": "Entity ID5 0x30-byte record",
        "behavior": "+00 hash; +04 raw u16; +06 shape mode; +07 raw byte; +08/+0C/+10 shape scalars; +14/+18/+1C local center; +20..+2C local quaternion",
        "status": "proven_static+implemented_parser",
        "proof_anchor": "runtime transformer copies these exact serialized fields into the common map-collision datum layout",
    },
    {
        "role": "animskin_serialized_datum_provider",
        "function_va": "EXItemAnimator_Anim slot12 0x004F4990 -> 0x00500569",
        "source": "serialized EXGeoBaseAnimSkin +0x60 count / +0x64 self-relative AnimDatum channel",
        "behavior": "0x00500569 follows AnimSkin +0x64, reads byte+5 count and i16+6 index pointer, scans 8-byte {u32 hash,u16 skip_count,i16 datum_rel} records, and transforms the referenced datum through 0x005391E8",
        "status": "proven_static+implemented_parser+corpus",
        "proof_anchor": "eb01_spi AnimSkin 0x0D000001 object0x1460 has +0x64 self-relative 0x5F64 -> block0x7428; index0x7444 hash0x10000004 rel0x82 -> exact mode3 datum0x74CC",
    },
    {
        "role": "animskin_datum_transform_selector",
        "function_va": "0x00500569",
        "source": "AnimDatum payload +0x30",
        "behavior": "uses byte +0x30 * 0x10 to select the current AnimSkin bone-transform matrix before 0x005391E8 transforms the local datum",
        "status": "proven_static+corpus",
        "proof_anchor": "all 669 serialized AnimSkin datums preserve an in-range selector and hierarchy tail; Rodney MapCollision selector remains separate from trigger/root pose",
    },
    {
        "role": "animskin_map_collision_shipped_corpus",
        "function_va": "eurochef-edb AnimSkin v248 corpus regression",
        "source": "234 shipped AnimSkins",
        "behavior": "93 nonempty +0x60/+0x64 sections; 669 raw index records, 644 native-searchable heads; 87 raw MapCollision records = 84 mode3 capsules + 3 mode1 spheres; searchable MapCollision = 83 capsules + 3 spheres",
        "status": "proven_corpus",
        "proof_anchor": "p01_rod AnimSkin 0x0D000001 searchable body is mode3 half_segment=0.45 radius=0.40 local_center=[0,0.85,0]; 78 EDBs expose searchable MapCollision and all 5 multi-skin EDBs use identical geometry",
    },
    {
        "role": "maps_character_body_bootstrap",
        "function_va": "EuroChef Maps RuntimeCharacterBodyState",
        "source": "MonsterDatabase-resolved character EDB + searchable AnimSkin HT_AnimDatum_MapCollisionCapsule",
        "behavior": "creates a separate gameplay-body state from trigger/XItem owner pose plus native local sphere/capsule profile with registration mask bit0; explicitly does not use editor camera/player preview",
        "status": "implemented+real_edb_regression",
        "proof_anchor": "m02_city resolves 77/77 runtime-created characters with native collision profiles and 77/77 RuntimeCharacterBodyState instances at registration mask 0x01",
    },
    {
        "role": "map_collision_world_transform",
        "function_va": "0x00538963",
        "source": "common 0x30-byte datum + Entity/Animator transform",
        "behavior": "transforms center, composes orientation quaternion and scales shape scalar lanes +0x20/+0x24/+0x28 by maximum transform scale",
        "status": "proven_static",
        "proof_anchor": "shared by Collision/Entity datum providers before Physics shape conversion",
    },
    {
        "role": "map_collision_sphere_conversion",
        "function_va": "0x004D64C0 -> 0x00539A40",
        "source": "datum shape_mode != 3",
        "behavior": "produces compact sphere representation from center vec4 plus datum+0x20 radius",
        "status": "proven_static",
        "proof_anchor": "0x004D6800 dispatches result type0 through sphere/sphere and sphere/capsule intersection routines",
    },
    {
        "role": "map_collision_capsule_conversion",
        "function_va": "0x004D64C0 -> 0x00539A63",
        "source": "datum shape_mode == 3",
        "behavior": "rotates local Y by quaternion, computes segment_start=center-axis*half_length, segment_delta=2*axis*half_length and radius=datum+0x24",
        "status": "proven_static",
        "proof_anchor": "datum+0x20 is half-segment; +0x24 is radius; 0x005DD938 is exact 2.0 multiplier; output type1 follows capsule intersection branches",
    },
    {
        "role": "script_collision_shipped_corpus",
        "function_va": "eurochef-cli script-health collision_commands.tsv",
        "source": "179 EDB shipped corpus",
        "behavior": "6/6 opcode13 Collision commands are mode0x01 sphere, all in fx03_exp.edb; no mode3 capsule commands",
        "status": "proven_corpus",
        "proof_anchor": "target/map_runtime_stage116_script_health/collision_commands.tsv",
    },
    {
        "role": "entity_map_collision_shipped_corpus",
        "function_va": "eurochef-cli entity-report ht_entity_anim_datums.tsv",
        "source": "2159 shipped local Entity headers",
        "behavior": "99 ID5 directories / 202 AnimDatum records / 7 HT_AnimDatum_MapCollisionCapsule records; all 7 are mode0x01 sphere and none are mode3 capsule",
        "status": "proven_corpus",
        "proof_anchor": "includes HT_Entity_MagneticBoxLarge radius1.0 and HT_Entity_MagneticBoxStandard radius0.659327984; remaining five are local Entity UIDs and stay unnamed",
    },
)


ANIMSKIN_POSE_UPDATE_ROLES = (
    {
        "role": "animator_anim_slot4_update_orchestrator",
        "function_va": "EXItemAnimator_Anim slot4 0x004F33FD",
        "source": "live EXItemAnimator_Anim state",
        "behavior": "runs active-clip selection 0x004F4BD1, controller update 0x004F336D, animation-layer pose accumulation 0x004F341B, then the base animator update",
        "status": "proven_static",
        "proof_anchor": "EXItemAnimator_Anim vtable 0x005F3AE0 slot4 is 0x004F33FD; AnimModifier overrides the same slot for its specialized pose modifier",
    },
    {
        "role": "animation_layer_pose_accumulation",
        "function_va": "0x004F341B",
        "source": "animation-layer linked nodes at animator +0x130...",
        "behavior": "initializes pose scratch state and calls each active node virtual +0x24 to decode/accumulate pose channels before optional blend/final flush",
        "status": "proven_static_decoder_entry",
        "proof_anchor": "0x004F341B traverses every active layer list, invokes node slot+0x24, calls 0x004F358B for blend cases and 0x004FE8AB when the final pose-dirty flag 0x4000 is set",
    },
    {
        "role": "animation_pose_blend",
        "function_va": "0x004F358B + 0x005099CA",
        "source": "two scratch pose buffers plus layer weight and optional bone mask",
        "behavior": "linearly interpolates translation vec4 lanes and performs shortest-path quaternion SLERP; applies either all bones or the active clip bitmask and also blends scalar channels",
        "status": "proven_static",
        "proof_anchor": "0x005099CA computes quaternion dot, flips sign for negative dot, derives angular sine weights and combines the two quaternions; 0x004F358B calls it per selected bone",
    },
    {
        "role": "skinanim_live_bone_record_flush",
        "function_va": "0x004FE8AB",
        "source": "final blended scratch pose DAT_007BE094",
        "behavior": "copies exactly eight DWORDs per bone into SkinAnim runtime record +0x18..+0x34 with source stride 0x20 and destination stride 0x48; clip flag 0x80 gates writes through its per-bone bitmask",
        "status": "proven_static",
        "proof_anchor": "destination starts record+0x18, writes translation vec4 + quaternion vec4, advances 0x12 DWORDs, then clears SkinAnim cache/dirty bytes +0x10 and +0x0E so matrices rebuild from the new pose",
    },
    {
        "role": "skinanim_current_matrix_rebuild",
        "function_va": "0x004FF298 / 0x004FEA72 / 0x0050139D",
        "source": "SkinAnim 0x48-byte live bone records and AnimDatum hierarchy chain",
        "behavior": "reads translation +0x18..+0x24 and quaternion +0x28..+0x34, composes hierarchy matrices and fills/uses the separate 0x40-per-bone matrix cache",
        "status": "proven_static",
        "proof_anchor": "0x004FC82C initializes records and identity matrix cache; these rebuilders consume the records after 0x004FE8AB flush",
    },
    {
        "role": "animdatum_world_transform_order",
        "function_va": "0x004F4990 -> 0x004F4911 -> 0x00500569 -> 0x004E5C6F -> 0x00538963",
        "source": "serialized AnimDatum + current selected bone matrix + owning animator/XItem transform",
        "behavior": "applies exact order AnimDatum-local -> current AnimSkin bone matrix -> animator/owner/world transform",
        "status": "proven_static",
        "proof_anchor": "0x004F4911 resolves family 0x10 through 0x00500569, then when query flags do not request local space builds owner transform through 0x004E5C6F and applies 0x00538963 again",
    },
    {
        "role": "maps_character_world_shape_kernel",
        "function_va": "EuroChef runtime_character_world_shape_from_bone_matrix",
        "source": "RuntimeCharacterBodyState + supplied current bone matrix",
        "behavior": "replays native bone-then-owner order, max-scale shape scaling and sphere/capsule conversion; current animated world shape remains gated on a native EDB pose decoder rather than RAPC",
        "status": "implemented_kernel+tests",
        "proof_anchor": "Rodney identity-bone capsule test and bone-then-owner/max-scale test pass; Inspector exposes only native pre-animation identity-bone shape until a live sampled matrix exists",
    },
)


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


def scan_handler_event_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    handler_rows = sorted(
        (row for row in classes if row.family == "XItemHandler"),
        key=lambda item: item.name,
    )
    base_handler = by_name.get("XItemHandler")
    base_slots = read_vtable_slots(
        pe,
        data,
        base_handler.vtable_va if base_handler else None,
        HANDLER_EVENT_SLOT + 1,
    )
    base_function_va = (
        base_slots[HANDLER_EVENT_SLOT]
        if len(base_slots) > HANDLER_EVENT_SLOT
        else None
    )
    report_rows: list[dict] = []
    event_family_targets: dict[str, int] = {}
    classes_using_base_target = 0
    classes_using_event_families = 0
    for row in handler_rows:
        own_slots = read_vtable_slots(pe, data, row.vtable_va, HANDLER_EVENT_SLOT + 1)
        function_va = (
            own_slots[HANDLER_EVENT_SLOT]
            if len(own_slots) > HANDLER_EVENT_SLOT
            else None
        )
        parent = by_name.get(row.parent_name or "")
        parent_slots = read_vtable_slots(
            pe,
            data,
            parent.vtable_va if parent and parent.family == "XItemHandler" else None,
            HANDLER_EVENT_SLOT + 1,
        )
        parent_function_va = (
            parent_slots[HANDLER_EVENT_SLOT]
            if len(parent_slots) > HANDLER_EVENT_SLOT
            else None
        )
        is_parent_override = bool(
            parent_function_va is not None and function_va != parent_function_va
        )
        is_base_target = bool(
            function_va is not None
            and base_function_va is not None
            and function_va == base_function_va
        )
        if is_base_target:
            classes_using_base_target += 1
        elif function_va is not None:
            classes_using_event_families += 1
            key = f"0x{function_va:08X}"
            event_family_targets[key] = event_family_targets.get(key, 0) + 1
        report_rows.append(
            {
                "name": row.name,
                "parent_name": row.parent_name or "",
                "vtable_va": hex_or_empty(row.vtable_va),
                "vtable_proof": (
                    "exact" if row.name in EXACT_VTABLES else "inferred_constructor_window"
                ),
                "function_va": hex_or_empty(function_va),
                "parent_function_va": hex_or_empty(parent_function_va),
                "is_parent_override": int(is_parent_override),
                "is_base_target": int(is_base_target),
                "classification": (
                    "missing_slot"
                    if function_va is None
                    else "base_event_target"
                    if is_base_target
                    else "event_family_target"
                ),
                "proof_status": "xitemhandler_slot5_script_event_dispatch",
            }
        )
    return report_rows, {
        "handler_runtime_classes": len(handler_rows),
        "slot_index": HANDLER_EVENT_SLOT,
        "slot_offset": f"0x{HANDLER_EVENT_SLOT * 4:02X}",
        "base_function_va": hex_or_empty(base_function_va),
        "classes_using_base_target": classes_using_base_target,
        "classes_using_event_families": classes_using_event_families,
        "unique_event_family_targets": len(event_family_targets),
        "event_family_targets": dict(sorted(event_family_targets.items())),
    }


def scan_handler_script_command_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    handler_rows = sorted(
        (row for row in classes if row.family == "XItemHandler"),
        key=lambda item: item.name,
    )
    report_rows: list[dict] = []
    command_family_targets: dict[str, int] = {}
    for row in handler_rows:
        own_slots = read_vtable_slots(
            pe, data, row.vtable_va, HANDLER_SCRIPT_COMMAND_SLOT + 1
        )
        function_va = (
            own_slots[HANDLER_SCRIPT_COMMAND_SLOT]
            if len(own_slots) > HANDLER_SCRIPT_COMMAND_SLOT
            else None
        )
        if function_va is not None:
            key = f"0x{function_va:08X}"
            command_family_targets[key] = command_family_targets.get(key, 0) + 1
        report_rows.append(
            {
                "name": row.name,
                "parent_name": row.parent_name or "",
                "vtable_va": hex_or_empty(row.vtable_va),
                "vtable_proof": (
                    "exact" if row.name in EXACT_VTABLES else "inferred_constructor_window"
                ),
                "function_va": hex_or_empty(function_va),
                "classification": (
                    "missing_slot" if function_va is None else "script_command_family_target"
                ),
                "proof_status": "xitemhandler_slot23_script_command_dispatch",
            }
        )
    return report_rows, {
        "handler_runtime_classes": len(handler_rows),
        "slot_index": HANDLER_SCRIPT_COMMAND_SLOT,
        "slot_offset": f"0x{HANDLER_SCRIPT_COMMAND_SLOT * 4:02X}",
        "unique_script_command_targets": len(command_family_targets),
        "script_command_targets": dict(sorted(command_family_targets.items())),
    }


def scan_handler_contact_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    handler_rows = sorted(
        (row for row in classes if row.family == "XItemHandler"),
        key=lambda item: item.name,
    )
    report_rows: list[dict] = []
    classifications: dict[str, int] = {}
    override_targets: dict[str, int] = {}
    for row in handler_rows:
        own_slots = read_vtable_slots(pe, data, row.vtable_va, HANDLER_CONTACT_SLOT + 1)
        function_va = (
            own_slots[HANDLER_CONTACT_SLOT]
            if len(own_slots) > HANDLER_CONTACT_SLOT
            else None
        )
        parent = by_name.get(row.parent_name or "")
        parent_slots = read_vtable_slots(
            pe,
            data,
            parent.vtable_va if parent and parent.family == "XItemHandler" else None,
            HANDLER_CONTACT_SLOT + 1,
        )
        parent_function_va = (
            parent_slots[HANDLER_CONTACT_SLOT]
            if len(parent_slots) > HANDLER_CONTACT_SLOT
            else None
        )
        is_override = bool(
            parent_function_va is not None and function_va != parent_function_va
        )
        vtable_is_exact = row.name in EXACT_VTABLES
        if function_va is None:
            classification = "missing_slot"
        elif function_va in HANDLER_CONTACT_NOOPS:
            classification = "native_noop"
        elif not vtable_is_exact:
            classification = "inferred_candidate_unpromoted"
        elif is_override or parent_function_va is None:
            classification = "specialized_contact_override"
        else:
            classification = "inherited_specialized_contact_callback"
        classifications[classification] = classifications.get(classification, 0) + 1
        if vtable_is_exact and function_va is not None and function_va not in HANDLER_CONTACT_NOOPS:
            key = f"0x{function_va:08X}"
            override_targets[key] = override_targets.get(key, 0) + 1
        report_rows.append(
            {
                "name": row.name,
                "parent_name": row.parent_name or "",
                "vtable_va": hex_or_empty(row.vtable_va),
                "vtable_proof": "exact" if vtable_is_exact else "inferred_constructor_window",
                "function_va": hex_or_empty(function_va),
                "parent_function_va": hex_or_empty(parent_function_va),
                "is_parent_override": int(is_override),
                "classification": classification,
                "proof_status": "xitemhandler_slot26_first_contact_dispatch",
            }
        )
    return report_rows, {
        "handler_runtime_classes": len(handler_rows),
        "slot_index": HANDLER_CONTACT_SLOT,
        "slot_offset": f"0x{HANDLER_CONTACT_SLOT * 4:02X}",
        "native_noop_functions": [f"0x{value:08X}" for value in sorted(HANDLER_CONTACT_NOOPS)],
        "classification_counts": dict(sorted(classifications.items())),
        "non_noop_targets": dict(sorted(override_targets.items())),
    }


def scan_handler_hit_callback_slots(
    exe: Path, classes: list[RuntimeClass]
) -> tuple[list[dict], dict]:
    """Census the Hittable-family vslot +0xC8 accepted-hit callback.

    XItemHandler_Hittable introduces slot50; unrelated direct subclasses can
    reuse the numeric offset for class-specific virtuals with different ABIs.
    Projectile 0x00414EF0 ends in RET 0x38 and is therefore not this callback.
    """

    pe = pefile.PE(str(exe), fast_load=False)
    data = exe.read_bytes()
    by_name = {row.name: row for row in classes}
    handler_rows = sorted(
        (row for row in classes if row.family == "XItemHandler"),
        key=lambda item: item.name,
    )
    report_rows: list[dict] = []
    target_classes: dict[str, list[str]] = {}
    semantic_callbacks = 0
    outside_hierarchy = 0
    outside_with_slot50 = 0
    exact_callbacks = 0
    inferred_callbacks = 0
    descendant_cache: dict[str, bool] = {}

    def executable_slot(vtable_va: int | None) -> int | None:
        slots = read_vtable_slots(pe, data, vtable_va, HANDLER_HIT_CALLBACK_SLOT + 1)
        if len(slots) <= HANDLER_HIT_CALLBACK_SLOT:
            return None
        function_va = slots[HANDLER_HIT_CALLBACK_SLOT]
        if not is_executable_rva(pe, function_va - pe.OPTIONAL_HEADER.ImageBase):
            return None
        return function_va

    def is_hittable_descendant(name: str) -> bool:
        cached = descendant_cache.get(name)
        if cached is not None:
            return cached
        if name == "XItemHandler_Hittable":
            descendant_cache[name] = True
            return True
        current = by_name.get(name)
        parent_name = current.parent_name if current else None
        result = bool(
            parent_name
            and parent_name in by_name
            and by_name[parent_name].family == "XItemHandler"
            and is_hittable_descendant(parent_name)
        )
        descendant_cache[name] = result
        return result

    for row in handler_rows:
        function_va = executable_slot(row.vtable_va)
        parent = by_name.get(row.parent_name or "")
        parent_function_va = executable_slot(
            parent.vtable_va if parent and parent.family == "XItemHandler" else None
        )
        hittable_descendant = is_hittable_descendant(row.name)
        vtable_is_exact = row.name in EXACT_VTABLES
        is_override = bool(
            function_va is not None
            and parent_function_va is not None
            and function_va != parent_function_va
        )

        if hittable_descendant:
            if function_va is None:
                raise RuntimeError(
                    f"Hittable descendant {row.name} has no executable +0xC8 callback"
                )
            classification = (
                "exact_hit_callback"
                if vtable_is_exact
                else "constructor_vtable_hit_callback"
            )
            semantic_callbacks += 1
            if vtable_is_exact:
                exact_callbacks += 1
            else:
                inferred_callbacks += 1
            key = f"0x{function_va:08X}"
            target_classes.setdefault(key, []).append(row.name)
        else:
            outside_hierarchy += 1
            if function_va is None:
                classification = "outside_hittable_hierarchy_no_slot50"
            else:
                classification = "outside_hittable_hierarchy_class_specific_slot50"
                outside_with_slot50 += 1

        report_rows.append(
            {
                "name": row.name,
                "parent_name": row.parent_name or "",
                "vtable_va": hex_or_empty(row.vtable_va),
                "vtable_proof": (
                    "exact" if vtable_is_exact else "inferred_constructor_window"
                ),
                "function_va": hex_or_empty(function_va),
                "parent_function_va": hex_or_empty(parent_function_va),
                "is_parent_override": int(is_override),
                "is_hittable_descendant": int(hittable_descendant),
                "classification": classification,
                "proof_status": (
                    "xitemhandler_hittable_slot50_accepted_hit_callback"
                    if hittable_descendant
                    else "slot50_not_hit_semantic_outside_hittable_hierarchy"
                ),
            }
        )

    callback_targets = {
        target: {
            "class_count": len(names),
            "classes": sorted(names),
        }
        for target, names in sorted(target_classes.items())
    }
    return report_rows, {
        "handler_runtime_classes": len(handler_rows),
        "slot_index": HANDLER_HIT_CALLBACK_SLOT,
        "slot_offset": f"0x{HANDLER_HIT_CALLBACK_SLOT * 4:02X}",
        "hittable_hierarchy_classes": semantic_callbacks,
        "semantic_hit_callback_classes": semantic_callbacks,
        "outside_hittable_hierarchy_classes": outside_hierarchy,
        "outside_hierarchy_with_executable_slot50": outside_with_slot50,
        "exact_vtable_callback_classes": exact_callbacks,
        "constructor_vtable_callback_classes": inferred_callbacks,
        "unique_hit_callback_targets": len(callback_targets),
        "hit_callback_targets": callback_targets,
    }


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
        stripped = line.strip()
        match = ENTITY_DEFINE_RE.match(stripped)
        if match:
            name = match.group(1)
            hashcode = int(match.group(2), 16)
        else:
            tuple_match = ENTITY_HASHDB_TUPLE_RE.match(stripped)
            if not tuple_match:
                continue
            hashcode = int(tuple_match.group(1), 16)
            name = tuple_match.group(2)
        rows.append(
            EntityName(
                hashcode=hashcode,
                name=name,
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
    handler_event_rows, handler_event_summary = scan_handler_event_slots(
        args.exe, classes
    )
    documented_event_targets = {row["handler_target"] for row in HANDLER_EVENT_FAMILY_CASES}
    missing_event_targets = sorted(
        set(handler_event_summary["event_family_targets"]) - documented_event_targets
    )
    if missing_event_targets:
        raise RuntimeError(
            "missing XItemHandler Event family case coverage for: "
            + ", ".join(missing_event_targets)
        )
    handler_script_command_rows, handler_script_command_summary = (
        scan_handler_script_command_slots(args.exe, classes)
    )
    handler_contact_rows, handler_contact_summary = scan_handler_contact_slots(
        args.exe, classes
    )
    handler_hit_callback_rows, handler_hit_callback_summary = (
        scan_handler_hit_callback_slots(args.exe, classes)
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
        args.output / "xitem_handler_event_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "vtable_proof",
            "function_va",
            "parent_function_va",
            "is_parent_override",
            "is_base_target",
            "classification",
            "proof_status",
        ),
        handler_event_rows,
    )
    (args.output / "xitem_handler_event_summary.json").write_text(
        json.dumps(handler_event_summary, indent=2), encoding="utf-8"
    )
    write_tsv(
        args.output / "xitem_handler_event_family_cases.tsv",
        (
            "handler_target",
            "event_uid",
            "event_name",
            "payload_contract",
            "native_action",
        ),
        HANDLER_EVENT_FAMILY_CASES,
    )
    write_tsv(
        args.output / "xitem_handler_script_command_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "vtable_proof",
            "function_va",
            "classification",
            "proof_status",
        ),
        handler_script_command_rows,
    )
    (args.output / "xitem_handler_script_command_summary.json").write_text(
        json.dumps(handler_script_command_summary, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "xitem_handler_contact_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "vtable_proof",
            "function_va",
            "parent_function_va",
            "is_parent_override",
            "classification",
            "proof_status",
        ),
        handler_contact_rows,
    )
    (args.output / "xitem_handler_contact_summary.json").write_text(
        json.dumps(handler_contact_summary, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "xitem_handler_hit_callback_slots.tsv",
        (
            "name",
            "parent_name",
            "vtable_va",
            "vtable_proof",
            "function_va",
            "parent_function_va",
            "is_parent_override",
            "is_hittable_descendant",
            "classification",
            "proof_status",
        ),
        handler_hit_callback_rows,
    )
    (args.output / "xitem_handler_hit_callback_summary.json").write_text(
        json.dumps(handler_hit_callback_summary, indent=2), encoding="utf-8"
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
        args.output / "exitem_physics_sphere_response_parameters.tsv",
        (
            "field_offset",
            "runtime_role",
            "default_raw",
            "player_ball_raw",
            "initializer",
            "consumer",
            "proof_anchor",
        ),
        SPHERE_RESPONSE_PARAMETER_ROLES,
    )
    (args.output / "exitem_physics_sphere_response_parameters.json").write_text(
        json.dumps(SPHERE_RESPONSE_PARAMETER_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "exitem_physics_sphere_surface_categories.tsv",
        (
            "face_metadata_masked",
            "runtime_category",
            "runtime_role",
            "producer",
            "consumer",
            "proof_anchor",
        ),
        SPHERE_SURFACE_CATEGORY_ROLES,
    )
    (args.output / "exitem_physics_sphere_surface_categories.json").write_text(
        json.dumps(SPHERE_SURFACE_CATEGORY_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "exitem_physics_sphere_static_contact_response.tsv",
        ("role", "function_va", "condition", "response", "proof_anchor"),
        SPHERE_STATIC_CONTACT_RESPONSE_ROLES,
    )
    (args.output / "exitem_physics_sphere_static_contact_response.json").write_text(
        json.dumps(SPHERE_STATIC_CONTACT_RESPONSE_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "exitem_xitem_ownership.tsv",
        (
            "owner_class",
            "descriptor_va",
            "object_size",
            "field",
            "role",
            "attach_function",
            "reverse_link",
            "teardown",
            "proof_anchor",
        ),
        EXITEM_XITEM_OWNERSHIP_ROLES,
    )
    (args.output / "exitem_xitem_ownership.json").write_text(
        json.dumps(EXITEM_XITEM_OWNERSHIP_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "physics_scheduler_stages.tsv",
        ("stage", "function_va", "selector", "dispatch", "role", "proof_anchor"),
        PHYSICS_SCHEDULER_STAGE_ROLES,
    )
    (args.output / "physics_scheduler_stages.json").write_text(
        json.dumps(PHYSICS_SCHEDULER_STAGE_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "xitem_manager_registration.tsv",
        ("role", "function_va", "field", "value", "proof_anchor"),
        XITEM_MANAGER_REGISTRATION_ROLES,
    )
    (args.output / "xitem_manager_registration.json").write_text(
        json.dumps(XITEM_MANAGER_REGISTRATION_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "gameplay_body_bootstrap.tsv",
        ("role", "function_va", "source", "destination", "ordering", "proof_anchor"),
        GAMEPLAY_BODY_BOOTSTRAP_ROLES,
    )
    (args.output / "gameplay_body_bootstrap.json").write_text(
        json.dumps(GAMEPLAY_BODY_BOOTSTRAP_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "map_collision_datum_roles.tsv",
        ("role", "function_va", "source", "behavior", "status", "proof_anchor"),
        MAP_COLLISION_DATUM_ROLES,
    )
    (args.output / "map_collision_datum_roles.json").write_text(
        json.dumps(MAP_COLLISION_DATUM_ROLES, indent=2), encoding="utf-8"
    )

    write_tsv(
        args.output / "animskin_pose_update_roles.tsv",
        ("role", "function_va", "source", "behavior", "status", "proof_anchor"),
        ANIMSKIN_POSE_UPDATE_ROLES,
    )
    (args.output / "animskin_pose_update_roles.json").write_text(
        json.dumps(ANIMSKIN_POSE_UPDATE_ROLES, indent=2), encoding="utf-8"
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
            "canonical_logic_source": canonical_logic_source,
            "evidence_role": evidence_role,
        }
        for level_id, uid, label, source, trigger_count, canonical_logic_source, evidence_role in DEV_MAPS
    ]
    write_tsv(
        args.output / "dev_map_registry.tsv",
        ("launcher_level_id", "edb_uid", "label", "source_edb", "trigger_count", "canonical_logic_source", "evidence_role"),
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
        "handler_event_base_target_classes": handler_event_summary["classes_using_base_target"],
        "handler_event_family_classes": handler_event_summary["classes_using_event_families"],
        "handler_event_unique_targets": handler_event_summary["unique_event_family_targets"],
        "handler_event_family_cases": len(HANDLER_EVENT_FAMILY_CASES),
        "handler_script_command_unique_targets": handler_script_command_summary[
            "unique_script_command_targets"
        ],
        "handler_contact_native_noop": handler_contact_summary["classification_counts"].get(
            "native_noop", 0
        ),
        "handler_contact_specialized_overrides": handler_contact_summary[
            "classification_counts"
        ].get("specialized_contact_override", 0),
        "handler_contact_inherited_specialized": handler_contact_summary[
            "classification_counts"
        ].get("inherited_specialized_contact_callback", 0),
        "handler_contact_non_noop_targets": len(handler_contact_summary["non_noop_targets"]),
        "handler_hit_callback_hittable_classes": handler_hit_callback_summary[
            "semantic_hit_callback_classes"
        ],
        "handler_hit_callback_outside_hittable_classes": handler_hit_callback_summary[
            "outside_hittable_hierarchy_classes"
        ],
        "handler_hit_callback_unique_targets": handler_hit_callback_summary[
            "unique_hit_callback_targets"
        ],
        "animator_vtable_rows": animator_vtable_summary["vtable_rows"],
        "animator_parent_overrides": animator_vtable_summary["parent_overrides"],
        "animator_instruction_linked_roles": animator_vtable_summary[
            "instruction_linked_roles"
        ],
        "sphere_response_parameter_roles": len(SPHERE_RESPONSE_PARAMETER_ROLES),
        "sphere_surface_category_roles": len(SPHERE_SURFACE_CATEGORY_ROLES),
        "sphere_static_contact_response_roles": len(SPHERE_STATIC_CONTACT_RESPONSE_ROLES),
        "exitem_xitem_ownership_roles": len(EXITEM_XITEM_OWNERSHIP_ROLES),
        "physics_scheduler_stages": len(PHYSICS_SCHEDULER_STAGE_ROLES),
        "xitem_manager_registration_roles": len(XITEM_MANAGER_REGISTRATION_ROLES),
        "gameplay_body_bootstrap_roles": len(GAMEPLAY_BODY_BOOTSTRAP_ROLES),
        "map_collision_datum_roles": len(MAP_COLLISION_DATUM_ROLES),
        "animskin_pose_update_roles": len(ANIMSKIN_POSE_UPDATE_ROLES),
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
