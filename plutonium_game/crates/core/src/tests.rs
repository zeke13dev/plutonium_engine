#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_insert_and_query() {
        #[derive(Clone, Copy)]
        struct Foo(u32);
        let mut w = World::new();
        let e1 = w.spawn();
        let e2 = w.spawn();
        w.insert_component(e1, Foo(1));
        w.insert_component(e2, Foo(2));
        let sum: u32 = w.query::<Foo>().map(|(_, f)| f.0).sum();
        assert_eq!(sum, 3);
    }

    #[test]
    fn events_send_and_drain() {
        #[derive(Clone)]
        struct Evt(i32);
        let mut w = World::new();
        w.send_event(Evt(1));
        w.send_event(Evt(2));
        let drained = w.drain_events::<Evt>();
        assert_eq!(drained.len(), 2);
        assert!(w.drain_events::<Evt>().is_empty());
    }

    #[test]
    fn tween_scale_advances() {
        let mut tw = TweenScale::new(1.0, 2.0, 1.0);
        assert!((tw.current() - 1.0).abs() < 1e-6);
        tw.step(0.5);
        assert!((tw.current() - 1.5).abs() < 1e-3);
        tw.step(0.5);
        assert!(tw.finished());
        assert!((tw.current() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn query2_and_remove() {
        #[derive(Clone, Copy)]
        struct A(u8);
        #[derive(Clone, Copy)]
        struct B(u8);
        let mut w = World::new();
        let e1 = w.spawn();
        let e2 = w.spawn();
        w.insert_component(e1, A(1));
        w.insert_component(e1, B(2));
        w.insert_component(e2, A(3));
        let pairs: Vec<(u32, u8, u8)> = w.query2::<A, B>().map(|(e, a, b)| (e.0, a.0, b.0)).collect();
        assert_eq!(pairs, vec![(e1.0, 1, 2)]);
        w.remove_component::<A>(e1);
        assert!(w.query2::<A, B>().next().is_none());
        w.despawn(e2);
        assert!(w.get_component::<A>(e2).is_none());
    }

    #[test]
    fn fixed_step_runs_expected_times() {
        let mut fs = crate::FixedStep::new(0.1);
        let mut count = 0;
        fs.advance(0.05, || count += 1);
        assert_eq!(count, 0);
        fs.advance(0.05, || count += 1);
        assert_eq!(count, 1);
        fs.advance(0.35, || count += 1);
        assert_eq!(count, 4);
    }

    #[test]
    fn scene_stack_push_pop_replace() {
        let mut w = World::new();
        crate::scene_push(&mut w, "Menu");
        assert_eq!(w.get_resource::<crate::SceneStack>().unwrap().top(), Some("Menu"));
        crate::scene_push(&mut w, "Game");
        assert_eq!(w.get_resource::<crate::SceneStack>().unwrap().top(), Some("Game"));
        crate::scene_pop(&mut w);
        assert_eq!(w.get_resource::<crate::SceneStack>().unwrap().top(), Some("Menu"));
        crate::scene_replace(&mut w, "Pause");
        assert_eq!(w.get_resource::<crate::SceneStack>().unwrap().top(), Some("Pause"));
        // events present
        let enters = w.drain_events::<crate::SceneEnter>();
        let exits = w.drain_events::<crate::SceneExit>();
        assert!(!enters.is_empty());
        assert!(!exits.is_empty());
    }

    #[test]
    fn easing_and_timeline() {
        use crate::{Ease, ease_value, Timeline};
        assert!((ease_value(Ease::Linear, 0.5) - 0.5).abs() < 1e-6);
        assert!(ease_value(Ease::QuadIn, 0.5) < 0.5);
        assert!(ease_value(Ease::QuadOut, 0.5) > 0.5);
        let mut tl = Timeline::new(2.0);
        tl.step(0.5);
        assert!((tl.progress() - 0.25).abs() < 1e-6);
        tl.loops = true;
        tl.step(2.0);
        assert!((tl.progress() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn scene_helpers_run_startup_and_update() {
        use crate::{SceneSystems, process_scene_events, run_current_scene_update, scene_push};
        let mut w = World::new();
        // Register a startup + update schedule for scene "Menu"
        let mut systems = SceneSystems::default();
        let mut hit: i32 = 0;
        systems.register_startup("Menu", Schedule::new().with_system(|world| {
            world.insert_resource::<i32>(1);
        }));
        systems.register_update("Menu", Schedule::new().with_system(|world| {
            if let Some(x) = world.get_resource_mut::<i32>() { *x += 1; }
        }));
        w.insert_resource(systems);
        // Push scene and process events
        scene_push(&mut w, "Menu");
        process_scene_events(&mut w);
        assert_eq!(*w.get_resource::<i32>().unwrap(), 1);
        // Top of stack is Menu; run update
        run_current_scene_update(&mut w);
        assert_eq!(*w.get_resource::<i32>().unwrap(), 2);
        let _ = hit; // silence unused
    }
}


