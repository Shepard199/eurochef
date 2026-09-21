use super::monster_transporter::TYPE as MONSTER_TRANSPORTER_TYPE;
use super::sweeper_boss::{
    sweeper_ratchet_anchor, NativeSweeperBossEyeSpawnSource,
    NativeSweeperBossTransporterSpawnConfig, NativeSweeperBossTransporterSpawnTarget,
    NativeSweeperRollerbotPathGraph, CONTROLLER_TYPE, EYE_COUNT, EYE_TYPE,
};
use crate::maps::{entities::robots_character_runtime_type, ProcessedMap};

/// Shipped Bo5_Final fallback graph used by the RollerBot created from the Sweeper pattern table.
/// This is a map resource identity, not a replay/tick oracle.
const ROLLERBOT_PATH_HASH: u32 = 0x0B00_0049;

/// Immutable map-side bindings needed by the generic Sweeper runtime. The controller's serialized
/// link order is authoritative for Eye ordinals; callers must not rediscover Eye0..Eye4 by spatial
/// sorting or by scanning all type86 triggers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossMapBindings {
    pub controller_trigger_index: usize,
    pub transporter_trigger_index: usize,
    pub transporter_spawn: NativeSweeperBossTransporterSpawnConfig,
    pub transporter_spawn_position_local: [f32; 3],
    pub initial_ratchet_yaw: f32,
    pub eye_trigger_indices: [usize; EYE_COUNT],
    pub ratchet_anchors: [[f32; 4]; EYE_COUNT],
    pub eye_spawn_sources: [NativeSweeperBossEyeSpawnSource; EYE_COUNT],
    pub roller_path_graph: NativeSweeperRollerbotPathGraph,
}

pub(crate) fn resolve_sweeper_boss_map_bindings(
    map: &ProcessedMap,
) -> Option<NativeSweeperBossMapBindings> {
    let mut controllers = map
        .triggers
        .iter()
        .enumerate()
        .filter(|(_, trigger)| trigger.ttype == CONTROLLER_TYPE);
    let (controller_trigger_index, controller) = controllers.next()?;
    if controllers.next().is_some() {
        return None;
    }

    let local_center = map.sweeper_ratchet_position_local?;
    let mut eye_trigger_indices = [0usize; EYE_COUNT];
    let mut ratchet_anchors = [[0.0f32; 4]; EYE_COUNT];
    let mut eye_spawn_sources = [NativeSweeperBossEyeSpawnSource::default(); EYE_COUNT];
    for eye_ordinal in 0..EYE_COUNT {
        let trigger_index = usize::try_from(*controller.links.get(eye_ordinal)?).ok()?;
        if eye_trigger_indices[..eye_ordinal].contains(&trigger_index) {
            return None;
        }
        let eye = map.triggers.get(trigger_index)?;
        if eye.ttype != EYE_TYPE {
            return None;
        }
        eye_trigger_indices[eye_ordinal] = trigger_index;
        ratchet_anchors[eye_ordinal] = sweeper_ratchet_anchor(
            [eye.position.x, eye.position.y, eye.position.z],
            [eye.rotation.x, eye.rotation.y, eye.rotation.z],
            local_center,
        );
        eye_spawn_sources[eye_ordinal] = NativeSweeperBossEyeSpawnSource {
            owner_position: [eye.position.x, eye.position.y, eye.position.z, 1.0],
            owner_yaw: eye.rotation.y,
        };
    }

    let transporter_trigger_index = usize::try_from(*controller.links.get(EYE_COUNT)?).ok()?;
    let transporter = map.triggers.get(transporter_trigger_index)?;
    if transporter.ttype != MONSTER_TRANSPORTER_TYPE {
        return None;
    }
    let mut transporter_targets = [None; 4];
    let mut transporter_targets_complete = true;
    for (target_ordinal, link_slot) in (4usize..8).enumerate() {
        let link = transporter.links.get(link_slot).copied().unwrap_or(-1);
        let Ok(trigger_index) = usize::try_from(link) else {
            continue;
        };
        let Some(target) = map.triggers.get(trigger_index) else {
            transporter_targets_complete = false;
            continue;
        };
        match robots_character_runtime_type(target.ttype) {
            Some(5) => {
                let Some(config_index) = target
                    .data
                    .first()
                    .copied()
                    .flatten()
                    .and_then(|value| u8::try_from(value).ok())
                else {
                    transporter_targets_complete = false;
                    continue;
                };
                transporter_targets[target_ordinal] =
                    Some(NativeSweeperBossTransporterSpawnTarget {
                        trigger_index,
                        config_index,
                        position: [target.position.x, target.position.y, target.position.z, 1.0],
                        yaw: target.rotation.y,
                    });
            }
            // Native 0x00469450 filters all non-AI links before touching RNG.
            None => {}
            // Other AI runtime families are valid native candidates but this boss runtime currently
            // hydrates only runtime-type5 MonsterDatabase selectors, so keep them fail-closed.
            Some(_) => transporter_targets_complete = false,
        }
    }
    let transporter_spawn = NativeSweeperBossTransporterSpawnConfig {
        targets: transporter_targets,
        targets_complete: transporter_targets_complete,
        max_successful_spawns: transporter.data.get(3).copied().flatten()?,
        max_carried_monsters: transporter.data.get(5).copied().flatten()?,
    };
    let transporter_spawn_position_local = map.sweeper_transporter_spawn_position_local?;

    let mut roller_paths = map
        .paths
        .iter()
        .filter(|path| path.hashcode == ROLLERBOT_PATH_HASH);
    let roller_path = roller_paths.next()?;
    if roller_paths.next().is_some() {
        return None;
    }
    let node_positions = roller_path
        .nodes
        .iter()
        .map(|node| [node.position.x, node.position.y, node.position.z])
        .collect::<Vec<_>>();
    let roller_path_graph =
        NativeSweeperRollerbotPathGraph::new(&node_positions, &roller_path.links)?;

    Some(NativeSweeperBossMapBindings {
        controller_trigger_index,
        transporter_trigger_index,
        transporter_spawn,
        transporter_spawn_position_local,
        initial_ratchet_yaw: controller.rotation.y + std::f32::consts::PI,
        eye_trigger_indices,
        ratchet_anchors,
        eye_spawn_sources,
        roller_path_graph,
    })
}
