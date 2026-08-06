use super::*;

impl EurochefApp {
    pub(super) fn load_into_render_store(
        &mut self,
        references: &[Hashcode],
        file_map: &mut IntMap<Hashcode, EdbFile>,
        file_ref: Hashcode,
        platform: Platform,
    ) -> anyhow::Result<()> {
        let edb = &mut (if let Some(path) = self.path_cache.get(&file_ref) {
            match file_map.entry(file_ref) {
                hash_map::Entry::Occupied(e) => e.into_mut(),
                hash_map::Entry::Vacant(a) => {
                    let file = File::open(path)?;
                    let reader = BufReader::new(file);

                    a.insert(EdbFile::new(Box::new(reader), platform)?)
                }
            }
        } else {
            warn!(
                "External EDB file hash 0x{:08X} is not present in path_cache; requested objects: {:?}",
                file_ref,
                references
            );
            println!(
                "[Robots] unresolved external file 0x{:08X}; objects={:?}; path_cache={}",
                file_ref,
                references,
                self.path_cache.len()
            );
            return Ok(());
        });

        let header = edb.header.clone();

        let mut rs_lock = self.render_store.write();

        // ROBOTS_PATCH_0024_SCRIPT_LOCAL_CLOSURE
        // Explicit script references seed the closure directly. External entity
        // references also need assembly scripts: map vehicle triggers request the
        // body entity, while the complete body + wheel composition lives in an
        // AnimScript in the same external EDB.
        let mut script_refs: Vec<Hashcode> = references
            .iter()
            .copied()
            .filter(|hashcode| hashcode.base() == 0x04000000)
            .collect();
        let requested_entities: Vec<Hashcode> = references
            .iter()
            .copied()
            .filter(|hashcode| hashcode.base() == 0x02000000)
            .collect();

        if !requested_entities.is_empty() {
            // read_all() records references for every script. Restore the original
            // reference sets afterwards, then re-read only selected assembly scripts
            // so unrelated scripts do not pull their whole dependency graph into the map.
            let saved_internal_references = edb.internal_references.clone();
            let saved_external_references = edb.external_references.clone();
            let script_catalog = UXGeoScript::read_all(edb)?;
            edb.internal_references = saved_internal_references;
            edb.external_references = saved_external_references;

            for script in script_catalog {
                let mut entity_command_count = 0usize;
                let mut contains_requested_body = false;

                for command in &script.commands {
                    if let UXGeoScriptCommandData::Entity { hashcode, .. } = &command.data {
                        entity_command_count += 1;
                        let resolved_hashcode = if hashcode.is_local() {
                            header
                                .entity_list
                                .data()
                                .get(hashcode.index() as usize)
                                .map(|entity| entity.common.hashcode)
                        } else {
                            Some(*hashcode)
                        };

                        contains_requested_body |= resolved_hashcode
                            .map(|entity| requested_entities.contains(&entity))
                            .unwrap_or(false);
                    }
                }

                if contains_requested_body
                    && entity_command_count > 1
                    && !script_refs.contains(&script.hashcode)
                {
                    println!(
                        "[Robots] external assembly selected: file=0x{:08X} script=0x{:08X} requested_entities={:?} entity_commands={}",
                        header.hashcode,
                        script.hashcode,
                        requested_entities,
                        entity_command_count
                    );
                    script_refs.push(script.hashcode);
                }
            }
        }

        let mut seen_script_refs: Vec<Hashcode> = vec![];
        let mut scripts: Vec<UXGeoScript> = vec![];

        loop {
            let batch: Vec<Hashcode> = script_refs
                .iter()
                .copied()
                .filter(|hashcode| !seen_script_refs.contains(hashcode))
                .collect();

            if batch.is_empty() {
                break;
            }

            seen_script_refs.extend(batch.iter().copied());
            let batch_scripts = UXGeoScript::read_hashcodes(edb, &batch)?;

            for script in &batch_scripts {
                for command in &script.commands {
                    if let UXGeoScriptCommandData::SubScript { hashcode, file } = &command.data {
                        if hashcode.base() == 0x04000000
                            && (hashcode.is_local() || *file == header.hashcode)
                            && !script_refs.contains(hashcode)
                        {
                            script_refs.push(*hashcode);
                        }
                    }
                }
            }

            scripts.extend(batch_scripts);
        }

        for s in &scripts {
            rs_lock.insert_script(header.hashcode, s.clone());
        }
        if let Some(sound_preview) = &self.sound_preview {
            preload_script_sounds(sound_preview, &scripts);
        }

        // Particle resources are parsed before entities because the proven tail array at
        // EXGeoParticle+0x100/+0x104 can contribute local entity references.
        let particles = EXGeoParticle::read_all(edb)?;
        for particle in particles {
            rs_lock.insert_particle(header.hashcode, particle);
        }

        // Entities should come after scripts, since we need all references to resolve first
        // Also include the requested references
        let interal_references_filtered: Vec<Hashcode> = edb
            .internal_references
            .iter()
            .filter(|v| !rs_lock.is_object_loaded(header.hashcode, **v))
            .copied()
            .collect();
        let mut internal_refs = [references, &interal_references_filtered].concat();
        let animation_catalog = animations::read_from_file(edb)?;
        // Animation commands commonly serialize skin=0xFFFFFFFF, meaning use the
        // AnimSkin bound by the Animation asset. Promote that bound skin into the
        // selected entity closure so static Maps previews load its component meshes.
        for reference in internal_refs.clone() {
            if let Some(skin_hashcode) = animation_catalog.bound_skin_hashcode(reference) {
                if !internal_refs.contains(&skin_hashcode) {
                    internal_refs.push(skin_hashcode);
                }
            }
        }
        let (entities, skins, _) = entities::read_from_file(edb, Some(&internal_refs))?;
        if !animation_catalog.clips.is_empty() {
            rs_lock.insert_animation_runtime(
                header.hashcode,
                Arc::new(animations::AnimationRuntime::new(
                    header.hashcode,
                    &self.gl,
                    edb.platform,
                    animation_catalog,
                    &entities,
                )),
            );
        }
        for (i, e) in entities.into_iter() {
            let mut r = EntityRenderer::new(header.hashcode, edb.platform);
            if let Ok((_, m)) = &e.data {
                unsafe {
                    r.load_mesh(&self.gl, m);
                }
            }
            rs_lock.insert_entity(header.hashcode, e.hashcode, i, r);
        }

        // ROBOTS_PATCH_0024_REGISTER_ANIMSKIN
        for skin_result in &skins {
            if let Ok(skin) = &skin_result.data {
                let skin_index = header
                    .animskin_list
                    .iter()
                    .position(|header_skin| header_skin.common.hashcode == skin_result.hashcode)
                    .unwrap_or(0);

                let mut entity_hashcodes: Vec<Hashcode> = vec![];
                for entry in skin.entities.iter().chain(skin.more_entities.iter()) {
                    let entity_index = (entry.entity_index & 0x00ff_ffff) as usize;
                    if let Some(entity_header) = header.entity_list.data().get(entity_index) {
                        let hashcode = entity_header.common.hashcode;
                        if !entity_hashcodes.contains(&hashcode) {
                            entity_hashcodes.push(hashcode);
                        }
                    }
                }

                rs_lock.insert_animskin(
                    header.hashcode,
                    skin_result.hashcode,
                    skin_index,
                    entity_hashcodes,
                );
            }
        }

        // Textures should come last, since textures refer to nothing (aside from a few external references)
        let internal_refs = edb.internal_references.clone();
        let textures = UXGeoTexture::read_hashcodes(edb, &internal_refs);
        for (i, t) in entities::EntityListPanel::load_textures(&self.gl, &textures) {
            rs_lock.insert_texture(header.hashcode, t.hashcode, i, t);
        }

        drop(rs_lock);
        let external_references = edb.external_references.clone();
        self.resolve_references(platform, &external_references, file_map)?;

        Ok(())
    }

    pub(super) fn resolve_references(
        &mut self,
        platform: Platform,
        references: &[(Hashcode, Hashcode)],
        file_map: &mut IntMap<Hashcode, EdbFile>,
    ) -> anyhow::Result<()> {
        let mut grouped_refs: Vec<(Hashcode, Vec<Hashcode>)> = vec![];
        for (rf, ro) in references {
            let group = if let Some(f) = grouped_refs.iter_mut().find(|(f, _)| f == rf) {
                f
            } else {
                grouped_refs.push((*rf, vec![]));
                grouped_refs.last_mut().unwrap()
            };

            if !group.1.contains(ro) && !self.render_store.read().is_object_loaded(*rf, *ro) {
                group.1.push(*ro)
            }
        }

        grouped_refs.retain(|v| !v.1.is_empty());

        for (ref_file, ref_objects) in grouped_refs {
            self.load_into_render_store(&ref_objects, file_map, ref_file, platform)?;
        }

        Ok(())
    }

    pub(super) fn load_file<R: Read + Seek + 'static>(
        &mut self,
        platform: Platform,
        reader: Box<R>,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        if platform == Platform::Ps2 {
            self.ps2_warning = true;
        }

        self.render_store.write().purge(true);
        let mut edb = EdbFile::new(reader, platform)?;
        let header = edb.header.clone();
        let sound_preview =
            crate::sound_preview::shared_sound_preview(self.current_source_path.as_deref());
        self.sound_preview = Some(sound_preview.clone());

        self.current_panel = if std::env::var("EUROCHEF_START_PANEL")
            .is_ok_and(|panel| panel.trim().eq_ignore_ascii_case("maps"))
        {
            Panel::Maps
        } else {
            Panel::FileInfo
        };
        self.spreadsheetlist = None;
        self.fileinfo = None;
        self.textures = None;
        self.maps = None;
        self.scripts = None;
        self.animations = None;

        self.fileinfo = Some(fileinfo::FileInfoPanel::new(edb.header.clone()));

        let spreadsheets = UXGeoSpreadsheet::read_all(&mut edb)?;
        if !spreadsheets.is_empty() {
            self.spreadsheetlist = Some(spreadsheet::TextItemList::new(spreadsheets.clone()));
        }

        if [
            Platform::Xbox,
            Platform::Xbox360,
            Platform::Pc,
            Platform::Ps2,
            Platform::GameCube,
            Platform::Wii,
        ]
        .contains(&platform)
        {
            let particles = EXGeoParticle::read_all(&mut edb)?;
            let animation_catalog = animations::read_from_file(&mut edb)?;
            let (entities, skins, ref_entities) = entities::read_from_file(&mut edb, None)?;

            for (i, e) in entities.iter() {
                if e.hashcode.is_local() {
                    debug_assert_eq!(e.hashcode.index(), *i as u32);
                }
            }

            let scripts = UXGeoScript::read_all(&mut edb)?;
            preload_script_sounds(&sound_preview, &scripts);
            let has_animations = !animation_catalog.clips.is_empty();
            {
                let mut rs_lock = self.render_store.write();
                for s in &scripts {
                    rs_lock.insert_script(header.hashcode, s.clone());
                }
                for particle in particles {
                    rs_lock.insert_particle(header.hashcode, particle);
                }

                if has_animations {
                    rs_lock.insert_animation_runtime(
                        header.hashcode,
                        Arc::new(animations::AnimationRuntime::new(
                            header.hashcode,
                            &self.gl,
                            platform,
                            animation_catalog.clone(),
                            &entities,
                        )),
                    );
                }
            }

            let mut rs_lock = self.render_store.write();
            for (i, e) in entities.iter() {
                let mut r = EntityRenderer::new(header.hashcode, platform);
                if let Ok((_, m)) = &e.data {
                    unsafe {
                        r.load_mesh(&self.gl, m);
                    }
                }
                rs_lock.insert_entity(header.hashcode, e.hashcode, *i, r);
            }

            // ROBOTS_PATCH_0024_REGISTER_CURRENT_ANIMSKIN
            for skin_result in &skins {
                if let Ok(skin) = &skin_result.data {
                    let skin_index = header
                        .animskin_list
                        .iter()
                        .position(|header_skin| header_skin.common.hashcode == skin_result.hashcode)
                        .unwrap_or(0);

                    let mut entity_hashcodes: Vec<Hashcode> = vec![];
                    for entry in skin.entities.iter().chain(skin.more_entities.iter()) {
                        let entity_index = (entry.entity_index & 0x00ff_ffff) as usize;
                        if let Some(entity_header) = header.entity_list.data().get(entity_index) {
                            let hashcode = entity_header.common.hashcode;
                            if !entity_hashcodes.contains(&hashcode) {
                                entity_hashcodes.push(hashcode);
                            }
                        }
                    }

                    rs_lock.insert_animskin(
                        header.hashcode,
                        skin_result.hashcode,
                        skin_index,
                        entity_hashcodes,
                    );
                }
            }
            drop(rs_lock);

            if has_animations {
                self.animations = Some(animations::AnimationListPanel::new(
                    header.hashcode,
                    &self.gl,
                    animation_catalog,
                    &entities,
                    &scripts,
                    platform,
                    self.render_store.clone(),
                    self.hashcodes.clone(),
                ));
            }

            if !scripts.is_empty() {
                self.scripts = Some(scripts::ScriptListPanel::new(
                    header.hashcode,
                    &self.gl,
                    scripts,
                    self.render_store.clone(),
                    self.hashcodes.clone(),
                    sound_preview.clone(),
                ));
            }

            if entities.len() + skins.len() + ref_entities.len() > 0 {
                if self.fileinfo.as_ref().unwrap().header.map_list.len() > 0 {
                    let mut map = maps::read_from_file(&mut edb);
                    if self.game.eq_ignore_ascii_case("robots") {
                        let resolved_characters = maps::resolve_robots_character_visuals(
                            &mut edb,
                            &mut map,
                            &self.path_cache,
                            platform,
                        )?;
                        info!(
                            "Resolved {} runtime-created Monster/NPC character visuals",
                            resolved_characters
                        );
                    }
                    sound_preview.lock().preload_hashes(
                        map.iter()
                            .flat_map(|map| map.sounds.iter().map(|sound| sound.sound_ref)),
                    );

                    self.maps = Some(maps::MapViewerPanel::new(
                        header.hashcode,
                        self.gl.clone(),
                        map,
                        ref_entities.clone(),
                        self.render_store.clone(),
                        platform,
                        self.hashcodes.clone(),
                        &self.game,
                        sound_preview.clone(),
                    ));
                }

                self.entities = Some(entities::EntityListPanel::new(
                    header.hashcode,
                    self.render_store.clone(),
                    ctx,
                    self.gl.clone(),
                    entities.into_iter().map(|(_, ires)| ires).collect(),
                    skins,
                    ref_entities,
                    self.hashcodes.clone(),
                    platform,
                ));
            }
        } else {
            self.entities = None;
        }

        let textures = UXGeoTexture::read_all(&mut edb);
        {
            let mut rs_lock = self.render_store.write();
            for (i, t) in entities::EntityListPanel::load_textures(&self.gl, &textures).into_iter()
            {
                rs_lock.insert_texture(header.hashcode, t.hashcode, i, t);
            }
        }

        if textures.len() == 1 && textures[0].1.hashcode == 0x06000000 {
            self.textures = None;
        } else {
            self.textures = Some(textures::TextureList::new(
                ctx,
                textures.into_iter().map(|(_, t)| t).collect(),
                self.hashcodes.clone(),
            ));
        }

        edb.external_references.sort_by(|(a, _), (b, _)| a.cmp(b));
        self.fileinfo.as_mut().unwrap().external_references = edb.external_references.clone();

        let start = Instant::now();
        let mut file_map: IntMap<Hashcode, EdbFile> = Default::default();
        self.resolve_references(platform, &edb.external_references, &mut file_map)?;
        info!(
            "Resolving references took {}s",
            start.elapsed().as_secs_f32()
        );

        self.state = AppState::Ready;

        Ok(())
    }
}

fn preload_script_sounds(
    sound_preview: &crate::sound_preview::SharedSoundPreview,
    scripts: &[UXGeoScript],
) {
    sound_preview
        .lock()
        .preload_hashes(scripts.iter().flat_map(|script| {
            script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Sound { hashcode } => Some(*hashcode),
                    _ => None,
                })
        }));
}
