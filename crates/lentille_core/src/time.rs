use std::time::{Duration, Instant};

use bevy_app::{Plugin, PreUpdate};
use bevy_ecs::{prelude::Resource, system::ResMut};

pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Time>()
            .add_systems(PreUpdate, sys_update_time);
    }
}

fn sys_update_time(mut time: ResMut<Time>) {
    time.update();
}

#[derive(Default, Resource, Clone)]
pub struct Time {
    pub last_time: Option<Instant>,
    pub delta_time: Duration,
}

impl Time {
    pub fn update(&mut self) {
        let now = Instant::now();

        self.delta_time = match self.last_time.as_ref() {
            Some(instant) => now - *instant,
            None => Duration::from_secs_f32(0.0001),
        };
        self.last_time = Some(now);
    }
}
