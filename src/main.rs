use bevy::prelude::*;

mod grid;
mod net;
mod settings;
mod sim;
mod ui;

fn main() -> AppExit {
    App::new()
        .add_plugins((
            DefaultPlugins,
            grid::plugin,
            settings::plugin,
            sim::plugin,
            ui::plugin,
        ))
        .init_state::<AppState>()
        .add_systems(Startup, spawn_cam)
        .add_systems(Update, update_window_scale_factor)
        .run()
}

const WIN_WIDTH: f32 = 1920.0;
const WIN_HEIGHT: f32 = 1080.0;
const MIN_FPS: f32 = 60.0;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Settings,
    Sim,
}

#[derive(Resource, Clone, Copy, Debug)]
struct Rng(u64);

impl Rng {
    fn u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// 0.0..1.0
    fn f32(&mut self) -> f32 {
        self.u64() as f32 / u64::MAX as f32
    }

    fn bool(&mut self) -> bool {
        self.u64() & 1 == 0
    }
}

fn spawn_cam(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn update_window_scale_factor(mut window: Single<&mut Window>) {
    let scale_factor = (window.physical_width() as f32 / WIN_WIDTH)
        .min(window.physical_height() as f32 / WIN_HEIGHT);
    window.resolution.set_scale_factor(scale_factor);
}
