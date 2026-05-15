use std::time::Duration;

use bevy::{
    camera::ScalingMode, platform::collections::HashSet, prelude::*,
    time::common_conditions::on_timer,
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (move_right, CellEntity::update_tf)
                .chain()
                .run_if(on_timer(Duration::from_secs_f32(0.1))),
        )
        .run()
}

const WIN_WIDTH: f32 = 1920.0;
const WIN_HEIGHT: f32 = 1080.0;
const GRID_SIZE: u16 = 50;
const CELL_PX: f32 = 20.0;
const LINE_WIDTH: f32 = 3.0;
const ENTITY_COUNT: usize = 100;

#[derive(Resource)]
struct Grid([Cell; GRID_SIZE as usize * GRID_SIZE as usize]);

impl Grid {
    fn get(&self, x: u16, y: u16) -> Option<Cell> {
        self.0
            .get(x as usize + y as usize * GRID_SIZE as usize)
            .cloned()
    }

    fn pos_from_idx(i: usize) -> (u16, u16) {
        (
            (i % GRID_SIZE as usize) as u16,
            (i / GRID_SIZE as usize) as u16,
        )
    }

    fn idx_from_pos(x: u16, y: u16) -> usize {
        x as usize + y as usize * GRID_SIZE as usize
    }

    fn world_pos_from_grid_pos(x: u16, y: u16) -> Vec2 {
        Vec2::new(
            (x as f32 + 0.5) * CELL_PX - WIN_WIDTH / 2.0,
            WIN_HEIGHT / 2.0 - (y as f32 + 0.5) * CELL_PX,
        )
    }

    fn move_cell(&mut self, x: u16, y: u16, dir: Dir) -> bool {
        let Some((tx, ty)) = dir.apply(x, y) else {
            return false;
        };
        let source = Self::idx_from_pos(x, y);
        let target = Self::idx_from_pos(tx, ty);
        self.0[target] = self.0[source];
        self.0[source] = Cell::Empty;
        true
    }
}

enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn apply(self, x: u16, y: u16) -> Option<(u16, u16)> {
        match self {
            Self::Up => {
                if y > 0 {
                    Some((x, y - 1))
                } else {
                    None
                }
            }
            Self::Down => {
                if y < GRID_SIZE - 1 {
                    Some((x, y + 1))
                } else {
                    None
                }
            }
            Self::Left => {
                if x > 0 {
                    Some((x - 1, y))
                } else {
                    None
                }
            }
            Self::Right => {
                if x < GRID_SIZE - 1 {
                    Some((x + 1, y))
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    Entity(Entity),
}

#[derive(Component)]
struct CellEntity(u16, u16);

impl CellEntity {
    fn spawn(
        In((x, y)): In<(u16, u16)>,
        mut commands: Commands,
        square: Res<Square>,
        material: Res<CellEntityMaterial>,
        mut grid: ResMut<Grid>,
    ) {
        let id = commands
            .spawn((
                Self(x, y),
                Mesh2d(square.0.clone()),
                MeshMaterial2d(material.0.clone()),
                Transform {
                    translation: Grid::world_pos_from_grid_pos(x, y).extend(0.0),
                    scale: Vec3::new(CELL_PX, CELL_PX, 1.0),
                    ..default()
                },
            ))
            .id();
        grid.0[x as usize + y as usize * GRID_SIZE as usize] = Cell::Entity(id);
    }

    fn update_tf(mut q: Query<(&mut Transform, &Self), Changed<Self>>) {
        for (mut tf, Self(x, y)) in &mut q {
            let pos = Grid::world_pos_from_grid_pos(*x, *y);
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
        }
    }
}

#[derive(Resource)]
struct Square(Handle<Mesh>);

#[derive(Resource)]
struct CellEntityMaterial(Handle<ColorMaterial>);

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
    commands.insert_resource(Square(square.clone()));

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
                    1.0,
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
                    1.0,
                ),
                scale: Vec3::new(GRID_SIZE as f32 * CELL_PX, LINE_WIDTH, 1.0),
                ..default()
            },
        )
    }));

    commands.insert_resource(CellEntityMaterial(
        materials.add(Color::srgb(0.8, 0.2, 0.2)),
    ));

    commands.insert_resource(Grid([Cell::Empty; _]));

    let mut seed = 1;
    let mut pos = HashSet::with_capacity(ENTITY_COUNT);
    for _ in 0..ENTITY_COUNT {
        let x = rand(&mut seed) as u16 % GRID_SIZE;
        let y = rand(&mut seed) as u16 % GRID_SIZE;
        if pos.insert((x, y)) {
            commands.run_system_cached_with(CellEntity::spawn, (x, y));
        }
    }
}

fn rand(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn move_right(mut q: Query<&mut CellEntity>, mut grid: ResMut<Grid>) {
    for mut ce in &mut q {
        if grid.get(ce.0 + 1, ce.1) == Some(Cell::Empty) && grid.move_cell(ce.0, ce.1, Dir::Right) {
            ce.0 += 1;
        }
    }
}
