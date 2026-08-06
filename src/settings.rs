use std::mem;

use bevy::prelude::*;

use crate::{
    AppState, WIN_HEIGHT,
    grid::{CellConfigIndicator, CellType, Grid},
    ui::{ActionButton, Focus, Focusable, TextInput},
};

pub fn plugin(app: &mut App) {
    app.add_sub_state::<SettingsState>()
        .add_systems(OnEnter(SettingsState::Parameters), setup_settings)
        .add_systems(OnEnter(SettingsState::Grid), setup_grid_settings)
        .add_systems(
            Update,
            (
                (paint_region, cycle_paint_type).run_if(in_state(SettingsState::Grid)),
                return_to_settings_on_esc,
            ),
        );
}

#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Settings)]
pub enum SettingsState {
    #[default]
    Parameters,
    Grid,
}

#[derive(Resource, Clone, Copy)]
pub struct GridSize(u16);

impl GridSize {
    pub fn get(&self) -> u16 {
        self.0
    }

    pub fn cell_px(&self) -> f32 {
        1000.0 / self.0 as f32
    }
}

fn submit_grid_size(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(GridSize(v));
    true
}

#[derive(Resource)]
pub struct EntityCount(usize);

impl EntityCount {
    pub fn get(&self) -> usize {
        self.0
    }
}

fn submit_entity_count(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(EntityCount(v));
    true
}

#[derive(Resource)]
pub struct TicksPerGen(u32);

impl TicksPerGen {
    pub fn get(&self) -> u32 {
        self.0
    }
}

fn submit_ticks_per_gen(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(TicksPerGen(v));
    true
}

#[derive(Resource, Clone)]
pub struct HiddenLayers(pub Vec<usize>);

impl HiddenLayers {
    fn text(&self) -> String {
        let mut s = String::new();
        if self.0.is_empty() {
            return s;
        }
        s += &format!("{}", self.0[0]);
        for v in self.0.iter().skip(1) {
            s += &format!(", {v}");
        }
        s
    }
}

fn submit_hidden_layers(text: &str, mut commands: Commands) -> bool {
    let mut vec = vec![];
    for s in text.split(',') {
        match s.trim().parse() {
            Ok(v) => vec.push(v),
            _ => return false,
        }
    }
    commands.insert_resource(HiddenLayers(vec));
    true
}

#[derive(Resource)]
pub struct PreferClosePartners(bool);

impl PreferClosePartners {
    pub fn get(&self) -> bool {
        self.0
    }
}

fn submit_prefer_close_partners(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(PreferClosePartners(v));
    true
}

#[derive(Resource)]
struct CurrentCellType(CellType);

#[derive(Component)]
struct PaintInstructionLabel;

impl PaintInstructionLabel {
    fn text(cell_type: CellType) -> String {
        match cell_type {
            CellType::Normal => "Paint normal cells (eraser)",
            CellType::Safe => "Paint survival region",
            CellType::Wall => "Paint walls",
        }
        .into()
    }
}

fn continue_to_grid_settings(mut commands: Commands) {
    commands.set_state(SettingsState::Grid);
}

fn start_sim(mut commands: Commands) {
    commands.set_state(AppState::Sim);
}

fn setup_settings(mut commands: Commands) {
    let default_grid_size = 50;
    let default_entity_count = 100;
    let default_ticks_per_gen = 100;
    let default_hidden_layers = HiddenLayers(vec![6]);
    let default_prefer_close_partners = true;
    commands.insert_resource(GridSize(default_grid_size));
    commands.insert_resource(EntityCount(default_entity_count));
    commands.insert_resource(TicksPerGen(default_ticks_per_gen));
    commands.insert_resource(default_hidden_layers.clone());
    commands.insert_resource(PreferClosePartners(default_prefer_close_partners));

    commands.spawn((
        Text2d::new("Settings"),
        TextFont {
            font_size: 50.0,
            ..default()
        },
        Transform::from_xyz(0.0, WIN_HEIGHT / 2.0 - 30.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Text2d::new("Grid size: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-300.0, WIN_HEIGHT / 2.0 - 100.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focus,
        Focusable { order: 0 },
        TextInput {
            on_submit: submit_grid_size,
        },
        Text2d(format!("{default_grid_size}")),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(200.0, WIN_HEIGHT / 2.0 - 100.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Text2d::new("Entity count: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-300.0, WIN_HEIGHT / 2.0 - 150.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focusable { order: 1 },
        TextInput {
            on_submit: submit_entity_count,
        },
        Text2d(format!("{default_entity_count}")),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(200.0, WIN_HEIGHT / 2.0 - 150.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Text2d::new("Ticks per generation: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-400.0, WIN_HEIGHT / 2.0 - 200.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focusable { order: 2 },
        TextInput {
            on_submit: submit_ticks_per_gen,
        },
        Text2d(format!("{default_ticks_per_gen}")),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(200.0, WIN_HEIGHT / 2.0 - 200.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Text2d::new("Hidden layers (CSV): "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-400.0, WIN_HEIGHT / 2.0 - 250.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focusable { order: 3 },
        TextInput {
            on_submit: submit_hidden_layers,
        },
        Text2d(default_hidden_layers.text()),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(200.0, WIN_HEIGHT / 2.0 - 250.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Text2d::new("Prefer close partners: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-400.0, WIN_HEIGHT / 2.0 - 300.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focusable { order: 4 },
        TextInput {
            on_submit: submit_prefer_close_partners,
        },
        Text2d(format!("{default_prefer_close_partners}")),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(200.0, WIN_HEIGHT / 2.0 - 300.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));

    commands.spawn((
        Focusable { order: 5 },
        ActionButton {
            on_press: continue_to_grid_settings,
        },
        Text2d::new("Continue"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(0.0, WIN_HEIGHT / 2.0 - 370.0, 2.0),
        DespawnOnExit(SettingsState::Parameters),
    ));
}

fn setup_grid_settings(mut commands: Commands) {
    commands.insert_resource(CurrentCellType(CellType::Safe));

    commands.spawn((
        Text2d::new("Settings"),
        TextFont {
            font_size: 50.0,
            ..default()
        },
        Transform::from_xyz(350.0, WIN_HEIGHT / 2.0 - 30.0, 2.0),
        DespawnOnExit(SettingsState::Grid),
    ));

    commands.spawn((
        PaintInstructionLabel,
        Text2d(PaintInstructionLabel::text(CellType::Safe)),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(450.0, WIN_HEIGHT / 2.0 - 90.0, 2.0),
        DespawnOnExit(SettingsState::Grid),
    ));

    commands.spawn((
        Text2d::new("Press space to change what you're painting."),
        TextFont {
            font_size: 30.0,
            ..default()
        },
        Transform::from_xyz(500.0, WIN_HEIGHT / 2.0 - 150.0, 2.0),
        DespawnOnExit(SettingsState::Grid),
    ));

    commands.spawn((
        Focus,
        Focusable { order: 0 },
        ActionButton {
            on_press: start_sim,
        },
        Text2d::new("Start"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(350.0, WIN_HEIGHT / 2.0 - 300.0, 2.0),
        DespawnOnExit(SettingsState::Grid),
    ));
}

fn cycle_paint_type(
    mut current_cell_type: ResMut<CurrentCellType>,
    kb: Res<ButtonInput<KeyCode>>,
    mut text: Single<&mut Text2d, With<PaintInstructionLabel>>,
) {
    if !kb.just_pressed(KeyCode::Space) {
        return;
    };
    let new_type = match current_cell_type.0 {
        CellType::Normal => CellType::Safe,
        CellType::Safe => CellType::Wall,
        CellType::Wall => CellType::Normal,
    };
    current_cell_type.0 = new_type;
    text.0 = PaintInstructionLabel::text(new_type);
}

fn paint_region(
    cell_type: Res<CurrentCellType>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    mut grid: ResMut<Grid>,
    grid_size: Res<GridSize>,
    mut drag_start: Local<(u16, u16)>,
    mut commands: Commands,
) {
    let Some(cursor) = window
        .cursor_position()
        .map(|pos| Vec2::new(pos.x - window.width() / 2.0, -pos.y + window.height() / 2.0))
    else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        *drag_start = Grid::grid_pos_from_world_pos(cursor, grid_size.cell_px());
    } else if mouse.just_released(MouseButton::Left) {
        let mut drag_end = Grid::grid_pos_from_world_pos(cursor, grid_size.cell_px());
        if drag_start.0 > drag_end.0 {
            mem::swap(&mut drag_start.0, &mut drag_end.0);
        }
        if drag_start.1 > drag_end.1 {
            mem::swap(&mut drag_start.1, &mut drag_end.1);
        }
        for x in drag_start.0..=drag_end.0 {
            for y in drag_start.1..=drag_end.1 {
                if x >= grid_size.0 || y >= grid_size.0 {
                    return;
                }
                let i = grid.idx_from_pos(x, y);
                if cell_type.0 == grid[i].cell_type() {
                    continue;
                }
                match cell_type.0 {
                    CellType::Normal => {
                        CellConfigIndicator::despawn(x, y, &grid, commands.reborrow());
                        grid[i].set_cell_type(CellType::Normal);
                    }
                    CellType::Safe | CellType::Wall => {
                        CellConfigIndicator::despawn(x, y, &grid, commands.reborrow());
                        grid[i].set_cell_type(cell_type.0);
                        commands.run_system_cached_with(
                            CellConfigIndicator::spawn,
                            (x, y, cell_type.0),
                        );
                    }
                }
            }
        }
    }
}

fn return_to_settings_on_esc(
    kb: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppState>>,
    settings_state: Option<Res<State<SettingsState>>>,
    mut commands: Commands,
) {
    if (*app_state.get() != AppState::Settings
        || *settings_state.unwrap().get() == SettingsState::Grid)
        && kb.pressed(KeyCode::Escape)
    {
        commands.set_state(AppState::Settings);
        commands.set_state(SettingsState::Parameters);
    }
}
