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

    fn effective_bgm_volume(&self) -> f32 {
        let master = self.master_volume.lock().map(|g| *g).unwrap_or(1.0);
        let bgm = self.bgm_volume.lock().map(|g| *g).unwrap_or(1.0);
        (master * bgm).clamp(0.0, 1.0)
    }
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
    use super::*;
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;

    pub struct AudioInner {
        _stream: OutputStream,
        stream_handle: rodio::OutputStreamHandle,
        bgm_sink: Arc<Mutex<Option<Sink>>>,
    }

    impl AudioInner {
        pub fn new() -> Self {
            let (stream, handle) = OutputStream::try_default().expect("rodio output stream");
            Self {
                _stream: stream,
                stream_handle: handle,
                bgm_sink: Arc::new(Mutex::new(None)),
            }
        }

        pub fn set_bgm_volume(&self, vol: f32) {
            if let Ok(mut sink_opt) = self.bgm_sink.lock() {
                if let Some(sink) = sink_opt.as_ref() {
                    sink.set_volume(vol.max(0.0));
                }
            }
        }

        pub fn play_bgm_loop(&self, path: &str, vol: f32) {
            // Stop existing
            self.stop_bgm();
            if !Path::new(path).exists() {
                eprintln!("[audio] bgm not found: {}", path);
                return;
            }
            match File::open(path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    match Decoder::new(reader) {
                        Ok(src) => {
                            let sink = Sink::try_new(&self.stream_handle).ok();
                            if let Some(sink) = sink {
                                sink.set_volume(vol.max(0.0));
                                sink.append(src.repeat_infinite());
                                sink.play();
                                if let Ok(mut guard) = self.bgm_sink.lock() {
                                    *guard = Some(sink);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[audio] decode bgm failed: {}", e);
                        }
                    }
                }
                Err(e) => eprintln!("[audio] open bgm failed: {}", e),
            }
        }

        pub fn stop_bgm(&self) {
            if let Ok(mut sink_opt) = self.bgm_sink.lock() {
                if let Some(sink) = sink_opt.take() {
                    sink.stop();
                }
            }
        }

        pub fn play_sfx(&self, path: &str, vol: f32) {
            if !Path::new(path).exists() {
                eprintln!("[audio] sfx not found: {}", path);
                return;
            }
            let file = match File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[audio] open sfx failed: {}", e);
                    return;
                }
            };
            let reader = BufReader::new(file);
            let src = match Decoder::new(reader) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[audio] decode sfx failed: {}", e);
                    return;
                }
            };
            if let Ok(sink) = Sink::try_new(&self.stream_handle) {
                sink.set_volume(vol.max(0.0));
                sink.append(src);
                sink.detach(); // allow to play out without holding handle
            }
        }
    }
}
