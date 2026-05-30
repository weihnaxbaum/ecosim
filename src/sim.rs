use std::time::Instant;

use bevy::{platform::collections::HashSet, prelude::*};

use crate::{
    AppState, MIN_FPS, Rng, WIN_HEIGHT,
    grid::{CellEntity, CellType, DIRS, Grid},
    net::Net,
    settings::{EntityCount, GridSize, HiddenLayers, TicksPerGen},
    ui::{Focus, Focusable, TextInput},
};

pub fn plugin(app: &mut App) {
    app.add_sub_state::<SimState>()
        .add_systems(OnEnter(AppState::Sim), setup_sim)
        .add_systems(Update, run_sim.run_if(in_state(SimState::Running)))
        .add_observer(finish_generation);
}

#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Sim)]
enum SimState {
    #[default]
    Running,
    Stopped,
}

#[derive(Resource)]
struct Tick(u32);

#[derive(Resource)]
struct Generation(u32);

#[derive(Event)]
struct FinishGeneration;

#[derive(Component)]
struct GenerationLabel;

#[derive(Component)]
struct SurvivorsLabel;

#[derive(Component)]
struct DiversityLabel;

impl DiversityLabel {
    fn text(nets: &[&Net]) -> String {
        format!("Diversity: {:.3}", Self::calc(nets))
    }

    // mean of root mean square deviations
    fn calc(nets: &[&Net]) -> f32 {
        let avg_net = Net::avg(nets).flattened_properties();
        let sum = nets
            .iter()
            .map(|n| {
                n.flattened_properties()
                    .iter()
                    .zip(&avg_net)
                    .map(|(v, avg)| (v - avg) * (v - avg))
                    .sum::<f32>()
                    .sqrt()
            })
            .sum::<f32>();
        sum / nets.len() as f32 / (avg_net.len() as f32).sqrt()
    }
}

#[derive(Component)]
struct MutationRateLabel;

impl MutationRateLabel {
    fn text(nets: &[&Net]) -> String {
        format!("Avg mutation rate: {:.3}", Self::calc(nets))
    }

    fn calc(nets: &[&Net]) -> f32 {
        nets.iter().map(|n| n.mutation_rate()).sum::<f32>() / nets.len() as f32
    }
}

/// Ticks per second
#[derive(Resource)]
struct DesiredTps(f32);

fn submit_tps(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(DesiredTps(v));
    true
}

fn setup_sim(
    mut commands: Commands,
    entity_count: Res<EntityCount>,
    grid_size: Res<GridSize>,
    mut hidden_layers: ResMut<HiddenLayers>,
) {
    let mut rng = Rng(1);

    let mut layers = Vec::with_capacity(hidden_layers.0.len() + 2);
    layers.push(8);
    layers.append(&mut hidden_layers.0);
    layers.push(5);
    commands.remove_resource::<HiddenLayers>();

    let mut pos = HashSet::with_capacity(entity_count.get());
    let mut ce = Vec::with_capacity(entity_count.get());
    assert!(entity_count.get() <= grid_size.get() as usize * grid_size.get() as usize);
    while pos.len() < entity_count.get() {
        let x = rng.u64() as u16 % grid_size.get();
        let y = rng.u64() as u16 % grid_size.get();
        if pos.insert((x, y)) {
            let net = Net::random(&layers, &mut rng);
            ce.push(CellEntity { x, y, net });
        }
    }

    commands.insert_resource(rng);
    commands.insert_resource(Tick(0));
    commands.insert_resource(DesiredTps(60.0));
    commands.insert_resource(Generation(0));

    commands.spawn((
        GenerationLabel,
        Text2d::new("Generation 0"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(250.0, WIN_HEIGHT / 2.0 - 30.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    commands.spawn((
        SurvivorsLabel,
        Text2d(format!("Survivors: N/A / {}", entity_count.get())),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(340.0, WIN_HEIGHT / 2.0 - 80.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    let nets: Vec<_> = ce.iter().map(|ce| &ce.net).collect();

    commands.spawn((
        DiversityLabel,
        Text2d(DiversityLabel::text(&nets)),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(300.0, WIN_HEIGHT / 2.0 - 130.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    commands.spawn((
        MutationRateLabel,
        Text2d(MutationRateLabel::text(&nets)),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(400.0, WIN_HEIGHT / 2.0 - 180.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    commands.spawn((
        Text2d::new("Desired TPS:"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(250.0, WIN_HEIGHT / 2.0 - 230.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    commands.spawn((
        Focus,
        Focusable { order: 0 },
        TextInput {
            on_submit: submit_tps,
        },
        Text2d::default(),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(500.0, WIN_HEIGHT / 2.0 - 230.0, 2.0),
        DespawnOnExit(AppState::Sim),
    ));

    ce.into_iter()
        .for_each(|ce| commands.run_system_cached_with(CellEntity::spawn, ce));
}

fn run_sim(world: &mut World, mut time_acc: Local<f32>) -> Result {
    *time_acc += world.resource::<Time>().delta_secs();
    let spt = 1.0 / world.resource::<DesiredTps>().0;
    if *time_acc < spt {
        return Ok(());
    }
    let instant = Instant::now();
    while *time_acc >= spt {
        world.run_system_cached(tick)?;
        if instant.elapsed().as_secs_f32() >= 1.0 / MIN_FPS {
            *time_acc = 0.0;
            break;
        }
        *time_acc -= spt;
    }
    world.run_system_cached(CellEntity::update_tf)?;
    Ok(())
}

fn tick(
    mut ce_q: Query<&mut CellEntity>,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<Rng>,
    mut tick: ResMut<Tick>,
    ticks_per_gen: Res<TicksPerGen>,
    mut commands: Commands,
) {
    if tick.0 >= ticks_per_gen.get() {
        commands.trigger(FinishGeneration);
        return;
    }
    for mut ce in &mut ce_q {
        let x = ce.x as f32 / grid.size() as f32;
        let y = ce.y as f32 / grid.size() as f32;
        let mut inputs = vec![x, y, tick.0 as f32 / ticks_per_gen.get() as f32, rng.f32()];
        for dir in DIRS {
            inputs.push(
                if let Some((x, y)) = dir.apply(ce.x, ce.y, grid.size())
                    && let Some(cell) = grid.get(x, y)
                    && cell.cell_type() == CellType::Empty
                {
                    1.0
                } else {
                    0.0
                },
            );
        }
        ce.net.set_inputs(&inputs);
        ce.net.eval();
        let output = ce.net.output();
        let rand = rng.f32();
        let mut i = -1;
        let mut sum = 0.0;
        while sum < rand && i + 1 < output.len() as i32 {
            i += 1;
            sum += output[i as usize];
        }
        let Some(dir) = DIRS.get(i as usize) else {
            continue;
        };
        if let Some((x, y)) = dir.apply(ce.x, ce.y, grid.size())
            && let Some(cell) = grid.get(x, y)
            && cell.cell_type() == CellType::Empty
        {
            grid.move_cell(ce.x, ce.y, *dir);
            ce.x = x;
            ce.y = y;
        }
    }
    tick.0 += 1;
}

fn finish_generation(
    _: On<FinishGeneration>,
    mut generation: ResMut<Generation>,
    mut gen_label: Single<&mut Text2d, With<GenerationLabel>>,
    mut survivors_label: Single<&mut Text2d, (With<SurvivorsLabel>, Without<GenerationLabel>)>,
    ce_q: Query<(Entity, &CellEntity)>,
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<Rng>,
    mut diversity_label: Single<
        &mut Text2d,
        (
            With<DiversityLabel>,
            Without<GenerationLabel>,
            Without<SurvivorsLabel>,
        ),
    >,
    mut mutation_rate_label: Single<
        &mut Text2d,
        (
            With<MutationRateLabel>,
            Without<GenerationLabel>,
            Without<DiversityLabel>,
            Without<SurvivorsLabel>,
        ),
    >,
    mut tick: ResMut<Tick>,
    entity_count: Res<EntityCount>,
    grid_size: Res<GridSize>,
) {
    generation.0 += 1;
    gen_label.0 = format!("Generation {}", generation.0);

    let mut survivors = vec![];
    for (e, ce) in &ce_q {
        commands.entity(e).despawn();
        if ce.cell(&grid).safe {
            survivors.push(&ce.net);
        }
    }

    survivors_label.0 = format!("Survivors: {} / {}", survivors.len(), entity_count.get());
    if survivors.is_empty() {
        commands.set_state(SimState::Stopped);
        return;
    }

    dbg!(survivors.len());
    dbg!(survivors[0].output());

    let mut pos = HashSet::with_capacity(entity_count.get());
    let mut ce = Vec::with_capacity(entity_count.get());
    while pos.len() < entity_count.get() {
        let x = rng.u64() as u16 % grid_size.get();
        let y = rng.u64() as u16 % grid_size.get();
        if !pos.insert((x, y)) {
            continue;
        }
        let net1 = rng.u64() as usize % survivors.len();
        let net2 = rng.u64() as usize % survivors.len();
        let mut net = survivors[net1].mix(survivors[net2], &mut rng);
        net.mutate(&mut rng);
        ce.push(CellEntity { x, y, net });
    }

    let nets: Vec<_> = ce.iter().map(|ce| &ce.net).collect();
    diversity_label.0 = DiversityLabel::text(&nets);
    mutation_rate_label.0 = MutationRateLabel::text(&nets);

    grid.clear();

    ce.into_iter()
        .for_each(|ce| commands.run_system_cached_with(CellEntity::spawn, ce));

    tick.0 = 0;
}
