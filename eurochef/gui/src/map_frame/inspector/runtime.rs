use super::super::*;

impl MapFrame {
    /// Inspector wrapper around the shared runtime LOS host query. Gameplay AI
    /// and diagnostics now consume the same live Script geometry path.
    pub(super) fn live_dynamic_script_los_state(
        &self,
        ctx: &egui::Context,
        map: &ProcessedMap,
        start: Vec3,
        end: Vec3,
    ) -> Option<(bool, usize, usize)> {
        let time = ctx.input(|input| input.time);
        self.runtime_dynamic_script_line_of_sight_state(map, start, end, time)
    }
}
