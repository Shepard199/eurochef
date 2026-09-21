use serde::Serialize;

pub const ROBOTS_AI_EVENT_THROTTLE_STALK_ID: u32 = 1;
pub const ROBOTS_AI_EVENT_THROTTLE_ATTACK_ID: u32 = 2;
pub const ROBOTS_AI_EVENT_THROTTLE_STALK_INTERVAL_SECONDS: f32 = 3.0;
pub const ROBOTS_AI_EVENT_THROTTLE_ATTACK_INTERVAL_SECONDS: f32 = 4.0;
pub const ROBOTS_AI_EVENT_THROTTLE_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct RobotsAiEventThrottleRecord {
    id: u32,
    interval_seconds: f32,
    last_grant_seconds: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsAiEventThrottleRuntime {
    current_seconds: f32,
    records: [RobotsAiEventThrottleRecord; 2],
}

impl Default for RobotsAiEventThrottleRuntime {
    fn default() -> Self {
        let current_seconds = 0.0;
        Self {
            current_seconds,
            records: [
                RobotsAiEventThrottleRecord {
                    id: ROBOTS_AI_EVENT_THROTTLE_STALK_ID,
                    interval_seconds: ROBOTS_AI_EVENT_THROTTLE_STALK_INTERVAL_SECONDS,
                    last_grant_seconds: current_seconds,
                },
                RobotsAiEventThrottleRecord {
                    id: ROBOTS_AI_EVENT_THROTTLE_ATTACK_ID,
                    interval_seconds: ROBOTS_AI_EVENT_THROTTLE_ATTACK_INTERVAL_SECONDS,
                    last_grant_seconds: current_seconds,
                },
            ],
        }
    }
}

impl RobotsAiEventThrottleRuntime {
    /// Process-global manager update from `0x004561F0`.
    pub fn advance_fixed(&mut self) {
        self.current_seconds += ROBOTS_AI_EVENT_THROTTLE_FIXED_STEP_SECONDS;
    }

    /// Exact keyed gate from `0x004565B0`.
    ///
    /// The comparison is strict: elapsed time must be greater than the
    /// configured interval. A successful request writes the manager's current
    /// time into the matching record. Unknown ids fail closed.
    pub fn request(&mut self, id: u32) -> bool {
        let Some(record) = self.records.iter_mut().find(|record| record.id == id) else {
            return false;
        };
        if self.current_seconds - record.last_grant_seconds <= record.interval_seconds {
            return false;
        }
        record.last_grant_seconds = self.current_seconds;
        true
    }

    pub const fn current_seconds(&self) -> f32 {
        self.current_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_ctor_installs_stalk_three_seconds_and_attack_four_seconds() {
        let runtime = RobotsAiEventThrottleRuntime::default();
        assert_eq!(runtime.records[0].id, 1);
        assert_eq!(runtime.records[0].interval_seconds.to_bits(), 3.0f32.to_bits());
        assert_eq!(runtime.records[1].id, 2);
        assert_eq!(runtime.records[1].interval_seconds.to_bits(), 4.0f32.to_bits());
        assert_eq!(runtime.records[0].last_grant_seconds.to_bits(), 0.0f32.to_bits());
        assert_eq!(runtime.records[1].last_grant_seconds.to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn request_uses_strict_native_elapsed_boundary_and_updates_only_matching_id() {
        let mut runtime = RobotsAiEventThrottleRuntime::default();
        for _ in 0..180 {
            runtime.advance_fixed();
        }
        assert!(!runtime.request(ROBOTS_AI_EVENT_THROTTLE_STALK_ID));
        runtime.advance_fixed();
        assert!(runtime.request(ROBOTS_AI_EVENT_THROTTLE_STALK_ID));
        assert!(!runtime.request(ROBOTS_AI_EVENT_THROTTLE_STALK_ID));

        let mut attack_runtime = RobotsAiEventThrottleRuntime::default();
        for _ in 0..240 {
            attack_runtime.advance_fixed();
        }
        assert!(!attack_runtime.request(ROBOTS_AI_EVENT_THROTTLE_ATTACK_ID));
        attack_runtime.advance_fixed();
        assert!(attack_runtime.request(ROBOTS_AI_EVENT_THROTTLE_ATTACK_ID));
        assert!(!attack_runtime.request(99));
    }
}
