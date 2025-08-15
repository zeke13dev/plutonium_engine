#![forbid(unsafe_code)]

use plutonium_game_core::World;

// ===== Audio requests/events and systems =====
#[derive(Debug, Clone)]
pub struct AudioRequest {
    pub sfx_path: String,
}

/// System: translate audio requests into actual playback via `Audio` resource
pub fn process_audio_requests(world: &mut World) {
    let reqs = world.drain_events::<AudioRequest>();
    if reqs.is_empty() {
        return;
    }
    if let Some(audio) = world.get_resource::<plutonium_game_audio::Audio>() {
        for r in reqs {
            audio.play_sfx(&r.sfx_path);
        }
    }
}

/// System: when an action is just pressed, emit an `AudioRequest`
pub fn sfx_on_action(world: &mut World, action: &str, sfx_path: &str) {
    if let (Some(input), Some(map)) = (
        world.get_resource::<plutonium_game_input::InputState>(),
        world.get_resource::<plutonium_game_input::ActionMap>(),
    ) {
        if map.action_just_pressed(input, action) {
            world.send_event(AudioRequest {
                sfx_path: sfx_path.to_string(),
            });
        }
    }
}

// ===== Deterministic deck/shuffle/draw using Rng64 =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card(pub u8);

#[derive(Debug, Clone)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new_standard_0_51() -> Self {
        let mut cards = Vec::with_capacity(52);
        for i in 0..52 {
            cards.push(Card(i));
        }
        Self { cards }
    }
    pub fn len(&self) -> usize {
        self.cards.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
    pub fn shuffle(&mut self, rng: &mut plutonium_game_core::Rng64) {
        // Fisher-Yates using Rng64
        let n = self.cards.len();
        if n <= 1 {
            return;
        }
        for i in (1..n).rev() {
            // Map u64 to 0..=i inclusive evenly via modulo (acceptible here for simplicity)
            let j = (rng.next_u64() as usize) % (i + 1);
            self.cards.swap(i, j);
        }
    }
    pub fn draw(&mut self, n: usize) -> Vec<Card> {
        let k = n.min(self.cards.len());
        self.cards.drain(0..k).collect()
    }
    pub fn top(&self) -> Option<Card> {
        self.cards.first().copied()
    }
}

#[derive(Debug, Clone)]
pub struct DealAnim {
    pub positions: Vec<(f32, f32)>,
}

impl DealAnim {
    pub fn linear_grid(
        start_x: f32,
        start_y: f32,
        dx: f32,
        dy: f32,
        cols: usize,
        rows: usize,
    ) -> Self {
        let mut positions = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                positions.push((start_x + c as f32 * dx, start_y + r as f32 * dy));
            }
        }
        Self { positions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plutonium_game_core::World;

    #[test]
    fn audio_request_emitted_on_action() {
        let mut w = World::new();
        // Resources
        w.insert_resource(plutonium_game_input::InputState::default());
        let mut map = plutonium_game_input::ActionMap::default();
        map.bind("click", "Enter");
        w.insert_resource(map);
        // Press Enter this frame
        {
            let mut input = w
                .get_resource_mut::<plutonium_game_input::InputState>()
                .unwrap();
            input.update_from_keys(vec!["Enter".to_string()]);
        }
        sfx_on_action(&mut w, "click", "assets/sfx/click.wav");
        let reqs = w.drain_events::<AudioRequest>();
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].sfx_path.contains("click.wav"));
    }

    #[test]
    fn deterministic_shuffle_with_seed() {
        let mut deck1 = Deck::new_standard_0_51();
        let mut deck2 = Deck::new_standard_0_51();
        let mut rng1 = plutonium_game_core::Rng64::seeded(1234);
        let mut rng2 = plutonium_game_core::Rng64::seeded(1234);
        deck1.shuffle(&mut rng1);
        deck2.shuffle(&mut rng2);
        assert_eq!(deck1.cards, deck2.cards);
        // Draw a few and ensure consistent sequence
        let d1 = deck1.draw(5);
        let d2 = deck2.draw(5);
        assert_eq!(d1, d2);
        assert_eq!(deck1.len(), 52 - 5);
    }
}
