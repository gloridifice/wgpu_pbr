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
    pub fps: f32,
    /// 平滑后的每帧毫秒数（指数平滑 EMA）
    pub frame_time_ms: f32,
    frame_count: u32,
    fps_accumulated_time: Duration,
}

impl Time {
    const SMOOTH_ALPHA: f32 = 0.01;

    pub fn update(&mut self) {
        let now = Instant::now();

        self.delta_time = match self.last_time.as_ref() {
            Some(instant) => now - *instant,
            None => Duration::from_secs_f32(0.0001),
        };
        self.last_time = Some(now);

        let current_ms = self.delta_time.as_secs_f32() * 1000.0;
        if self.frame_time_ms == 0.0 {
            self.frame_time_ms = current_ms;
        } else {
            self.frame_time_ms =
                Self::SMOOTH_ALPHA * current_ms + (1.0 - Self::SMOOTH_ALPHA) * self.frame_time_ms;
        }

        self.frame_count += 1;
        self.fps_accumulated_time += self.delta_time;

        if self.fps_accumulated_time >= Duration::from_secs_f32(0.5) {
            self.fps = self.frame_count as f32 / self.fps_accumulated_time.as_secs_f32();
            self.frame_count = 0;
            self.fps_accumulated_time = Duration::ZERO;
        }
    }
}
