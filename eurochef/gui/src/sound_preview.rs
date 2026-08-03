use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    fs::File,
    hash::{Hash, Hasher},
    io::BufReader,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex as StdMutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use egui::mutex::Mutex;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source, SpatialPlayer};
use serde_json::Value;

pub type SharedSoundPreview = Arc<Mutex<SoundPreview>>;

const LISTENER_LEFT_EAR: [f32; 3] = [-0.35, 0.0, 0.0];
const LISTENER_RIGHT_EAR: [f32; 3] = [0.35, 0.0, 0.0];
const SILENCE_EPSILON: f32 = 0.0001;
const DECODE_WORKER_COUNT: usize = 4;

pub fn shared_sound_preview(source_path: Option<&Path>) -> SharedSoundPreview {
    Arc::new(Mutex::new(SoundPreview::new(source_path)))
}

pub fn is_playable_audio_reference(hashcode: u32) -> bool {
    hashcode != 0 && hashcode != u32::MAX && (hashcode & 0xFF00_0000) != 0x1C00_0000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundVoiceGroup {
    Manual,
    MapAmbient,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundVoiceKey {
    Manual,
    MapAmbient {
        map_hashcode: u32,
        sound_index: usize,
    },
    Script {
        file: u32,
        script: u32,
        command_path: u64,
    },
}

impl SoundVoiceKey {
    fn group(self) -> SoundVoiceGroup {
        match self {
            Self::Manual => SoundVoiceGroup::Manual,
            Self::MapAmbient { .. } => SoundVoiceGroup::MapAmbient,
            Self::Script { .. } => SoundVoiceGroup::Script,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SoundVoiceSpec {
    pub hashcode: u32,
    pub looping: bool,
    pub volume: f32,
    pub speed: f32,
    pub pan: f32,
    pub fade_in_seconds: f32,
    pub fade_out_seconds: f32,
    pub seek_seconds: f32,
}

impl SoundVoiceSpec {
    pub fn one_shot(hashcode: u32) -> Self {
        Self {
            hashcode,
            looping: false,
            volume: 1.0,
            speed: 1.0,
            pan: 0.0,
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.05,
            seek_seconds: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DecodedSoundKey {
    project_fingerprint: u64,
    hashcode: u32,
    pool_index: usize,
}

#[derive(Debug)]
struct DecodeRequest {
    key: DecodedSoundKey,
    project_root: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug)]
struct DecodeResult {
    key: DecodedSoundKey,
    output_path: PathBuf,
    metadata: Option<Value>,
    result: Result<(), String>,
}

enum DecodeMessage {
    Decode(DecodeRequest),
    Shutdown,
}

struct DecodeWorker {
    sender: Sender<DecodeMessage>,
    receiver: Receiver<DecodeResult>,
    handles: Vec<JoinHandle<()>>,
}

impl DecodeWorker {
    fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<DecodeMessage>();
        let (result_tx, result_rx) = mpsc::channel::<DecodeResult>();
        let request_rx = Arc::new(StdMutex::new(request_rx));
        let catalogs = Arc::new(StdMutex::new(HashMap::new()));
        let mut handles = Vec::with_capacity(DECODE_WORKER_COUNT);
        for worker_index in 0..DECODE_WORKER_COUNT {
            let request_rx = request_rx.clone();
            let result_tx = result_tx.clone();
            let catalogs = catalogs.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("eurochef-eurosound-decode-{worker_index}"))
                    .spawn(move || loop {
                        let message = match request_rx.lock().expect("decode queue poisoned").recv()
                        {
                            Ok(message) => message,
                            Err(_) => break,
                        };
                        match message {
                            DecodeMessage::Decode(request) => {
                                let (metadata, result) = decode_native(&request, &catalogs);
                                let _ = result_tx.send(DecodeResult {
                                    key: request.key,
                                    output_path: request.output_path,
                                    metadata,
                                    result,
                                });
                            }
                            DecodeMessage::Shutdown => break,
                        }
                    })
                    .expect("failed to start EuroSound decode worker"),
            );
        }

        Self {
            sender: request_tx,
            receiver: result_rx,
            handles,
        }
    }

    fn shutdown(&mut self) {
        if self.handles.is_empty() {
            return;
        }
        for _ in 0..self.handles.len() {
            let _ = self.sender.send(DecodeMessage::Shutdown);
        }
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for DecodeWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ActiveVoice {
    player: SpatialPlayer,
    hashcode: u32,
    looping: bool,
    base_volume: f32,
    current_volume: f32,
    target_volume: f32,
    transition_seconds: f32,
    fade_out_seconds: f32,
    removing: bool,
}

pub struct SoundPreview {
    pub project_root: String,
    pub pool_index: usize,
    pub master_volume: f32,
    pub manual_volume: f32,
    pub ambient_volume: f32,
    pub script_volume: f32,
    pub ambient_enabled: bool,
    pub script_enabled: bool,
    pub max_ambient_voices: usize,

    status: String,
    metadata: Option<Value>,
    audio_sink: Option<MixerDeviceSink>,
    audio_error: Option<String>,
    voices: HashMap<SoundVoiceKey, ActiveVoice>,
    pending_voices: HashMap<SoundVoiceKey, SoundVoiceSpec>,
    completed_one_shots: HashSet<SoundVoiceKey>,
    decoded_paths: HashMap<DecodedSoundKey, PathBuf>,
    sound_metadata: HashMap<DecodedSoundKey, Value>,
    failed_decodes: HashMap<DecodedSoundKey, String>,
    pending_decodes: HashSet<DecodedSoundKey>,
    decode_worker: DecodeWorker,
    last_tick: Instant,
}

impl SoundPreview {
    pub fn new(source_path: Option<&Path>) -> Self {
        Self {
            project_root: discover_project_root(source_path)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            pool_index: 0,
            master_volume: 0.8,
            manual_volume: 1.0,
            ambient_volume: 0.65,
            script_volume: 1.0,
            ambient_enabled: true,
            script_enabled: true,
            max_ambient_voices: 8,
            status: "Ready".to_string(),
            metadata: None,
            audio_sink: None,
            audio_error: None,
            voices: HashMap::new(),
            pending_voices: HashMap::new(),
            completed_one_shots: HashSet::new(),
            decoded_paths: HashMap::new(),
            sound_metadata: HashMap::new(),
            failed_decodes: HashMap::new(),
            pending_decodes: HashSet::new(),
            decode_worker: DecodeWorker::new(),
            last_tick: Instant::now(),
        }
    }

    pub fn draw_settings(&mut self, ui: &mut egui::Ui) {
        self.tick();
        let mut invalidate_cache = false;
        egui::Grid::new("euro_sound_preview_settings")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Sound root");
                invalidate_cache |= ui.text_edit_singleline(&mut self.project_root).changed();
                if ui.button("Browse…").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.project_root = folder.to_string_lossy().to_string();
                        invalidate_cache = true;
                    }
                }
                ui.end_row();

                ui.label("Sample pool");
                invalidate_cache |= ui
                    .add(egui::DragValue::new(&mut self.pool_index).range(0..=255))
                    .changed();
                ui.label("SoundBank sample-pool entry");
                ui.end_row();

                ui.label("Master volume");
                ui.add(egui::Slider::new(&mut self.master_volume, 0.0..=1.5));
                ui.label("All EuroSound voices");
                ui.end_row();

                ui.label("Manual preview");
                ui.add(egui::Slider::new(&mut self.manual_volume, 0.0..=1.5));
                ui.label("Inspector Play button");
                ui.end_row();

                ui.checkbox(&mut self.ambient_enabled, "Map ambient audio");
                ui.add(egui::Slider::new(&mut self.ambient_volume, 0.0..=1.5));
                ui.add(
                    egui::DragValue::new(&mut self.max_ambient_voices)
                        .range(1..=32)
                        .prefix("voices "),
                );
                ui.end_row();

                ui.checkbox(&mut self.script_enabled, "Script timeline audio");
                ui.add(egui::Slider::new(&mut self.script_volume, 0.0..=1.5));
                ui.label("Sound commands follow play/pause/seek/loop");
                ui.end_row();
            });

        if !self.failed_decodes.is_empty() && ui.button("Retry unavailable/failed audio").clicked()
        {
            self.failed_decodes.clear();
            self.status = "Unavailable/failed audio cache cleared".to_string();
        }
        if invalidate_cache {
            self.invalidate_decode_cache("EuroSound paths or sample pool changed");
        }
        if !self.ambient_enabled {
            self.stop_group(SoundVoiceGroup::MapAmbient, 0.1);
        }
        if !self.script_enabled {
            self.stop_group(SoundVoiceGroup::Script, 0.05);
        }

        ui.small(format!(
            "{} | active {} (ambient {}, script {}) | decoding {} | unavailable/failed {}",
            self.status,
            self.voices.len(),
            self.voice_count(SoundVoiceGroup::MapAmbient),
            self.voice_count(SoundVoiceGroup::Script),
            self.pending_decodes.len(),
            self.failed_decodes.len(),
        ));
        if let Some(error) = &self.audio_error {
            ui.colored_label(egui::Color32::YELLOW, error);
        }
        if let Some(metadata) = &self.metadata {
            ui.monospace(format_probe_summary(metadata));
        }
        ui.small("Map fade bytes are previewed as fixed-60-Hz ticks. Unknown EXGeoSound flag/base-map semantics are retained diagnostically and are not invented.");
    }

    pub fn draw_actions(&mut self, ui: &mut egui::Ui, hashcode: u32) {
        ui.horizontal(|ui| {
            if ui.button("Queue decode").clicked() {
                self.preload_hashes([hashcode]);
            }
            if ui.button("Play").clicked() {
                self.play_manual(hashcode);
            }
            if ui.button("Stop").clicked() {
                self.stop_voice(SoundVoiceKey::Manual, 0.03);
            }
            if ui.button("Export WAV…").clicked() {
                let default_name = format!("{hashcode:08x}_pool{}.wav", self.pool_index);
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&default_name)
                    .add_filter("Wave audio", &["wav"])
                    .save_file()
                {
                    let key = self.decoded_key(hashcode);
                    match self.decoded_paths.get(&key) {
                        Some(decoded) => match std::fs::copy(decoded, &path) {
                            Ok(_) => self.status = format!("Exported {}", path.display()),
                            Err(error) => self.status = error.to_string(),
                        },
                        None => {
                            self.preload_hashes([hashcode]);
                            self.status =
                                "Decode queued; export again when it is ready".to_string();
                        }
                    }
                }
            }
        });
    }

    pub fn play_manual(&mut self, hashcode: u32) {
        if !is_playable_audio_reference(hashcode) {
            self.status =
                format!("0x{hashcode:08x} is a reverb preset reference, not playable sample data");
            return;
        }
        let spec = SoundVoiceSpec::one_shot(hashcode);
        self.reset_voice(SoundVoiceKey::Manual);
        self.request_voice(SoundVoiceKey::Manual, spec);
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_tick).as_secs_f32().min(0.25);
        self.last_tick = now;
        self.poll_decode_results();
        self.start_ready_pending_voices();
        self.update_voices(delta);
    }

    pub fn preload_hashes<I>(&mut self, hashcodes: I)
    where
        I: IntoIterator<Item = u32>,
    {
        self.tick();
        for hashcode in hashcodes {
            if !is_playable_audio_reference(hashcode) {
                continue;
            }
            let key = self.decoded_key(hashcode);
            self.request_decode(key);
        }
    }

    pub fn request_voice(&mut self, key: SoundVoiceKey, mut spec: SoundVoiceSpec) {
        self.tick();
        if !is_playable_audio_reference(spec.hashcode) {
            self.pending_voices.remove(&key);
            self.stop_voice(key, 0.0);
            return;
        }
        spec.volume = spec.volume.max(0.0);
        spec.speed = spec.speed.max(0.05);
        spec.pan = spec.pan.clamp(-1.0, 1.0);
        spec.seek_seconds = spec.seek_seconds.max(0.0);

        if !self.group_enabled(key.group()) {
            return;
        }
        if !spec.looping && self.completed_one_shots.contains(&key) {
            return;
        }

        let effective_volume = self.effective_volume(key.group(), spec.volume);
        if let Some(voice) = self.voices.get_mut(&key) {
            voice.base_volume = spec.volume;
            voice.target_volume = effective_volume;
            voice.transition_seconds = 0.08;
            voice.fade_out_seconds = spec.fade_out_seconds.max(0.0);
            voice.removing = false;
            voice.player.set_speed(spec.speed);
            voice.player.set_emitter_position(pan_position(spec.pan));
            if voice.hashcode != spec.hashcode || voice.looping != spec.looping {
                self.reset_voice(key);
            } else {
                return;
            }
        }

        let decoded_key = self.decoded_key(spec.hashcode);
        if let Some(error) = self.failed_decodes.get(&decoded_key) {
            self.pending_voices.remove(&key);
            self.status = error.clone();
            return;
        }
        if self.decoded_paths.contains_key(&decoded_key) {
            if let Err(error) = self.start_voice(key, spec, &decoded_key) {
                self.status = error;
            }
        } else {
            self.pending_voices.insert(key, spec);
            self.request_decode(decoded_key);
        }
    }

    pub fn sync_group<I>(&mut self, group: SoundVoiceGroup, desired: I, default_fade_out: f32)
    where
        I: IntoIterator<Item = (SoundVoiceKey, SoundVoiceSpec)>,
    {
        self.tick();
        let desired = desired.into_iter().collect::<HashMap<_, _>>();
        let desired_keys = desired.keys().copied().collect::<HashSet<_>>();

        let stale = self
            .voices
            .keys()
            .copied()
            .filter(|key| key.group() == group && !desired_keys.contains(key))
            .collect::<Vec<_>>();
        for key in stale {
            let fade = self
                .voices
                .get(&key)
                .map(|voice| voice.fade_out_seconds)
                .unwrap_or(default_fade_out)
                .max(default_fade_out);
            self.stop_voice(key, fade);
        }

        self.pending_voices
            .retain(|key, _| key.group() != group || desired_keys.contains(key));
        self.completed_one_shots
            .retain(|key| key.group() != group || desired_keys.contains(key));

        for (key, spec) in desired {
            self.request_voice(key, spec);
        }
    }

    pub fn stop_voice(&mut self, key: SoundVoiceKey, fade_seconds: f32) {
        self.pending_voices.remove(&key);
        if let Some(voice) = self.voices.get_mut(&key) {
            if fade_seconds <= 0.0 {
                voice.player.stop();
                self.voices.remove(&key);
            } else {
                voice.target_volume = 0.0;
                voice.transition_seconds = fade_seconds;
                voice.removing = true;
            }
        }
    }

    pub fn stop_group(&mut self, group: SoundVoiceGroup, fade_seconds: f32) {
        self.pending_voices.retain(|key, _| key.group() != group);
        self.completed_one_shots.retain(|key| key.group() != group);
        let keys = self
            .voices
            .keys()
            .copied()
            .filter(|key| key.group() == group)
            .collect::<Vec<_>>();
        for key in keys {
            self.stop_voice(key, fade_seconds);
        }
    }

    pub fn reset_group(&mut self, group: SoundVoiceGroup) {
        self.pending_voices.retain(|key, _| key.group() != group);
        self.completed_one_shots.retain(|key| key.group() != group);
        let keys = self
            .voices
            .keys()
            .copied()
            .filter(|key| key.group() == group)
            .collect::<Vec<_>>();
        for key in keys {
            self.reset_voice(key);
        }
    }

    pub fn pause_group(&mut self, group: SoundVoiceGroup) {
        for (key, voice) in &self.voices {
            if key.group() == group {
                voice.player.pause();
            }
        }
    }

    pub fn resume_group(&mut self, group: SoundVoiceGroup) {
        for (key, voice) in &self.voices {
            if key.group() == group {
                voice.player.play();
            }
        }
    }

    pub fn voice_count(&self, group: SoundVoiceGroup) -> usize {
        self.voices
            .keys()
            .filter(|key| key.group() == group)
            .count()
    }

    pub fn has_pending_work(&self) -> bool {
        !self.pending_decodes.is_empty()
            || !self.pending_voices.is_empty()
            || self.voices.values().any(|voice| {
                voice.removing || (voice.current_volume - voice.target_volume).abs() > 0.001
            })
    }

    fn group_enabled(&self, group: SoundVoiceGroup) -> bool {
        match group {
            SoundVoiceGroup::Manual => true,
            SoundVoiceGroup::MapAmbient => self.ambient_enabled,
            SoundVoiceGroup::Script => self.script_enabled,
        }
    }

    fn effective_volume(&self, group: SoundVoiceGroup, base_volume: f32) -> f32 {
        let group_volume = match group {
            SoundVoiceGroup::Manual => self.manual_volume,
            SoundVoiceGroup::MapAmbient => self.ambient_volume,
            SoundVoiceGroup::Script => self.script_volume,
        };
        (base_volume * group_volume * self.master_volume).max(0.0)
    }

    fn decoded_key(&self, hashcode: u32) -> DecodedSoundKey {
        DecodedSoundKey {
            project_fingerprint: project_fingerprint(&format!(
                "native-pc-musx-v2\0{}",
                self.project_root
            )),
            hashcode,
            pool_index: self.pool_index,
        }
    }

    fn decoded_path(&self, key: &DecodedSoundKey) -> PathBuf {
        decoded_cache_path(key)
    }

    fn request_decode(&mut self, key: DecodedSoundKey) {
        if self.decoded_paths.contains_key(&key)
            || self.failed_decodes.contains_key(&key)
            || self.pending_decodes.contains(&key)
        {
            return;
        }

        let project_root = PathBuf::from(self.project_root.trim());
        if !project_root.exists() {
            self.status = format!("Sound root not found: {}", project_root.display());
            return;
        }

        let output_path = self.decoded_path(&key);
        if let Some(parent) = output_path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                self.status = format!("Could not create EuroSound cache: {error}");
                return;
            }
        }
        if output_path.is_file() {
            self.decoded_paths.insert(key, output_path);
            return;
        }

        let request = DecodeRequest {
            key: key.clone(),
            project_root,
            output_path,
        };
        match self
            .decode_worker
            .sender
            .send(DecodeMessage::Decode(request))
        {
            Ok(()) => {
                self.pending_decodes.insert(key.clone());
                self.status = format!("Decoding 0x{:08x}, pool {}…", key.hashcode, key.pool_index);
            }
            Err(error) => self.status = format!("EuroSound decode worker stopped: {error}"),
        }
    }

    fn poll_decode_results(&mut self) {
        while let Ok(result) = self.decode_worker.receiver.try_recv() {
            self.pending_decodes.remove(&result.key);
            if self.decoded_key(result.key.hashcode) != result.key {
                let _ = std::fs::remove_file(&result.output_path);
                continue;
            }
            if let Some(metadata) = result.metadata {
                self.sound_metadata.insert(result.key.clone(), metadata);
            }
            match result.result {
                Ok(()) => {
                    self.status = format!(
                        "Decoded 0x{:08x}, pool {}",
                        result.key.hashcode, result.key.pool_index
                    );
                    self.failed_decodes.remove(&result.key);
                    self.decoded_paths.insert(result.key, result.output_path);
                }
                Err(error) => {
                    let _ = std::fs::remove_file(&result.output_path);
                    self.pending_voices
                        .retain(|_, spec| spec.hashcode != result.key.hashcode);
                    self.failed_decodes.insert(result.key, error.clone());
                    self.status = error;
                }
            }
        }
    }

    fn metadata_loop_info(
        &self,
        decoded_key: &DecodedSoundKey,
    ) -> Option<(bool, f32, Option<f32>)> {
        const MUSX_OLD_FLAG_LOOP: u64 = 1 << 6;
        let metadata = self.sound_metadata.get(decoded_key)?;
        let looping = metadata
            .get("looping")
            .and_then(Value::as_bool)
            .or_else(|| {
                metadata
                    .get("sound")
                    .and_then(|sound| sound.get("flags"))
                    .and_then(Value::as_u64)
                    .map(|flags| flags & MUSX_OLD_FLAG_LOOP != 0)
            })?;
        let frequency = metadata
            .get("frequency")
            .and_then(Value::as_f64)
            .filter(|frequency| *frequency > 0.0)
            .unwrap_or(0.0) as f32;
        if !looping || frequency <= f32::EPSILON {
            return Some((looping, 0.0, None));
        }
        let start = metadata
            .get("loop_start_sample")
            .and_then(Value::as_u64)
            .unwrap_or(0) as f32
            / frequency;
        let end = metadata
            .get("loop_end_sample")
            .and_then(Value::as_u64)
            .filter(|end| *end > 0)
            .map(|end| end as f32 / frequency)
            .filter(|end| *end > start);
        Some((looping, start, end))
    }

    fn start_ready_pending_voices(&mut self) {
        let ready = self
            .pending_voices
            .iter()
            .filter_map(|(key, spec)| {
                let decoded_key = self.decoded_key(spec.hashcode);
                self.decoded_paths
                    .contains_key(&decoded_key)
                    .then_some((*key, *spec, decoded_key))
            })
            .collect::<Vec<_>>();
        for (key, mut spec, decoded_key) in ready {
            self.pending_voices.remove(&key);
            if matches!(
                key.group(),
                SoundVoiceGroup::MapAmbient | SoundVoiceGroup::Script
            ) {
                if let Some((looping, _, _)) = self.metadata_loop_info(&decoded_key) {
                    spec.looping = looping;
                }
            }
            if self.group_enabled(key.group()) {
                if let Err(error) = self.start_voice(key, spec, &decoded_key) {
                    self.status = error;
                }
            }
        }
    }

    fn ensure_audio_sink(&mut self) -> Result<(), String> {
        if self.audio_sink.is_some() {
            return Ok(());
        }
        match DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                self.audio_error = None;
                self.audio_sink = Some(sink);
                Ok(())
            }
            Err(error) => {
                let error = format!("Audio output unavailable: {error}");
                self.audio_error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn start_voice(
        &mut self,
        key: SoundVoiceKey,
        spec: SoundVoiceSpec,
        decoded_key: &DecodedSoundKey,
    ) -> Result<(), String> {
        self.ensure_audio_sink()?;
        let path = self
            .decoded_paths
            .get(decoded_key)
            .cloned()
            .ok_or_else(|| format!("Decoded WAV is missing for 0x{:08x}", spec.hashcode))?;
        let mixer = self.audio_sink.as_ref().unwrap().mixer();
        let player = SpatialPlayer::connect_new(
            mixer,
            pan_position(spec.pan),
            LISTENER_LEFT_EAR,
            LISTENER_RIGHT_EAR,
        );
        player.set_speed(spec.speed);

        let loop_info = self.metadata_loop_info(decoded_key);
        if spec.looping {
            if let Some((true, loop_start, Some(loop_end))) = loop_info {
                append_marker_loop(&player, &path, loop_start, loop_end, spec.seek_seconds)?;
            } else {
                let file = File::open(&path).map_err(|error| {
                    format!("Could not open decoded WAV '{}': {error}", path.display())
                })?;
                let decoder = Decoder::new_looped(BufReader::new(file)).map_err(|error| {
                    format!("Could not decode looped WAV '{}': {error}", path.display())
                })?;
                if spec.seek_seconds > 0.0 {
                    player
                        .append(decoder.skip_duration(Duration::from_secs_f32(spec.seek_seconds)));
                } else {
                    player.append(decoder);
                }
            }
        } else {
            let file = File::open(&path).map_err(|error| {
                format!("Could not open decoded WAV '{}': {error}", path.display())
            })?;
            let decoder = Decoder::try_from(file)
                .map_err(|error| format!("Could not decode WAV '{}': {error}", path.display()))?;
            if spec.seek_seconds > 0.0 {
                player.append(decoder.skip_duration(Duration::from_secs_f32(spec.seek_seconds)));
            } else {
                player.append(decoder);
            }
        }

        let target_volume = self.effective_volume(key.group(), spec.volume);
        let current_volume = if spec.fade_in_seconds > 0.0 {
            0.0
        } else {
            target_volume
        };
        player.set_volume(current_volume);
        self.voices.insert(
            key,
            ActiveVoice {
                player,
                hashcode: spec.hashcode,
                looping: spec.looping,
                base_volume: spec.volume,
                current_volume,
                target_volume,
                transition_seconds: spec.fade_in_seconds.max(0.0),
                fade_out_seconds: spec.fade_out_seconds.max(0.0),
                removing: false,
            },
        );
        self.status = format!("Playing 0x{:08x}", spec.hashcode);
        Ok(())
    }

    fn update_voices(&mut self, delta_seconds: f32) {
        let master = self.master_volume;
        let manual = self.manual_volume;
        let ambient = self.ambient_volume;
        let script = self.script_volume;
        let mut remove = Vec::new();
        let mut completed = Vec::new();

        for (key, voice) in &mut self.voices {
            if !voice.removing {
                let group_volume = match key.group() {
                    SoundVoiceGroup::Manual => manual,
                    SoundVoiceGroup::MapAmbient => ambient,
                    SoundVoiceGroup::Script => script,
                };
                voice.target_volume = (voice.base_volume * group_volume * master).max(0.0);
            }

            if voice.player.empty() {
                if !voice.looping {
                    completed.push(*key);
                }
                remove.push(*key);
                continue;
            }

            if voice.transition_seconds <= 0.0 {
                voice.current_volume = voice.target_volume;
            } else {
                let step = delta_seconds / voice.transition_seconds;
                voice.current_volume +=
                    (voice.target_volume - voice.current_volume) * step.clamp(0.0, 1.0);
            }
            voice.player.set_volume(voice.current_volume.max(0.0));

            if voice.removing && voice.current_volume <= SILENCE_EPSILON {
                voice.player.stop();
                remove.push(*key);
            }
        }

        for key in remove {
            self.voices.remove(&key);
        }
        self.completed_one_shots.extend(completed);
    }

    fn reset_voice(&mut self, key: SoundVoiceKey) {
        self.pending_voices.remove(&key);
        self.completed_one_shots.remove(&key);
        if let Some(voice) = self.voices.remove(&key) {
            voice.player.stop();
        }
    }

    fn invalidate_decode_cache(&mut self, reason: &str) {
        self.stop_all_immediately();
        self.pending_voices.clear();
        self.pending_decodes.clear();
        self.completed_one_shots.clear();
        self.sound_metadata.clear();
        self.failed_decodes.clear();
        self.decoded_paths.clear();
        self.metadata = None;
        self.status = reason.to_string();
    }

    fn stop_all_immediately(&mut self) {
        for (_, voice) in self.voices.drain() {
            voice.player.stop();
        }
    }
}

fn decoded_cache_path(key: &DecodedSoundKey) -> PathBuf {
    std::env::temp_dir()
        .join("eurochef-eurosound-cache")
        .join(format!("{:016x}", key.project_fingerprint))
        .join(format!("{:08x}_pool{}.wav", key.hashcode, key.pool_index))
}

impl Drop for SoundPreview {
    fn drop(&mut self) {
        self.stop_all_immediately();
        self.decode_worker.shutdown();
        while self.decode_worker.receiver.try_recv().is_ok() {}
    }
}

fn decode_native(
    request: &DecodeRequest,
    catalogs: &StdMutex<
        HashMap<PathBuf, Result<Arc<crate::sound_native::NativeSoundCatalog>, String>>,
    >,
) -> (Option<Value>, Result<(), String>) {
    let catalog = {
        let mut catalogs = catalogs.lock().expect("sound catalog cache poisoned");
        catalogs
            .entry(request.project_root.clone())
            .or_insert_with(|| {
                crate::sound_native::NativeSoundCatalog::load_pc_robots(&request.project_root)
                    .map(Arc::new)
            })
            .clone()
    };
    let catalog = match catalog {
        Ok(catalog) => catalog,
        Err(error) => return (None, Err(error)),
    };
    let wave = match catalog.wave(request.key.hashcode, request.key.pool_index) {
        Some(wave) => wave,
        None => {
            return (
                Some(serde_json::json!({ "playable": false, "audio_kind": "unsupported" })),
                Err(format!(
                    "0x{:08x} pool {} is not a PC soundbank wave",
                    request.key.hashcode, request.key.pool_index
                )),
            )
        }
    };
    let result = crate::sound_native::decode_wave(wave).and_then(|decoded| {
        write_pcm16_wave(
            &request.output_path,
            &decoded.samples,
            decoded.frequency,
            decoded.channels,
        )
    });
    (
        Some(serde_json::json!({
            "playable": result.is_ok(),
            "audio_kind": if wave.channels == 2 { "music" } else { "soundbank" },
            "hashcode": request.key.hashcode,
            "frequency": wave.frequency,
            "total_samples": wave.total_samples,
        })),
        result,
    )
}

fn write_pcm16_wave(
    path: &Path,
    samples: &[i16],
    frequency: u32,
    channels: u16,
) -> Result<(), String> {
    if frequency == 0 || channels == 0 {
        return Err("Soundbank wave has a zero sample frequency.".to_string());
    }
    let data_length = u32::try_from(samples.len() * 2).map_err(|_| "WAV is too large")?;
    let mut bytes = Vec::with_capacity(44 + data_length as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&frequency.to_le_bytes());
    bytes.extend_from_slice(&(frequency * channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn append_marker_loop(
    player: &SpatialPlayer,
    path: &Path,
    loop_start_seconds: f32,
    loop_end_seconds: f32,
    seek_seconds: f32,
) -> Result<(), String> {
    let loop_start = loop_start_seconds.max(0.0);
    let loop_end = loop_end_seconds.max(loop_start);
    let loop_length = loop_end - loop_start;
    if loop_length <= f32::EPSILON {
        return Err(format!(
            "Invalid marker loop range for '{}': {loop_start:.6}..{loop_end:.6}",
            path.display()
        ));
    }

    let seek = seek_seconds.max(0.0);
    if seek < loop_start {
        let intro_length = loop_start - seek;
        if intro_length > f32::EPSILON {
            let file = File::open(path).map_err(|error| {
                format!("Could not open decoded WAV '{}': {error}", path.display())
            })?;
            let decoder = Decoder::try_from(file)
                .map_err(|error| format!("Could not decode WAV '{}': {error}", path.display()))?;
            player.append(
                decoder
                    .skip_duration(Duration::from_secs_f32(seek))
                    .take_duration(Duration::from_secs_f32(intro_length)),
            );
        }
    } else {
        let loop_offset = (seek - loop_start) % loop_length;
        let remainder = loop_length - loop_offset;
        if remainder > f32::EPSILON && loop_offset > f32::EPSILON {
            let file = File::open(path).map_err(|error| {
                format!("Could not open decoded WAV '{}': {error}", path.display())
            })?;
            let decoder = Decoder::try_from(file)
                .map_err(|error| format!("Could not decode WAV '{}': {error}", path.display()))?;
            player.append(
                decoder
                    .skip_duration(Duration::from_secs_f32(loop_start + loop_offset))
                    .take_duration(Duration::from_secs_f32(remainder)),
            );
        }
    }

    let file = File::open(path)
        .map_err(|error| format!("Could not open decoded WAV '{}': {error}", path.display()))?;
    let decoder = Decoder::try_from(file)
        .map_err(|error| format!("Could not decode WAV '{}': {error}", path.display()))?;
    let loop_region = decoder
        .skip_duration(Duration::from_secs_f32(loop_start))
        .take_duration(Duration::from_secs_f32(loop_length))
        .buffered()
        .repeat_infinite();
    player.append(loop_region);
    Ok(())
}

fn project_fingerprint(project_root: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    project_root.to_ascii_lowercase().hash(&mut hasher);
    hasher.finish()
}

fn pan_position(pan: f32) -> [f32; 3] {
    let pan = pan.clamp(-1.0, 1.0);
    let forward = (1.0 - pan * pan).max(0.0).sqrt();
    [pan * 2.0, 0.0, forward * 2.0]
}

pub fn map_distance_gain(distance: f32, inner_radius: f32, outer_radius: f32) -> f32 {
    let distance = distance.max(0.0);
    let inner = inner_radius.max(0.0);
    let outer = outer_radius.max(inner);
    if outer <= 0.0 {
        return 1.0;
    }
    if distance <= inner {
        1.0
    } else if distance >= outer {
        0.0
    } else if outer <= inner + f32::EPSILON {
        0.0
    } else {
        1.0 - (distance - inner) / (outer - inner)
    }
}

pub fn serialized_sound_volume(volume: u8) -> f32 {
    (volume as f32 / 100.0).clamp(0.0, 2.55)
}

pub fn serialized_fade_seconds(value: u8) -> f32 {
    value as f32 / 60.0
}

fn discover_project_root(source_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("EUROSOUND_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.exists() {
            return Some(root);
        }
    }

    let source_path = source_path?;
    for ancestor in source_path.ancestors() {
        if ancestor
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("_eurotools_out"))
            .unwrap_or(false)
        {
            if let Some(project_root) = ancestor.parent() {
                return Some(project_root.to_path_buf());
            }
        }
    }
    source_path.parent().map(Path::to_path_buf)
}

fn format_probe_summary(value: &Value) -> String {
    let kind = value
        .get("audio_kind")
        .and_then(Value::as_str)
        .unwrap_or("sfx");
    if kind == "metadata_only" {
        let details = value
            .get("details_file")
            .and_then(Value::as_str)
            .and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown details file");
        let duration = value
            .get("duration_seconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let inner = value
            .get("inner_radius")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let outer = value
            .get("outer_radius")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let looping = value
            .get("looping")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let streamed = value
            .get("sample_streamed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return format!(
            "Kind: metadata only (not playable)\nDetails: {details}\nDuration: {duration:.3} s\nRadius: {inner}..{outer}\nLooping: {looping}\nStreamed: {streamed}"
        );
    }

    let bank = value
        .get("bank_file")
        .and_then(Value::as_str)
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown bank");
    let wave = value.get("selected_wave").unwrap_or(&Value::Null);
    let frequency = value
        .get("frequency")
        .and_then(Value::as_u64)
        .or_else(|| wave.get("frequency").and_then(Value::as_u64))
        .unwrap_or(0);
    let samples = wave
        .get("total_samples")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let encoded = wave
        .get("encoded_size")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .get("encoded_bytes_per_channel")
                .and_then(Value::as_u64)
        })
        .unwrap_or(0);
    let looping = value
        .get("looping")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    format!(
        "Kind: {kind}\nBank: {bank}\nFrequency: {frequency} Hz\nSamples: {samples}\nEncoded: {encoded} bytes\nLooping: {looping}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_uses_parent_of_eurotools_out() {
        let source = Path::new(
            "D:/Games/Robots/_eurotools_out/extracted_main/robots/binary/_bin_pc/m03_hub1.edb",
        );
        assert_eq!(
            discover_project_root(Some(source)),
            Some(PathBuf::from("D:/Games/Robots"))
        );
    }

    #[test]
    fn decoded_cache_path_is_stable_between_gui_processes() {
        let key = DecodedSoundKey {
            project_fingerprint: 0x0123_4567_89ab_cdef,
            hashcode: 0x1af0_0312,
            pool_index: 0,
        };
        let path = decoded_cache_path(&key);
        assert!(path.ends_with("0123456789abcdef\\1af00312_pool0.wav"));
        assert_eq!(path, decoded_cache_path(&key));
    }

    #[test]
    fn probe_summary_keeps_bank_and_wave_values() {
        let value: Value = serde_json::json!({
            "bank_file": "D:/audio/usa_sb_m02_city.sfx",
            "selected_wave": {
                "frequency": 44100,
                "total_samples": 17304,
                "encoded_size": 9888
            }
        });
        let summary = format_probe_summary(&value);
        assert!(summary.contains("usa_sb_m02_city.sfx"));
        assert!(summary.contains("44100 Hz"));
        assert!(summary.contains("17304"));
    }

    #[test]
    fn map_distance_gain_uses_inner_full_volume_and_outer_silence() {
        assert_eq!(map_distance_gain(0.0, 5.0, 15.0), 1.0);
        assert_eq!(map_distance_gain(5.0, 5.0, 15.0), 1.0);
        assert!((map_distance_gain(10.0, 5.0, 15.0) - 0.5).abs() < 0.0001);
        assert_eq!(map_distance_gain(15.0, 5.0, 15.0), 0.0);
        assert_eq!(map_distance_gain(20.0, 5.0, 15.0), 0.0);
    }

    #[test]
    fn serialized_sound_fields_have_stable_preview_scaling() {
        assert_eq!(serialized_sound_volume(100), 1.0);
        assert_eq!(serialized_fade_seconds(60), 1.0);
    }

    #[test]
    fn pan_position_is_symmetric() {
        let left = pan_position(-1.0);
        let right = pan_position(1.0);
        assert_eq!(left[0], -right[0]);
        assert_eq!(left[2], right[2]);
    }

    #[test]
    fn null_and_reverb_references_are_not_playable() {
        assert!(!is_playable_audio_reference(0));
        assert!(!is_playable_audio_reference(u32::MAX));
        assert!(!is_playable_audio_reference(0x1C00_0000));
        assert!(is_playable_audio_reference(0x1AF0_0312));
        assert!(is_playable_audio_reference(0x1B00_0024));
    }

    #[test]
    fn real_native_mixer_when_fixture_is_requested() {
        let (Ok(root), Ok(source_edb)) = (
            std::env::var("EUROCHEF_REAL_AUDIO_ROOT"),
            std::env::var("EUROCHEF_REAL_AUDIO_EDB"),
        ) else {
            eprintln!("SKIP real_native_mixer_when_fixture_is_requested: real audio environment is incomplete");
            return;
        };

        let mut preview = SoundPreview::new(Some(Path::new(&source_edb)));
        preview.project_root = root;
        preview.master_volume = 0.0;
        preview.manual_volume = 0.0;
        preview.request_voice(
            SoundVoiceKey::Manual,
            SoundVoiceSpec {
                hashcode: 0x1AF0_0312,
                looping: true,
                volume: 1.0,
                speed: 1.0,
                pan: -0.5,
                fade_in_seconds: 0.0,
                fade_out_seconds: 0.0,
                seek_seconds: 0.0,
            },
        );
        preview.request_voice(
            SoundVoiceKey::Script {
                file: 0x0100_0071,
                script: 0x0400_0000,
                command_path: 1,
            },
            SoundVoiceSpec {
                hashcode: 0x1B00_0024,
                looping: true,
                volume: 1.0,
                speed: 1.0,
                pan: 0.5,
                fade_in_seconds: 0.0,
                fade_out_seconds: 0.0,
                seek_seconds: 10.0,
            },
        );
        let unavailable_voice = SoundVoiceKey::MapAmbient {
            map_hashcode: 0x0500_0000,
            sound_index: 3,
        };
        let unavailable_decoded = preview.decoded_key(0x1AF0_0003);
        preview.request_voice(
            unavailable_voice,
            SoundVoiceSpec {
                hashcode: 0x1AF0_0003,
                looping: false,
                volume: 1.0,
                speed: 1.0,
                pan: 0.0,
                fade_in_seconds: 0.0,
                fade_out_seconds: 0.0,
                seek_seconds: 0.0,
            },
        );

        let deadline = Instant::now() + std::time::Duration::from_secs(15);
        while Instant::now() < deadline
            && (preview.voice_count(SoundVoiceGroup::Manual) == 0
                || preview.voice_count(SoundVoiceGroup::Script) == 0
                || !preview.failed_decodes.contains_key(&unavailable_decoded))
            && preview.audio_error.is_none()
        {
            preview.tick();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert!(
            preview.audio_error.is_none(),
            "real mixer output failed: {:?}",
            preview.audio_error
        );
        assert_eq!(preview.voice_count(SoundVoiceGroup::Manual), 1);
        assert_eq!(preview.voice_count(SoundVoiceGroup::Script), 1);
        assert_eq!(preview.voice_count(SoundVoiceGroup::MapAmbient), 0);
        assert!(!preview.pending_voices.contains_key(&unavailable_voice));
        let unavailable_error = preview
            .failed_decodes
            .get(&unavailable_decoded)
            .expect("missing sound was not retained in the failed-decode cache");
        assert!(unavailable_error.contains("not a PC soundbank wave"));
        assert!(preview.sound_metadata.contains_key(&unavailable_decoded));
        for expected in [0x1AF0_0312u64, 0x1B00_0024u64] {
            assert!(preview.sound_metadata.values().any(|metadata| {
                metadata.get("hashcode").and_then(Value::as_u64) == Some(expected)
            }), "missing native metadata for 0x{expected:08x}: {:?}", preview.sound_metadata);
        }
        let music_metadata = preview
            .sound_metadata
            .values()
            .find(|metadata| metadata.get("hashcode").and_then(Value::as_u64) == Some(0x1B00_0024))
            .expect("music metadata was not retained");
        assert_eq!(music_metadata.get("audio_kind").and_then(Value::as_str), Some("music"));
        assert_eq!(music_metadata.get("frequency").and_then(Value::as_u64), Some(32_000));
        preview.stop_group(SoundVoiceGroup::Manual, 0.0);
        preview.stop_group(SoundVoiceGroup::Script, 0.0);
        assert_eq!(preview.voice_count(SoundVoiceGroup::Manual), 0);
        assert_eq!(preview.voice_count(SoundVoiceGroup::Script), 0);
    }
}
