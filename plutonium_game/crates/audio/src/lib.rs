#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Audio {
    master_volume: Arc<Mutex<f32>>, // 0..1
    bgm_volume: Arc<Mutex<f32>>,    // 0..1
    sfx_volume: Arc<Mutex<f32>>,    // 0..1
    sfx_cooldown_ms: Arc<Mutex<u64>>,
    #[cfg(feature = "rodio-backend")]
    inner: Arc<rodio_impl::AudioInner>,
    // For debounce/throttle even when rodio is disabled
    last_played: Arc<Mutex<HashMap<String, Instant>>>,
}

impl Audio {
    pub fn new() -> Self {
        Self {
            master_volume: Arc::new(Mutex::new(1.0)),
            bgm_volume: Arc::new(Mutex::new(1.0)),
            sfx_volume: Arc::new(Mutex::new(1.0)),
            sfx_cooldown_ms: Arc::new(Mutex::new(80)),
            #[cfg(feature = "rodio-backend")]
            inner: Arc::new(rodio_impl::AudioInner::new()),
            last_played: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn set_master_volume(&self, v: f32) {
        let vv = v.clamp(0.0, 1.0);
        if let Ok(mut m) = self.master_volume.lock() {
            *m = vv;
        }
        self.update_bgm_volume();
    }
    pub fn master_volume(&self) -> f32 {
        self.master_volume.lock().map(|g| *g).unwrap_or(1.0)
    }

    pub fn set_bgm_volume(&self, v: f32) {
        if let Ok(mut m) = self.bgm_volume.lock() {
            *m = v.clamp(0.0, 1.0);
        }
        self.update_bgm_volume();
    }
    pub fn set_sfx_volume(&self, v: f32) {
        if let Ok(mut m) = self.sfx_volume.lock() {
            *m = v.clamp(0.0, 1.0);
        }
    }
    pub fn set_sfx_cooldown_ms(&self, ms: u64) {
        if let Ok(mut c) = self.sfx_cooldown_ms.lock() {
            *c = ms;
        }
    }

    #[allow(dead_code)] // part of the in-progress mixer API
    fn effective_bgm_volume(&self) -> f32 {
        let master = self.master_volume.lock().map(|g| *g).unwrap_or(1.0);
        let bgm = self.bgm_volume.lock().map(|g| *g).unwrap_or(1.0);
        (master * bgm).clamp(0.0, 1.0)
    }
    #[allow(dead_code)] // part of the in-progress mixer API
    fn effective_sfx_volume(&self) -> f32 {
        let master = self.master_volume.lock().map(|g| *g).unwrap_or(1.0);
        let sfx = self.sfx_volume.lock().map(|g| *g).unwrap_or(1.0);
        (master * sfx).clamp(0.0, 1.0)
    }

    fn should_play(&self, key: &str) -> bool {
        let now = Instant::now();
        let cooldown = self.sfx_cooldown_ms.lock().map(|g| *g).unwrap_or(80);
        let mut map = self.last_played.lock().unwrap();
        if let Some(prev) = map.get(key) {
            if now.duration_since(*prev) < Duration::from_millis(cooldown) {
                return false;
            }
        }
        map.insert(key.to_string(), now);
        true
    }

    #[cfg(feature = "rodio-backend")]
    fn update_bgm_volume(&self) {
        self.inner.set_bgm_volume(self.effective_bgm_volume());
    }
    #[cfg(not(feature = "rodio-backend"))]
    fn update_bgm_volume(&self) {}

    #[cfg(feature = "rodio-backend")]
    pub fn play_bgm_loop(&self, path: &str) {
        self.inner.play_bgm_loop(path, self.effective_bgm_volume());
    }
    #[cfg(not(feature = "rodio-backend"))]
    pub fn play_bgm_loop(&self, _path: &str) {}

    #[cfg(feature = "rodio-backend")]
    pub fn stop_bgm(&self) {
        self.inner.stop_bgm();
    }
    #[cfg(not(feature = "rodio-backend"))]
    pub fn stop_bgm(&self) {}

    #[cfg(feature = "rodio-backend")]
    pub fn play_sfx(&self, path: &str) {
        if !self.should_play(path) {
            return;
        }
        self.inner.play_sfx(path, self.effective_sfx_volume());
    }
    #[cfg(not(feature = "rodio-backend"))]
    pub fn play_sfx(&self, path: &str) {
        let _ = path;
        if !self.should_play(path) {
            return;
        }
    }
}

#[cfg(feature = "rodio-backend")]
mod rodio_impl {
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use std::sync::mpsc::{self, Sender};
    use std::sync::Mutex;
    use std::thread;

    // rodio's `OutputStream`/`Sink` wrap a thread-affine `cpal` stream and are
    // therefore `!Send`/`!Sync`. The ECS `World` stores resources under a
    // `Send + Sync` bound, so we keep the audio handles entirely on a dedicated
    // thread and expose only a `Send + Sync` command channel.
    enum Cmd {
        SetBgmVolume(f32),
        PlayBgmLoop { path: String, vol: f32 },
        StopBgm,
        PlaySfx { path: String, vol: f32 },
    }

    pub struct AudioInner {
        tx: Mutex<Sender<Cmd>>,
    }

    fn decode(path: &str) -> Option<Decoder<BufReader<File>>> {
        if !Path::new(path).exists() {
            eprintln!("[audio] file not found: {}", path);
            return None;
        }
        match File::open(path) {
            Ok(file) => match Decoder::new(BufReader::new(file)) {
                Ok(src) => Some(src),
                Err(e) => {
                    eprintln!("[audio] decode failed for {}: {}", path, e);
                    None
                }
            },
            Err(e) => {
                eprintln!("[audio] open failed for {}: {}", path, e);
                None
            }
        }
    }

    impl AudioInner {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel::<Cmd>();
            thread::spawn(move || {
                // The `!Send` rodio handles live and die on this thread only.
                let (_stream, handle) = match OutputStream::try_default() {
                    Ok(pair) => pair,
                    Err(e) => {
                        eprintln!("[audio] rodio output stream unavailable: {}", e);
                        return;
                    }
                };
                let mut bgm_sink: Option<Sink> = None;
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        Cmd::SetBgmVolume(vol) => {
                            if let Some(sink) = bgm_sink.as_ref() {
                                sink.set_volume(vol.max(0.0));
                            }
                        }
                        Cmd::StopBgm => {
                            if let Some(sink) = bgm_sink.take() {
                                sink.stop();
                            }
                        }
                        Cmd::PlayBgmLoop { path, vol } => {
                            if let Some(sink) = bgm_sink.take() {
                                sink.stop();
                            }
                            if let Some(src) = decode(&path) {
                                if let Ok(sink) = Sink::try_new(&handle) {
                                    sink.set_volume(vol.max(0.0));
                                    sink.append(src.repeat_infinite());
                                    sink.play();
                                    bgm_sink = Some(sink);
                                }
                            }
                        }
                        Cmd::PlaySfx { path, vol } => {
                            if let Some(src) = decode(&path) {
                                if let Ok(sink) = Sink::try_new(&handle) {
                                    sink.set_volume(vol.max(0.0));
                                    sink.append(src);
                                    sink.detach(); // play out without holding the handle
                                }
                            }
                        }
                    }
                }
            });
            Self { tx: Mutex::new(tx) }
        }

        fn send(&self, cmd: Cmd) {
            if let Ok(tx) = self.tx.lock() {
                let _ = tx.send(cmd);
            }
        }

        pub fn set_bgm_volume(&self, vol: f32) {
            self.send(Cmd::SetBgmVolume(vol));
        }

        pub fn play_bgm_loop(&self, path: &str, vol: f32) {
            self.send(Cmd::PlayBgmLoop {
                path: path.to_string(),
                vol,
            });
        }

        pub fn stop_bgm(&self) {
            self.send(Cmd::StopBgm);
        }

        pub fn play_sfx(&self, path: &str, vol: f32) {
            self.send(Cmd::PlaySfx {
                path: path.to_string(),
                vol,
            });
        }
    }
}
