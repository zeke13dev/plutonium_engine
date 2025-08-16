#![forbid(unsafe_code)]

use ahash::AHashMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct Time {
    pub delta_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity(pub u32);

pub trait Component: 'static + Send + Sync {}

impl<T: 'static + Send + Sync> Component for T {}

#[derive(Default)]
pub struct World {
    next_id: u32,
    // Very simple component stores keyed by TypeId string
    components: AHashMap<&'static str, AHashMap<u32, Box<dyn std::any::Any + Send + Sync>>>,
    resources: AHashMap<&'static str, Box<dyn std::any::Any + Send + Sync>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self) -> Entity {
        let id = self.next_id;
        self.next_id += 1;
        Entity(id)
    }

    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        let store = self
            .components
            .entry(std::any::type_name::<T>())
            .or_default();
        store.insert(entity.0, Box::new(component));
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.components
            .get(std::any::type_name::<T>())
            .and_then(|store| store.get(&entity.0))
            .and_then(|b| b.downcast_ref::<T>())
    }

    pub fn get_component_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        self.components
            .get_mut(std::any::type_name::<T>())
            .and_then(|store| store.get_mut(&entity.0))
            .and_then(|b| b.downcast_mut::<T>())
    }

    pub fn insert_resource<R: 'static + Send + Sync>(&mut self, resource: R) {
        self.resources
            .insert(std::any::type_name::<R>(), Box::new(resource));
    }

    pub fn get_resource<R: 'static + Send + Sync>(&self) -> Option<&R> {
        self.resources
            .get(std::any::type_name::<R>())
            .and_then(|b| b.downcast_ref::<R>())
    }

    pub fn get_resource_mut<R: 'static + Send + Sync>(&mut self) -> Option<&mut R> {
        self.resources
            .get_mut(std::any::type_name::<R>())
            .and_then(|b| b.downcast_mut::<R>())
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(store) = self.components.get_mut(std::any::type_name::<T>()) {
            store.remove(&entity.0);
        }
    }

    pub fn despawn(&mut self, entity: Entity) {
        for store in self.components.values_mut() {
            store.remove(&entity.0);
        }
    }

    /// Temporarily remove a resource of type `R`, run a closure with `&mut R` and `&mut World`,
    /// then insert the resource back. Returns `None` if the resource does not exist.
    pub fn with_resource_mut<R: 'static + Send + Sync, T>(
        &mut self,
        f: impl FnOnce(&mut R, &mut World) -> T,
    ) -> Option<T> {
        let key = std::any::type_name::<R>();
        let boxed = self.resources.remove(key)?;
        // Downcast to the concrete type
        let mut resource = match boxed.downcast::<R>() {
            Ok(b) => *b,
            Err(_) => return None,
        };
        let out = f(&mut resource, self);
        self.resources.insert(key, Box::new(resource));
        Some(out)
    }
}

type SystemFn = Box<dyn Fn(&mut World) + Send + Sync>;

#[derive(Default)]
pub struct Schedule {
    systems: Vec<SystemFn>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    pub fn with_system(mut self, f: impl Fn(&mut World) + Send + Sync + 'static) -> Self {
        self.systems.push(Box::new(f));
        self
    }

    pub fn add_system(&mut self, f: impl Fn(&mut World) + Send + Sync + 'static) {
        self.systems.push(Box::new(f));
    }

    pub fn run(&self, world: &mut World) {
        for sys in &self.systems {
            (sys)(world);
        }
    }
}

#[derive(Default)]
pub struct App {
    pub world: World,
    pub startup: Schedule,
    pub update: Schedule,
    pub fixed_update: Schedule,
    pub render: Schedule,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run_startup(&mut self) {
        self.startup.run(&mut self.world);
    }

    pub fn run_update(&mut self, delta_seconds: f32) {
        if let Some(time) = self.world.get_resource_mut::<Time>() {
            time.delta_seconds = delta_seconds;
        } else {
            self.world.insert_resource(Time { delta_seconds });
        }
        if let Some(frame) = self.world.get_resource_mut::<FrameNumber>() {
            frame.0 = frame.0.wrapping_add(1);
        } else {
            self.world.insert_resource(FrameNumber(1));
        }
        self.update.run(&mut self.world);
    }
}

// Frame counter resource
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameNumber(pub u64);

// Simple scene stack resource
#[derive(Debug, Default, Clone)]
pub struct SceneStack {
    stack: Vec<String>,
}

impl SceneStack {
    pub fn top(&self) -> Option<&str> {
        self.stack.last().map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct SceneEnter(pub String);
#[derive(Debug, Clone)]
pub struct SceneExit(pub String);

pub fn scene_push(world: &mut World, name: impl Into<String>) {
    let name = name.into();
    let scenes = world.get_resource_mut::<SceneStack>();
    let stack = if let Some(s) = scenes {
        s
    } else {
        world.insert_resource(SceneStack::default());
        world.get_resource_mut::<SceneStack>().unwrap()
    };
    stack.stack.push(name.clone());
    world.send_event(SceneEnter(name));
}

pub fn scene_pop(world: &mut World) {
    if let Some(stack) = world.get_resource_mut::<SceneStack>() {
        if let Some(name) = stack.stack.pop() {
            world.send_event(SceneExit(name));
        }
    }
}

pub fn scene_replace(world: &mut World, name: impl Into<String>) {
    let name = name.into();
    if let Some(stack) = world.get_resource_mut::<SceneStack>() {
        let prev = stack.stack.pop();
        let _ = stack;
        if let Some(p) = prev {
            world.send_event(SceneExit(p));
        }
        let stack = world.get_resource_mut::<SceneStack>().unwrap();
        stack.stack.push(name.clone());
        world.send_event(SceneEnter(name));
    } else {
        world.insert_resource(SceneStack {
            stack: vec![name.clone()],
        });
        world.send_event(SceneEnter(name));
    }
}

// Scene-specific schedules registry
#[derive(Default)]
pub struct SceneSystems {
    startup: AHashMap<String, Schedule>,
    update: AHashMap<String, Schedule>,
    render: AHashMap<String, Schedule>,
    exit: AHashMap<String, Schedule>,
}

impl SceneSystems {
    pub fn register_startup(&mut self, scene: &str, schedule: Schedule) {
        self.startup.insert(scene.to_string(), schedule);
    }
    pub fn register_update(&mut self, scene: &str, schedule: Schedule) {
        self.update.insert(scene.to_string(), schedule);
    }
    pub fn register_render(&mut self, scene: &str, schedule: Schedule) {
        self.render.insert(scene.to_string(), schedule);
    }
    pub fn register_exit(&mut self, scene: &str, schedule: Schedule) {
        self.exit.insert(scene.to_string(), schedule);
    }
    pub fn run_startup_for(&self, scene: &str, world: &mut World) {
        if let Some(s) = self.startup.get(scene) {
            s.run(world);
        }
    }
    pub fn run_update_for(&self, scene: &str, world: &mut World) {
        if let Some(s) = self.update.get(scene) {
            s.run(world);
        }
    }
    pub fn run_render_for(&self, scene: &str, world: &mut World) {
        if let Some(s) = self.render.get(scene) {
            s.run(world);
        }
    }
    pub fn run_exit_for(&self, scene: &str, world: &mut World) {
        if let Some(s) = self.exit.get(scene) {
            s.run(world);
        }
    }
}

/// Helper: consume `SceneEnter`/`SceneExit` events and invoke registered schedules.
pub fn process_scene_events(world: &mut World) {
    let enters = world.drain_events::<SceneEnter>();
    for e in enters {
        let scene = e.0;
        let _ = world.with_resource_mut::<SceneSystems, _>(|sys, w| {
            sys.run_startup_for(&scene, w);
        });
    }
    let exits = world.drain_events::<SceneExit>();
    for e in exits {
        let scene = e.0;
        let _ = world.with_resource_mut::<SceneSystems, _>(|sys, w| {
            sys.run_exit_for(&scene, w);
        });
    }
}

/// Helper: run update for current scene (top of stack if present)
pub fn run_current_scene_update(world: &mut World) {
    let name_opt = world
        .get_resource::<SceneStack>()
        .and_then(|s| s.top().map(|t| t.to_string()));
    if let Some(scene_name) = name_opt {
        let _ = world.with_resource_mut::<SceneSystems, _>(|sys, w| {
            sys.run_update_for(&scene_name, w);
        });
    }
}

// Deterministic RNG (xorshift64*)
#[derive(Debug, Clone, Copy)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(2685821657736338717)
    }
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() as f32) / (u64::MAX as f32)
    }
}

// Simple tween component and system helpers
#[derive(Debug, Clone, Copy)]
pub struct TweenScale {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub t: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TweenPosition {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub duration: f32,
    pub t: f32,
}

impl TweenPosition {
    pub fn new(from: (f32, f32), to: (f32, f32), duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            t: 0.0,
        }
    }
    pub fn current(&self) -> (f32, f32) {
        if self.duration <= 0.0 {
            return self.to;
        }
        let k = (self.t / self.duration).clamp(0.0, 1.0);
        (
            self.from.0 + (self.to.0 - self.from.0) * k,
            self.from.1 + (self.to.1 - self.from.1) * k,
        )
    }
    pub fn step(&mut self, dt: f32) {
        self.t += dt;
    }
    pub fn finished(&self) -> bool {
        self.t >= self.duration
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TweenAlpha {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub t: f32,
}

impl TweenAlpha {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            t: 0.0,
        }
    }
    pub fn current(&self) -> f32 {
        if self.duration <= 0.0 {
            self.to
        } else {
            self.from + (self.to - self.from) * (self.t / self.duration).clamp(0.0, 1.0)
        }
    }
    pub fn step(&mut self, dt: f32) {
        self.t += dt;
    }
    pub fn finished(&self) -> bool {
        self.t >= self.duration
    }
}

// Easing
pub enum Ease {
    Linear,
    QuadIn,
    QuadOut,
    CubicIn,
    CubicOut,
}

pub fn ease_value(e: Ease, x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    match e {
        Ease::Linear => t,
        Ease::QuadIn => t * t,
        Ease::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
        Ease::CubicIn => t * t * t,
        Ease::CubicOut => 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t),
    }
}

// Timeline: advance multiple tweens in sequence
#[derive(Default)]
pub struct Timeline {
    pub duration: f32,
    pub t: f32,
    pub loops: bool,
}

impl Timeline {
    pub fn new(duration: f32) -> Self {
        Self {
            duration,
            t: 0.0,
            loops: false,
        }
    }
    pub fn step(&mut self, dt: f32) {
        self.t += dt;
        if self.loops && self.duration > 0.0 {
            self.t %= self.duration;
        }
    }
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.t / self.duration).clamp(0.0, 1.0)
        }
    }
}

impl TweenScale {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            t: 0.0,
        }
    }
    pub fn current(&self) -> f32 {
        if self.duration <= 0.0 {
            self.to
        } else {
            self.from + (self.to - self.from) * (self.t / self.duration).clamp(0.0, 1.0)
        }
    }
    pub fn step(&mut self, dt: f32) {
        self.t += dt;
    }
    pub fn finished(&self) -> bool {
        self.t >= self.duration
    }
}

// Events: type-indexed Vec<T>
impl World {
    pub fn send_event<T: 'static + Send + Sync>(&mut self, event: T) {
        let key = event_type_key::<T>();
        let entry = self
            .resources
            .entry(key)
            .or_insert_with(|| Box::new(Vec::<T>::new()));
        if let Some(vec) = entry.downcast_mut::<Vec<T>>() {
            vec.push(event);
        }
    }

    pub fn drain_events<T: 'static + Send + Sync>(&mut self) -> Vec<T> {
        let key = event_type_key::<T>();
        if let Some(entry) = self.resources.get_mut(key) {
            if let Some(vec) = entry.downcast_mut::<Vec<T>>() {
                return std::mem::take(vec);
            }
        }
        Vec::new()
    }
}

fn event_type_key<T: 'static>() -> &'static str {
    // Separate namespace from resources by using a distinct key pattern if needed
    std::any::type_name::<Vec<T>>()
}

// Query API for single component type
impl World {
    pub fn query<T: Component>(&self) -> impl Iterator<Item = (Entity, &T)> {
        let maybe_store = self
            .components
            .get(std::any::type_name::<T>())
            .map(|store| store.iter());
        QueryIterRef::<T> {
            inner: maybe_store,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn query_mut<T: Component>(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        let maybe_store = self
            .components
            .get_mut(std::any::type_name::<T>())
            .map(|store| store.iter_mut());
        QueryIterMut::<T> {
            inner: maybe_store,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn query2<A: Component, B: Component>(&self) -> impl Iterator<Item = (Entity, &A, &B)> {
        let a_map_opt = self.components.get(std::any::type_name::<A>());
        let b_map_opt = self.components.get(std::any::type_name::<B>());
        Query2IterRef::<A, B> {
            a_map: a_map_opt,
            b_map: b_map_opt,
            a_iter: a_map_opt.map(|m| m.iter()),
            _phantom_a: std::marker::PhantomData,
            _phantom_b: std::marker::PhantomData,
        }
    }
}

struct QueryIterRef<'a, T: Component> {
    inner: Option<std::collections::hash_map::Iter<'a, u32, Box<dyn std::any::Any + Send + Sync>>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: Component> Iterator for QueryIterRef<'a, T> {
    type Item = (Entity, &'a T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, boxed) = self.inner.as_mut()?.next()?;
            if let Some(r) = boxed.downcast_ref::<T>() {
                return Some((Entity(*id), r));
            }
        }
    }
}

struct QueryIterMut<'a, T: Component> {
    inner:
        Option<std::collections::hash_map::IterMut<'a, u32, Box<dyn std::any::Any + Send + Sync>>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: Component> Iterator for QueryIterMut<'a, T> {
    type Item = (Entity, &'a mut T);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, boxed) = self.inner.as_mut()?.next()?;
            if let Some(r) = boxed.downcast_mut::<T>() {
                return Some((Entity(*id), r));
            }
        }
    }
}

struct Query2IterRef<'a, A: Component, B: Component> {
    #[allow(dead_code)]
    a_map: Option<&'a AHashMap<u32, Box<dyn std::any::Any + Send + Sync>>>,
    b_map: Option<&'a AHashMap<u32, Box<dyn std::any::Any + Send + Sync>>>,
    a_iter: Option<std::collections::hash_map::Iter<'a, u32, Box<dyn std::any::Any + Send + Sync>>>,
    _phantom_a: std::marker::PhantomData<A>,
    _phantom_b: std::marker::PhantomData<B>,
}

impl<'a, A: Component, B: Component> Iterator for Query2IterRef<'a, A, B> {
    type Item = (Entity, &'a A, &'a B);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((id, a_box)) = self.a_iter.as_mut()?.next() {
            if let Some(a_ref) = a_box.downcast_ref::<A>() {
                if let Some(b_box) = self.b_map?.get(id) {
                    if let Some(b_ref) = b_box.downcast_ref::<B>() {
                        return Some((Entity(*id), a_ref, b_ref));
                    }
                }
            }
        }
        None
    }
}

// Fixed step helper (accumulator-based)
#[derive(Debug, Clone, Copy)]
pub struct FixedStep {
    pub step_seconds: f32,
    pub accumulator: f32,
}

impl FixedStep {
    pub fn new(step_seconds: f32) -> Self {
        Self {
            step_seconds,
            accumulator: 0.0,
        }
    }
    pub fn advance(&mut self, delta_seconds: f32, mut on_step: impl FnMut()) {
        self.accumulator += delta_seconds;
        while self.accumulator >= self.step_seconds {
            on_step();
            self.accumulator -= self.step_seconds;
        }
    }
}

// Commands queue for safe world mutations
pub enum CommandOp {
    Spawn(Box<dyn Fn(&mut World) + Send + Sync>),
    Mutate(Box<dyn Fn(&mut World) + Send + Sync>),
}

#[derive(Default)]
pub struct Commands {
    ops: Vec<CommandOp>,
}

impl Commands {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }
    pub fn spawn_with<T: Component + Default + 'static>(mut self) -> Self {
        self.ops.push(CommandOp::Spawn(Box::new(|w: &mut World| {
            let e = w.spawn();
            w.insert_component(e, T::default());
        })));
        self
    }
    pub fn push(mut self, f: impl Fn(&mut World) + Send + Sync + 'static) -> Self {
        self.ops.push(CommandOp::Mutate(Box::new(f)));
        self
    }
    pub fn apply(self, world: &mut World) {
        for op in self.ops {
            match op {
                CommandOp::Spawn(f) | CommandOp::Mutate(f) => (f)(world),
            }
        }
    }
}
