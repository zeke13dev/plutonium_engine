#[derive(Debug, Clone, Copy)]
pub enum Ease {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// CSS-like cubic-bezier; maps input progress t in [0,1] to output y by solving x(t)=progress.
    CubicBezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

pub fn ease_value(e: Ease, t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    match e {
        Ease::Linear => x,
        Ease::EaseIn => x * x,
        Ease::EaseOut => 1.0 - (1.0 - x) * (1.0 - x),
        Ease::EaseInOut => {
            if x < 0.5 {
                2.0 * x * x
            } else {
                1.0 - (-2.0 * x + 2.0).powi(2) / 2.0
            }
        }
        Ease::CubicBezier { x1, y1, x2, y2 } => cubic_bezier_solve(x1, y1, x2, y2, x),
    }
}

// Solve y given progress p in [0,1] for a cubic-bezier defined by (0,0),(x1,y1),(x2,y2),(1,1)
// using Newton-Raphson on x(t)=p then evaluate y(t).
fn cubic_bezier_solve(x1: f32, y1: f32, x2: f32, y2: f32, p: f32) -> f32 {
    // Clamp control points to sane ranges
    let x1 = x1.clamp(0.0, 1.0);
    let x2 = x2.clamp(0.0, 1.0);
    // Initial guess: p
    let mut t = p;
    for _ in 0..6 {
        let (x_t, dx_dt) = bezier_x_and_derivative(t, x1, x2);
        let err = x_t - p;
        if err.abs() < 1e-4 {
            break;
        }
        if dx_dt.abs() > 1e-6 {
            t -= err / dx_dt;
        }
        t = t.clamp(0.0, 1.0);
    }
    bezier_y(t, y1, y2)
}

#[inline]
fn bezier_x_and_derivative(t: f32, x1: f32, x2: f32) -> (f32, f32) {
    // x(t) = 3(1-t)^2 t x1 + 3(1-t) t^2 x2 + t^3
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let x = 3.0 * uu * t * x1 + 3.0 * u * tt * x2 + tt * t;
    // dx/dt = 3( (1-t)^2 x1 + 2(1-t)t(x2 - x1) + t^2(1 - x2) )
    let dx = 3.0 * (uu * x1 + 2.0 * u * t * (x2 - x1) + tt * (1.0 - x2));
    (x, dx)
}

#[inline]
fn bezier_y(t: f32, y1: f32, y2: f32) -> f32 {
    let u = 1.0 - t;
    let b0 = u * u * u;
    let b1 = 3.0 * u * u * t;
    let b2 = 3.0 * u * t * t;
    let b3 = t * t * t;
    b1 * y1 + b2 * y2 + b3 // P0.y=0,P3.y=1
}

#[derive(Debug, Clone)]
pub struct Tween<T> {
    pub start: T,
    pub end: T,
    pub duration: f32,
    pub elapsed: f32,
    pub ease: Ease,
}

impl<
        T: Copy
            + core::ops::Add<Output = T>
            + core::ops::Sub<Output = T>
            + core::ops::Mul<f32, Output = T>,
    > Tween<T>
{
    pub fn new(start: T, end: T, duration: f32, ease: Ease) -> Self {
        Self {
            start,
            end,
            duration,
            elapsed: 0.0,
            ease,
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn sample(&self) -> T {
        if self.duration <= 0.0 {
            return self.end;
        }
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let w = ease_value(self.ease, t);
        self.start + (self.end - self.start) * w
    }

    pub fn step(&mut self, dt: f32) -> T {
        self.elapsed += dt;
        self.sample()
    }
}

#[derive(Debug, Clone)]
pub enum Track<T> {
    Sequence(Vec<Tween<T>>),
    Parallel(Vec<Tween<T>>),
}

impl<T> Track<T>
where
    T: Copy
        + core::ops::Add<Output = T>
        + core::ops::Sub<Output = T>
        + core::ops::Mul<f32, Output = T>,
{
    pub fn step(&mut self, dt: f32) -> Vec<T> {
        match self {
            Track::Sequence(ref mut tweens) => {
                let mut out = Vec::new();
                if tweens.is_empty() {
                    return out;
                }
                let mut i = 0usize;
                let mut remaining_dt = dt;
                while i < tweens.len() {
                    let before = tweens[i].elapsed;
                    let v = tweens[i].step(remaining_dt);
                    out.push(v);
                    if tweens[i].is_finished() {
                        // deduct used dt if any remained after finishing
                        let used = tweens[i].duration - before;
                        remaining_dt = (remaining_dt - used).max(0.0);
                        i += 1;
                        if i >= tweens.len() || remaining_dt <= 0.0 {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                out
            }
            Track::Parallel(ref mut tweens) => tweens.iter_mut().map(|tw| tw.step(dt)).collect(),
        }
    }
}

pub struct Timeline<T>
where
    T: Copy
        + core::ops::Add<Output = T>
        + core::ops::Sub<Output = T>
        + core::ops::Mul<f32, Output = T>,
{
    tracks: Vec<Track<T>>,
    time: f32,
    rate: f32,
    playing: bool,
    labels: std::collections::HashMap<String, f32>,
    callbacks: Vec<TimelineCallback>,
}

impl<T> Timeline<T>
where
    T: Copy
        + core::ops::Add<Output = T>
        + core::ops::Sub<Output = T>
        + core::ops::Mul<f32, Output = T>,
{
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            time: 0.0,
            rate: 1.0,
            playing: true,
            labels: std::collections::HashMap::new(),
            callbacks: Vec::new(),
        }
    }
    pub fn push_track(&mut self, track: Track<T>) {
        self.tracks.push(track);
    }
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
    }
    pub fn play(&mut self) {
        self.playing = true;
    }
    pub fn pause(&mut self) {
        self.playing = false;
    }
    pub fn seek(&mut self, _t: f32) {
        // Re-simulate all tracks from t=0 to requested time using a fixed small step for determinism.
        // This is a simple implementation intended for tests/snapshots, not a high-perf runtime.
        let t = _t.max(0.0);
        // Reset internal time and callbacks fired state
        self.time = 0.0;
        for cb in &mut self.callbacks {
            cb.fired = false;
        }
        // Reset all tweens by reconstructing them from their current values; we need owned data.
        // We approximate rewinding by creating new tracks with same tweens and calling reset.
        // Sequence/Parallel reset
        for tr in &mut self.tracks {
            match tr {
                Track::Sequence(tweens) | Track::Parallel(tweens) => {
                    for tw in tweens.iter_mut() {
                        tw.reset();
                    }
                }
            }
        }
        // Fixed small delta to step forward deterministically
        let dt = 1.0 / 240.0; // 240 Hz stepping for accuracy
        let mut rem = t;
        while rem > 0.0 {
            let step = if rem < dt { rem } else { dt };
            let prev_time = self.time;
            self.time += step;
            // Fire callbacks crossed in this step
            for cb in &mut self.callbacks {
                if !cb.fired && cb.time > prev_time && cb.time <= self.time {
                    (cb.func)();
                    cb.fired = true;
                }
            }
            // Advance tracks
            for tr in &mut self.tracks {
                let _ = tr.step(step);
            }
            rem -= step;
        }
    }
    pub fn step(&mut self, dt: f32) -> Vec<Vec<T>> {
        if !self.playing {
            return vec![];
        }
        let scaled = dt * self.rate;
        let prev_time = self.time;
        self.time += scaled;
        // Fire any callbacks whose time is now reached
        for cb in &mut self.callbacks {
            if !cb.fired && cb.time > prev_time && cb.time <= self.time {
                (cb.func)();
                cb.fired = true;
            }
        }
        self.tracks.iter_mut().map(|tr| tr.step(scaled)).collect()
    }

    // Labels
    pub fn add_label(&mut self, name: impl Into<String>, at_time: f32) {
        self.labels.insert(name.into(), at_time);
    }
    pub fn label_time(&self, name: &str) -> Option<f32> {
        self.labels.get(name).copied()
    }

    // Callbacks
    pub fn on_at(&mut self, at_time: f32, f: impl FnMut() + 'static) {
        self.callbacks.push(TimelineCallback {
            time: at_time,
            fired: false,
            func: Box::new(f),
        });
    }
    pub fn on_label(&mut self, name: &str, f: impl FnMut() + 'static) {
        if let Some(t) = self.labels.get(name).copied() {
            self.on_at(t, f);
        }
    }
}

struct TimelineCallback {
    time: f32,
    fired: bool,
    func: Box<dyn FnMut()>,
}
