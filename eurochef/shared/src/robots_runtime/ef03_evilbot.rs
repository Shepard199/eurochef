use serde::Serialize;

pub const ROBOTS_EF03_FLY_LOOP_SOUND_UID: u32 = 0x1AF0_0155;
pub const ROBOTS_EF03_FLY_LOOP_NATIVE_PARAMETER: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEf03FlyLoopPlan {
    pub sound_uid: u32,
    pub native_parameter: u32,
}

/// EF03 first-update `0x00467540`: after the common Monster first update the
/// handler looks up the fly-loop sound and starts it only when no instance exists.
pub const fn ef03_first_update_fly_loop_plan(
    sound_instance_already_exists: bool,
) -> Option<RobotsEf03FlyLoopPlan> {
    if sound_instance_already_exists {
        None
    } else {
        Some(RobotsEf03FlyLoopPlan {
            sound_uid: ROBOTS_EF03_FLY_LOOP_SOUND_UID,
            native_parameter: ROBOTS_EF03_FLY_LOOP_NATIVE_PARAMETER,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_update_starts_fly_loop_only_when_missing() {
        assert_eq!(
            ef03_first_update_fly_loop_plan(false),
            Some(RobotsEf03FlyLoopPlan {
                sound_uid: 0x1AF0_0155,
                native_parameter: 100,
            })
        );
        assert_eq!(ef03_first_update_fly_loop_plan(true), None);
    }

}
