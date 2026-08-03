pub fn draw_navmesh_controls(ui: &mut eframe::egui::Ui) {
    ui.label("Robots v248 NavMesh 0x607 is always decoded once; Maps controls visibility and texture scale dynamically without reloading.");

    draw_instance_controls(ui);

    let stats = eurochef_edb::entity::robots_navmesh_stats();
    ui.small(format!(
        "NavMesh processed this session: {} objects / {} verts / {} faces / {} groups",
        stats.objects, stats.vertices, stats.faces, stats.groups
    ));
    draw_entity_breakdown(ui);
    ui.small(format!(
        "Robots HashDB: {} symbols; 0x0100002c = {}",
        eurochef_edb::robots_hashdb::ROBOTS_HASHDB_ENTRY_COUNT,
        eurochef_edb::robots_hashdb::format_or_invalid(0x0100002c)
    ));
}
pub fn draw_entity_breakdown(ui: &mut eframe::egui::Ui) {
    let counts = eurochef_edb::entity::robots_entity_type_stats();
    ui.label("Robots v248 entity types decoded since reset:");
    ui.monospace(format!(
        "Mesh 0x601: {}   Split 0x603: {}   Instance 0x606: {}   NavMesh 0x607: {}   MapZone 0x608: {}   Unknown: {}",
        counts.mesh, counts.split, counts.instance, counts.navmesh, counts.mapzone, counts.unknown
    ));
    if ui.small_button("Reset Robots parser counters").clicked() {
        eurochef_edb::entity::reset_robots_entity_type_stats();
    }
}
pub fn draw_instance_controls(ui: &mut eframe::egui::Ui) {
    ui.small("Decoded v248 0x606 schema: selector @ +0x58 -> page = value >> 6, slot = value & 0x3F, record stride = 0x38. Selector is not a proven direct model/mesh/texture reference.");
    ui.small("Decoded 0x606 render path: +0x54 primitive count; +0x58 indexes the current EDB texture list; +0x60 is an inline vertex stream (0x24-byte stride) rendered as a triangle strip. EuroChef now renders this serialized geometry directly.");
    let mut visible = eurochef_edb::entity::robots_instance_bounds_visible();
    if ui
        .checkbox(&mut visible, "Instance bounds (Robots v248 / 0x606)")
        .on_hover_text("Diagnostic serialized EXGeoBaseEntity bounds only. This does not claim the remaining 0x606 transform/reference payload is decoded. Reopen/reload the entity after changing this because the legacy renderer caches geometry.")
        .changed()
    {
        eurochef_edb::entity::set_robots_instance_bounds_visible(visible);
    }
}
