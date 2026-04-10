use bevy::{camera::ScalingMode, prelude::*};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run()
}

const WIN_WIDTH: f32 = 1920.0;
const WIN_HEIGHT: f32 = 1080.0;
const GRID_SIZE: u16 = 50;
const CELL_PX: f32 = 20.0;
const LINE_WIDTH: f32 = 3.0;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: WIN_WIDTH,
                min_height: WIN_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    let square = meshes.add(Rectangle::default());
    let black = materials.add(Color::BLACK);
    let square_clone = square.clone();
    let black_clone = black.clone();
    commands.spawn_batch((0..GRID_SIZE + 1).map(move |i| {
        let square = square_clone.clone();
        let black = black_clone.clone();
        (
            Mesh2d(square),
            MeshMaterial2d(black),
            Transform {
                translation: Vec3::new(
                    i as f32 * CELL_PX - WIN_WIDTH / 2.0,
                    (WIN_HEIGHT - GRID_SIZE as f32 * CELL_PX) / 2.0,
                    0.0,
                ),
                scale: Vec3::new(LINE_WIDTH, GRID_SIZE as f32 * CELL_PX, 1.0),
                ..default()
            },
        )
    }));
    commands.spawn_batch((0..GRID_SIZE + 1).map(move |i| {
        (
            Mesh2d(square.clone()),
            MeshMaterial2d(black.clone()),
            Transform {
                translation: Vec3::new(
                    (WIN_WIDTH - GRID_SIZE as f32 * CELL_PX) / -2.0,
                    WIN_HEIGHT / 2.0 - i as f32 * CELL_PX,
                    0.0,
                ),
                scale: Vec3::new(GRID_SIZE as f32 * CELL_PX, LINE_WIDTH, 1.0),
                ..default()
            },
        )
    }));
}
