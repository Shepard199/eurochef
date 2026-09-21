use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScrambledHitConfig {
    pub start_anim_mode: u32,
    pub loop_anim_mode: u32,
    pub end_anim_mode: u32,
    pub loop_ticks: i32,
    pub priority: u8,
    pub required_query_flag: u32,
}

impl RobotsScrambledHitConfig {
    pub const fn malfbot() -> Self {
        Self {
            start_anim_mode: 0x0900_0076,
            loop_anim_mode: 0x0900_0078,
            end_anim_mode: 0x0900_0077,
            loop_ticks: 180,
            priority: 0x55,
            required_query_flag: 0x0000_0008,
        }
    }

    /// EP02 primary BehaviorHost builder `0x00460A40`.
    pub const fn ep02_primary() -> Self {
        Self {
            start_anim_mode: 0,
            loop_anim_mode: 0x0900_0001,
            end_anim_mode: 0,
            loop_ticks: 5,
            priority: 0x55,
            required_query_flag: 0x0000_0008,
        }
    }

    /// EP02 secondary BehaviorHost builder `0x00460A40`.
    pub const fn ep02_secondary() -> Self {
        Self {
            start_anim_mode: 0,
            loop_anim_mode: 0x0900_0003,
            end_anim_mode: 0,
            loop_ticks: 5,
            priority: 0x55,
            required_query_flag: 0x0000_0008,
        }
    }

    /// EP06 Handler+0x4DC host, builder `0x004619A0`.
    pub const fn ep06_primary() -> Self {
        Self {
            start_anim_mode: 0,
            loop_anim_mode: 0x0900_0087,
            end_anim_mode: 0,
            loop_ticks: 5,
            priority: 0x55,
            required_query_flag: 0x0000_0008,
        }
    }

    /// EP06 Handler+0x4C4 host, builder `0x004619A0`.
    pub const fn ep06_secondary() -> Self {
        Self {
            start_anim_mode: 0,
            loop_anim_mode: 0x0900_0088,
            end_anim_mode: 0,
            loop_ticks: 5,
            priority: 0x55,
            required_query_flag: 0x0000_0008,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScrambledHitRuntimeState {
    pub config: RobotsScrambledHitConfig,
    pub active: bool,
    pub completed: bool,
    pub current_anim_mode: u32,
    pub remaining_loop_ticks: i32,
}

impl Default for RobotsScrambledHitRuntimeState {
    fn default() -> Self {
        Self::new(RobotsScrambledHitConfig::malfbot())
    }
}

impl RobotsScrambledHitRuntimeState {
    pub const fn new(config: RobotsScrambledHitConfig) -> Self {
        Self {
            config,
            active: false,
            completed: false,
            current_anim_mode: 0,
            remaining_loop_ticks: config.loop_ticks,
        }
    }

    pub fn priority(&self, got_hit_latch: bool, query_flags: u32) -> u8 {
        if self.active && !self.completed {
            return self.config.priority;
        }
        if got_hit_latch && query_flags & self.config.required_query_flag != 0 {
            self.config.priority
        } else {
            1
        }
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.completed = false;
        self.current_anim_mode = if self.config.start_anim_mode != 0 {
            self.config.start_anim_mode
        } else {
            self.config.loop_anim_mode
        };
        self.remaining_loop_ticks = self.config.loop_ticks;
    }

    pub fn leave(&mut self) {
        self.active = false;
    }

    pub fn setup_idle(&mut self, got_hit_latch: &mut bool) {
        if self.config.start_anim_mode != 0 && self.current_anim_mode == self.config.start_anim_mode
        {
            self.current_anim_mode = self.config.loop_anim_mode;
        } else if self.config.end_anim_mode != 0
            && self.current_anim_mode == self.config.end_anim_mode
        {
            self.completed = true;
            *got_hit_latch = false;
        }
    }

    /// Native `AI_ScrambledHit::Execute 0x004582E0` decrements before testing
    /// `< 0`, so a configured count of five completes on the sixth loop Execute.
    pub fn step(&mut self, got_hit_latch: &mut bool) -> u32 {
        if self.current_anim_mode == self.config.loop_anim_mode {
            self.remaining_loop_ticks -= 1;
            if self.remaining_loop_ticks < 0 {
                if self.config.end_anim_mode != 0 {
                    self.current_anim_mode = self.config.end_anim_mode;
                } else {
                    self.completed = true;
                    *got_hit_latch = false;
                }
            }
        }
        if self.current_anim_mode == 0 {
            self.completed = true;
            *got_hit_latch = false;
        }
        self.current_anim_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ep02_loop_only_variants_complete_on_sixth_execute() {
        for config in [
            RobotsScrambledHitConfig::ep02_primary(),
            RobotsScrambledHitConfig::ep02_secondary(),
        ] {
            let mut state = RobotsScrambledHitRuntimeState::new(config);
            let mut got_hit = true;
            state.enter();
            for _ in 0..5 {
                assert_eq!(state.step(&mut got_hit), config.loop_anim_mode);
                assert!(!state.completed);
            }
            let _ = state.step(&mut got_hit);
            assert!(state.completed);
            assert!(!got_hit);
        }
    }
}
