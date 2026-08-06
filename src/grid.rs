use std::ops::{Index, IndexMut};

use bevy::prelude::*;

use crate::{
    AppState, WIN_HEIGHT, WIN_WIDTH,
    net::Net,
    settings::{GridSize, SettingsState},
};

pub fn plugin(app: &mut App) {
    app.add_computed_state::<GridState>()
        .add_systems(OnEnter(GridState), spawn);
}

const LINE_WIDTH: f32 = 3.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GridState;

impl ComputedStates for GridState {
    type SourceStates = (AppState, Option<SettingsState>);

    fn compute((app_state, settings_state): Self::SourceStates) -> Option<Self> {
        match app_state {
            AppState::Settings => {
                if settings_state.unwrap() == SettingsState::Grid {
                    Some(Self)
                } else {
                    None
                }
            }
            AppState::Sim => Some(Self),
        }
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    grid_size: Res<GridSize>,
) {
    let square = meshes.add(Rectangle::default());
    commands.insert_resource(Square(square.clone()));

    commands.insert_resource(CellEntityMaterial(
        materials.add(Color::srgb(0.8, 0.2, 0.2)),
    ));
    commands.insert_resource(SafeIndicatorMaterial(
        materials.add(Color::srgb(0.0, 0.2, 0.0)),
    ));
    commands.insert_resource(WallIndicatorMaterial(
        materials.add(Color::srgb(0.5, 0.5, 0.5)),
    ));

    let grid_size = *grid_size;

    let black = materials.add(Color::BLACK);
    let square_clone = square.clone();
    let black_clone = black.clone();
    commands.spawn_batch((0..grid_size.get() + 1).map(move |i| {
        let square = square_clone.clone();
        let black = black_clone.clone();
        (
            Mesh2d(square),
            MeshMaterial2d(black),
            Transform {
                translation: Vec3::new(
                    i as f32 * grid_size.cell_px() - WIN_WIDTH / 2.0,
                    (WIN_HEIGHT - grid_size.get() as f32 * grid_size.cell_px()) / 2.0,
                    1.0,
                ),
                scale: Vec3::new(
                    LINE_WIDTH,
                    grid_size.get() as f32 * grid_size.cell_px(),
                    1.0,
                ),
                ..default()
            },
            DespawnOnExit(GridState),
        )
    }));
    commands.spawn_batch((0..grid_size.get() + 1).map(move |i| {
        let square = square.clone();
        (
            Mesh2d(square),
            MeshMaterial2d(black.clone()),
            Transform {
                translation: Vec3::new(
                    (WIN_WIDTH - grid_size.get() as f32 * grid_size.cell_px()) / -2.0,
                    WIN_HEIGHT / 2.0 - i as f32 * grid_size.cell_px(),
                    1.0,
                ),
                scale: Vec3::new(
                    grid_size.get() as f32 * grid_size.cell_px(),
                    LINE_WIDTH,
                    1.0,
                ),
                ..default()
            },
            DespawnOnExit(GridState),
        )
    }));

    commands.insert_resource(Grid::new(grid_size.get()));
}

#[derive(Resource, Clone, Debug)]
pub struct Grid {
    size: u16,
    data: Box<[Cell]>,
}

impl Index<usize> for Grid {
    type Output = Cell;

    fn index(&self, i: usize) -> &Self::Output {
        &self.data[i]
    }
}

impl IndexMut<usize> for Grid {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.data[i]
    }
}

impl Grid {
    fn new(size: u16) -> Self {
        let mut data = Box::new_uninit_slice(size as usize * size as usize);
        for cell in &mut data {
            cell.write(Cell::Normal { cell_entity: None });
        }
        Self {
            size,
            data: unsafe { data.assume_init() },
        }
    }

    pub fn clear_entities(&mut self) {
        for cell in &mut self.data {
            cell.set_cell_entity(None);
        }
    }

    pub fn size(&self) -> u16 {
        self.size
    }

    pub fn get(&self, x: u16, y: u16) -> Option<Cell> {
        self.data
            .get(x as usize + y as usize * self.size as usize)
            .cloned()
    }

    pub fn idx_from_pos(&self, x: u16, y: u16) -> usize {
        x as usize + y as usize * self.size as usize
    }

    fn world_pos_from_grid_pos(x: u16, y: u16, cell_px: f32) -> Vec2 {
        Vec2::new(
            (x as f32 + 0.5) * cell_px - WIN_WIDTH / 2.0,
            WIN_HEIGHT / 2.0 - (y as f32 + 0.5) * cell_px,
        )
    }

    pub fn grid_pos_from_world_pos(pos: Vec2, cell_px: f32) -> (u16, u16) {
        (
            ((pos.x + WIN_WIDTH / 2.0) / cell_px - 0.5).round() as u16,
            ((pos.y - WIN_HEIGHT / 2.0) / -cell_px - 0.5).round() as u16,
        )
    }

    pub fn move_cell_entity(&mut self, x: u16, y: u16, dir: Dir) -> bool {
        let Some((tx, ty)) = dir.apply(x, y, self.size) else {
            return false;
        };
        let source = self.idx_from_pos(x, y);
        let target = self.idx_from_pos(tx, ty);
        self.data[target].set_cell_entity(self.data[source].cell_entity());
        self.data[source].set_cell_entity(None);
        true
    }
}

pub const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

#[derive(Clone, Copy, Debug)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub fn apply(self, x: u16, y: u16, grid_size: u16) -> Option<(u16, u16)> {
        match self {
            Self::Up => {
                if y > 0 {
                    Some((x, y - 1))
                } else {
                    None
                }
            }
            Self::Down => {
                if y < grid_size - 1 {
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
                if x < grid_size - 1 {
                    Some((x + 1, y))
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cell {
    Normal {
        cell_entity: Option<Entity>,
    },
    Safe {
        cell_entity: Option<Entity>,
        indicator: Entity,
    },
    Wall {
        indicator: Entity,
    },
}

impl Cell {
    pub fn cell_type(&self) -> CellType {
        match *self {
            Cell::Normal { .. } => CellType::Normal,
            Cell::Safe { .. } => CellType::Safe,
            Cell::Wall { .. } => CellType::Wall,
        }
    }

    /// Changes the `Cell` variant, without spawning or despawning cell indicators.
    /// `indicator` fields are filled with `Entity::PLACEHOLDER`.
    pub fn set_cell_type(&mut self, cell_type: CellType) {
        if self.cell_type() == cell_type {
            return;
        }
        *self = match cell_type {
            CellType::Normal => Cell::Normal {
                cell_entity: match *self {
                    Cell::Normal { .. } => unreachable!(),
                    Cell::Safe { cell_entity, .. } => cell_entity,
                    Cell::Wall { .. } => None,
                },
            },
            CellType::Safe => Cell::Safe {
                cell_entity: match *self {
                    Cell::Normal { cell_entity } => cell_entity,
                    Cell::Safe { .. } => unreachable!(),
                    Cell::Wall { .. } => None,
                },
                indicator: Entity::PLACEHOLDER,
            },
            CellType::Wall => Cell::Wall {
                indicator: Entity::PLACEHOLDER,
            },
        };
    }

    fn cell_entity(&self) -> Option<Entity> {
        match *self {
            Self::Normal { cell_entity } => cell_entity,
            Self::Safe { cell_entity, .. } => cell_entity,
            Self::Wall { .. } => None,
        }
    }

    fn set_cell_entity(&mut self, cell_entity: Option<Entity>) {
        *self = match *self {
            Self::Normal { .. } => Self::Normal { cell_entity },
            Self::Safe { indicator, .. } => Self::Safe {
                cell_entity,
                indicator,
            },
            Self::Wall { indicator } => Self::Wall { indicator },
        };
    }

    fn indicator(&self) -> Option<Entity> {
        match *self {
            Cell::Normal { .. } => None,
            Cell::Safe { indicator, .. } => Some(indicator),
            Cell::Wall { indicator } => Some(indicator),
        }
    }

    fn set_indicator(&mut self, indicator: Entity) {
        *self = match *self {
            Self::Normal { cell_entity } => Self::Normal { cell_entity },
            Self::Safe { cell_entity, .. } => Self::Safe {
                cell_entity,
                indicator,
            },
            Self::Wall { .. } => Self::Wall { indicator },
        };
    }

    pub fn is_free(&self) -> bool {
        self.cell_entity().is_none() && !matches!(self, Self::Wall { .. })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellType {
    Normal,
    Safe,
    Wall,
}

#[derive(Component, Clone, Debug)]
pub struct CellEntity {
    pub x: u16,
    pub y: u16,
    pub net: Net,
}

impl CellEntity {
    pub fn spawn(
        In(ce): In<Self>,
        mut commands: Commands,
        square: Res<Square>,
        material: Res<CellEntityMaterial>,
        mut grid: ResMut<Grid>,
        grid_size: Res<GridSize>,
    ) {
        let (x, y) = (ce.x, ce.y);
        let id = commands
            .spawn((
                ce,
                Mesh2d(square.0.clone()),
                MeshMaterial2d(material.0.clone()),
                Transform {
                    translation: Grid::world_pos_from_grid_pos(x, y, grid_size.cell_px())
                        .extend(0.0),
                    scale: Vec3::new(grid_size.cell_px(), grid_size.cell_px(), 1.0),
                    ..default()
                },
                DespawnOnExit(AppState::Sim),
            ))
            .id();
        grid.data[x as usize + y as usize * grid_size.get() as usize].set_cell_entity(Some(id));
    }

    pub fn update_tf(
        mut q: Query<(&mut Transform, &Self), Changed<Self>>,
        grid_size: Res<GridSize>,
    ) {
        for (mut tf, ce) in &mut q {
            let pos = Grid::world_pos_from_grid_pos(ce.x, ce.y, grid_size.cell_px());
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
        }
    }

    pub fn cell(&self, grid: &Grid) -> Cell {
        grid.get(self.x, self.y).unwrap()
    }
}

#[derive(Resource, Debug)]
pub struct Square(Handle<Mesh>);

#[derive(Resource, Debug)]
pub struct CellEntityMaterial(Handle<ColorMaterial>);

#[derive(Resource)]
pub struct SafeIndicatorMaterial(Handle<ColorMaterial>);

#[derive(Resource)]
pub struct WallIndicatorMaterial(Handle<ColorMaterial>);

#[derive(Component)]
pub struct CellConfigIndicator;

impl CellConfigIndicator {
    /// Cell type has to be updated *before* calling this system.
    pub fn spawn(
        input: In<(u16, u16, CellType)>,
        mut commands: Commands,
        square: Res<Square>,
        safe_material: Res<SafeIndicatorMaterial>,
        wall_material: Res<WallIndicatorMaterial>,
        mut grid: ResMut<Grid>,
        grid_size: Res<GridSize>,
    ) {
        let (x, y, cell_type) = *input;
        let i = grid.idx_from_pos(x, y);
        let material = match cell_type {
            CellType::Safe => safe_material.0.clone(),
            CellType::Wall => wall_material.0.clone(),
            _ => return,
        };
        let e = commands
            .spawn((
                Self,
                Mesh2d(square.0.clone()),
                MeshMaterial2d(material),
                Transform {
                    translation: Grid::world_pos_from_grid_pos(x, y, grid_size.cell_px())
                        .extend(-1.0),
                    scale: Vec3::new(grid_size.cell_px(), grid_size.cell_px(), 1.0),
                    ..default()
                },
                DespawnOnExit(GridState),
            ))
            .id();
        grid.data[i].set_indicator(e);
    }

    /// Cell type has to be updated *after* calling this system.
    pub fn despawn(x: u16, y: u16, grid: &Grid, mut commands: Commands) {
        let i = grid.idx_from_pos(x, y);
        let Some(e) = &mut grid.data[i].indicator() else {
            return;
        };
        commands.entity(*e).despawn();
        *e = Entity::PLACEHOLDER;
    }
}

#[cfg(test)]
mod tests {
    use super::Grid;

    #[test]
    fn pos() {
        assert_eq!(
            (10, 5),
            Grid::grid_pos_from_world_pos(Grid::world_pos_from_grid_pos(10, 5, 10.0), 10.0)
        );
        assert_eq!(
            (6, 8),
            Grid::grid_pos_from_world_pos(Grid::world_pos_from_grid_pos(6, 8, 5.3), 5.3)
        );
        assert_eq!(
            (3, 2),
            Grid::grid_pos_from_world_pos(Grid::world_pos_from_grid_pos(3, 2, 7.6), 7.6)
        );
    }
}
