#![forbid(unsafe_code)]

use crate::immediate::types::WidgetId;
use std::any::Any;
use std::collections::HashMap;

/// Persistent widget state cache.
pub struct StateCache {
    states: HashMap<WidgetId, Box<dyn Any>>,
    last_access: HashMap<WidgetId, f32>,
    current_time: f32,
}

impl StateCache {
    pub fn new() -> Self {
        StateCache {
            states: HashMap::new(),
            last_access: HashMap::new(),
            current_time: 0.0,
        }
    }

    /// Get cached state for a widget.
    pub fn get<T: 'static + Clone>(&self, id: WidgetId) -> Option<T> {
        self.states.get(&id)?.downcast_ref::<T>().cloned()
    }

    /// Set cached state for a widget.
    pub fn set<T: 'static + Clone>(&mut self, id: WidgetId, value: T) {
        self.states.insert(id, Box::new(value));
        self.last_access.insert(id, self.current_time);
    }

    /// Update current time (called in begin_frame).
    pub fn set_time(&mut self, time: f32) {
        self.current_time = time;
    }

    /// Remove state not accessed in the last N seconds.
    pub fn cleanup_old_state(&mut self, max_age_seconds: f32) {
        let cutoff_time = self.current_time - max_age_seconds;

        self.states.retain(|id, _| {
            self.last_access
                .get(id)
                .map(|t| *t > cutoff_time)
                .unwrap_or(false)
        });

        self.last_access.retain(|_, time| *time > cutoff_time);
    }

    /// Touch a widget to mark it as accessed this frame.
    pub fn touch(&mut self, id: WidgetId) {
        self.last_access.insert(id, self.current_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_cache_stores_and_retrieves() {
        let mut cache = StateCache::new();
        let id = WidgetId::new(1);

        cache.set(id, 42i32);
        assert_eq!(cache.get::<i32>(id), Some(42));
    }

    #[test]
    fn state_cache_cleans_old_state() {
        let mut cache = StateCache::new();
        let id1 = WidgetId::new(1);
        let id2 = WidgetId::new(2);

        cache.set_time(0.0);
        cache.set(id1, "old".to_string());

        cache.set_time(100.0);
        cache.set(id2, "new".to_string());

        cache.cleanup_old_state(60.0);

        assert_eq!(cache.get::<String>(id1), None);
        assert_eq!(cache.get::<String>(id2), Some("new".to_string()));
    }

    #[test]
    fn state_cache_different_types() {
        let mut cache = StateCache::new();
        let id1 = WidgetId::new(1);
        let id2 = WidgetId::new(2);

        cache.set(id1, 42i32);
        cache.set(id2, "hello".to_string());

        assert_eq!(cache.get::<i32>(id1), Some(42));
        assert_eq!(cache.get::<String>(id2), Some("hello".to_string()));
        assert_eq!(cache.get::<String>(id1), None);
    }
}
