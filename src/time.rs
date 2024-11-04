use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref TIME: Mutex<GameTime> = Mutex::new(GameTime::default());
}

#[derive(Default)]
pub struct GameTime {
    pub last_time: Option<Instant>,
    pub delta_time: Duration,
}

impl GameTime {
    pub fn update(&mut self) {
        let now = Instant::now();

        self.delta_time = match self.last_time.as_ref() {
            Some(instant) => now - *instant,
            None => Duration::from_secs_f32(0.0001),
        };
        self.last_time = Some(now);
    }
}
