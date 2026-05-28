use std::time::Instant;

use bevy::{
    camera::ScalingMode,
    input::{ButtonState, keyboard::KeyboardInput},
    platform::collections::HashSet,
    prelude::*,
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(AppState::Settings), setup_settings)
        .add_systems(OnEnter(AppState::Sim), setup_sim)
        .add_systems(
            Update,
            (
                run_sim.run_if(in_state(AppState::Sim)),
                get_btn_input,
                get_text_input,
            ),
        )
        .add_observer(finish_generation)
        .run()
}

const WIN_WIDTH: f32 = 1920.0;
const WIN_HEIGHT: f32 = 1080.0;
const LINE_WIDTH: f32 = 3.0;
const MIN_FPS: f32 = 60.0;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    Settings,
    Sim,
}

#[derive(Resource, Clone, Debug)]
struct Grid {
    size: u16,
    data: Box<[Cell]>,
}

impl Grid {
    fn new(size: u16) -> Self {
        let mut data = Box::new_uninit_slice(size as usize * size as usize);
        for cell in &mut data {
            cell.write(Cell::Empty);
        }
        Self {
            size,
            data: unsafe { data.assume_init() },
        }
    }

    fn get(&self, x: u16, y: u16) -> Option<Cell> {
        self.data
            .get(x as usize + y as usize * self.size as usize)
            .cloned()
    }

    fn idx_from_pos(&self, x: u16, y: u16) -> usize {
        x as usize + y as usize * self.size as usize
    }

    fn world_pos_from_grid_pos(x: u16, y: u16, cell_px: f32) -> Vec2 {
        Vec2::new(
            (x as f32 + 0.5) * cell_px - WIN_WIDTH / 2.0,
            WIN_HEIGHT / 2.0 - (y as f32 + 0.5) * cell_px,
        )
    }

    fn move_cell(&mut self, x: u16, y: u16, dir: Dir) -> bool {
        let Some((tx, ty)) = dir.apply(x, y, self.size) else {
            return false;
        };
        let source = self.idx_from_pos(x, y);
        let target = self.idx_from_pos(tx, ty);
        self.data[target] = self.data[source];
        self.data[source] = Cell::Empty;
        true
    }
}

const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

#[derive(Clone, Copy, Debug)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn apply(self, x: u16, y: u16, grid_size: u16) -> Option<(u16, u16)> {
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
enum Cell {
    Empty,
    Entity(Entity),
}

#[derive(Component, Clone, Debug)]
struct CellEntity {
    x: u16,
    y: u16,
    net: Net,
}

impl CellEntity {
    fn spawn(
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
            ))
            .id();
        grid.data[x as usize + y as usize * grid_size.0 as usize] = Cell::Entity(id);
    }

    fn update_tf(mut q: Query<(&mut Transform, &Self), Changed<Self>>, grid_size: Res<GridSize>) {
        for (mut tf, ce) in &mut q {
            let pos = Grid::world_pos_from_grid_pos(ce.x, ce.y, grid_size.cell_px());
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
        }
    }
}

#[derive(Clone, Debug)]
struct Net {
    layers: Vec<Layer>,
    temperature: f32,
    mutation_rate: f32,
}

impl Net {
    fn random(layers: &[usize], rng: &mut Rng) -> Net {
        Net {
            layers: (0..layers.len())
                .map(|i| {
                    if i == 0 {
                        Layer::input_layer(layers[0])
                    } else {
                        Layer::random(
                            layers[i],
                            layers[i - 1],
                            if i == layers.len() - 1 {
                                ActivationFn::Unchanged
                            } else {
                                ActivationFn::Relu
                            },
                            rng,
                        )
                    }
                })
                .collect(),
            temperature: 1.0,
            mutation_rate: rng.f32() * 0.05 + 0.025,
        }
    }

    fn mix(&self, other: &Net, rng: &mut Rng) -> Net {
        assert_eq!(self.layers.len(), other.layers.len());
        Net {
            layers: self
                .layers
                .iter()
                .zip(&other.layers)
                .map(|(l1, l2)| l1.mix(l2, rng))
                .collect(),
            temperature: if rng.bool() {
                self.temperature
            } else {
                other.temperature
            },
            mutation_rate: if rng.bool() {
                self.mutation_rate
            } else {
                other.mutation_rate
            },
        }
    }

    fn mutate(&mut self, rng: &mut Rng) {
        self.layers
            .iter_mut()
            .for_each(|l| l.mutate(self.mutation_rate, rng));
        self.mutation_rate *= rng.f32() * 0.2 + 0.9;
        if self.mutation_rate < 0.001 {
            self.mutation_rate = 0.001;
        }
    }

    fn avg(nets: &[&Net]) -> Net {
        assert!(!nets.is_empty());
        for net in nets.iter().skip(1) {
            assert_eq!(net.layers.len(), nets[0].layers.len());
        }
        Net {
            layers: (0..nets[0].layers.len())
                .map(|i| Layer::avg(&nets.iter().map(|n| &n.layers[i]).collect::<Vec<_>>()))
                .collect(),
            temperature: nets.iter().map(|n| n.temperature).sum::<f32>() / nets.len() as f32,
            mutation_rate: nets.iter().map(|n| n.mutation_rate).sum::<f32>() / nets.len() as f32,
        }
    }

    fn set_inputs(&mut self, inputs: &[f32]) {
        assert_eq!(self.layers[0].neurons.len(), inputs.len());
        for (neuron, input) in self.layers[0].neurons.iter_mut().zip(inputs) {
            neuron.value = *input;
        }
    }

    fn eval(&mut self) {
        for i in 1..self.layers.len() {
            let [current, prev] = self.layers.get_disjoint_mut([i, i - 1]).unwrap();
            current.eval(prev);
        }
        // softmax
        let output_neurons = &mut self.layers.last_mut().unwrap().neurons;
        let mut sum = 0.0;
        let mut softmax = Vec::with_capacity(output_neurons.len());
        for neuron in output_neurons.iter() {
            let v = (neuron.value / self.temperature).exp();
            sum += v;
            softmax.push(v);
        }
        for (neuron, softmax) in output_neurons.iter_mut().zip(softmax) {
            neuron.value = softmax / sum;
        }
    }

    fn flattened_properties(&self) -> Vec<f32> {
        let mut vec: Vec<_> = self
            .layers
            .iter()
            .skip(1)
            .map(|l| l.flattened_properties())
            .flatten()
            .collect();
        vec.push(self.temperature);
        vec.push(self.mutation_rate);
        vec
    }
}

#[derive(Clone, Debug)]
struct Layer {
    neurons: Vec<Neuron>,
    activation_fn: ActivationFn,
}

impl Layer {
    fn random(size: usize, prev_size: usize, activation_fn: ActivationFn, rng: &mut Rng) -> Self {
        Self {
            neurons: (0..size).map(|_| Neuron::random(prev_size, rng)).collect(),
            activation_fn,
        }
    }

    fn input_layer(size: usize) -> Self {
        Self {
            neurons: vec![Neuron::default(); size],
            activation_fn: ActivationFn::Unchanged,
        }
    }

    fn mix(&self, other: &Self, rng: &mut Rng) -> Self {
        assert_eq!(self.neurons.len(), other.neurons.len());
        Self {
            neurons: self
                .neurons
                .iter()
                .zip(&other.neurons)
                .map(|(n1, n2)| n1.mix(n2, rng))
                .collect(),
            activation_fn: if rng.bool() {
                self.activation_fn
            } else {
                other.activation_fn
            },
        }
    }

    fn mutate(&mut self, amount: f32, rng: &mut Rng) {
        self.neurons.iter_mut().for_each(|n| n.mutate(amount, rng));
    }

    fn avg(layers: &[&Self]) -> Self {
        for layer in layers.iter().skip(1) {
            assert_eq!(layer.neurons.len(), layers[0].neurons.len());
        }
        Self {
            neurons: (0..layers[0].neurons.len())
                .map(|i| Neuron::avg(&layers.iter().map(|l| &l.neurons[i]).collect::<Vec<_>>()))
                .collect(),
            activation_fn: layers[0].activation_fn,
        }
    }

    fn eval(&mut self, prev: &Self) {
        for neuron in &mut self.neurons {
            neuron.eval(prev, self.activation_fn);
        }
    }

    fn flattened_properties(&self) -> Vec<f32> {
        self.neurons
            .iter()
            .map(|n| n.flattened_properties())
            .flatten()
            .collect()
    }
}

#[derive(Clone, Debug)]
struct Neuron {
    value: f32,
    weights: Vec<f32>,
    bias: f32,
}

impl Default for Neuron {
    fn default() -> Self {
        Self {
            value: f32::NAN,
            weights: vec![],
            bias: f32::NAN,
        }
    }
}

impl Neuron {
    const INIT_MAX: f32 = 2.0;

    fn random(weight_count: usize, rng: &mut Rng) -> Self {
        Self {
            value: f32::NAN,
            weights: (0..weight_count)
                .map(|_| rng.f32() * 2.0 * Self::INIT_MAX - Self::INIT_MAX)
                .collect(),
            bias: rng.f32() * 2.0 * Self::INIT_MAX - Self::INIT_MAX,
        }
    }

    fn mix(&self, other: &Self, rng: &mut Rng) -> Self {
        Self {
            value: f32::NAN,
            weights: self
                .weights
                .iter()
                .zip(&other.weights)
                .map(|(w1, w2)| if rng.bool() { *w1 } else { *w2 })
                .collect(),
            bias: if rng.bool() { self.bias } else { other.bias },
        }
    }

    fn mutate(&mut self, amount: f32, rng: &mut Rng) {
        self.weights.iter_mut().for_each(|w| {
            *w += rng.f32() * amount * Self::INIT_MAX - 0.5 * amount * Self::INIT_MAX;
            *w = w.clamp(-1.5 * Self::INIT_MAX, 1.5 * Self::INIT_MAX);
        });
        self.bias += rng.f32() * amount * Self::INIT_MAX - 0.5 * amount * Self::INIT_MAX;
        self.bias = self.bias.clamp(-1.5 * Self::INIT_MAX, 1.5 * Self::INIT_MAX);
    }

    fn avg(neurons: &[&Self]) -> Self {
        Self {
            value: f32::NAN,
            weights: (0..neurons[0].weights.len())
                .map(|i| neurons.iter().map(|n| n.weights[i]).sum::<f32>() / neurons.len() as f32)
                .collect(),
            bias: neurons.iter().map(|n| n.bias).sum::<f32>() / neurons.len() as f32,
        }
    }

    fn eval(&mut self, prev: &Layer, activation_fn: ActivationFn) {
        let sum = prev
            .neurons
            .iter()
            .enumerate()
            .map(|(i, n)| n.value * self.weights[i])
            .sum::<f32>();
        self.value = activation_fn.eval(sum + self.bias);
    }

    fn flattened_properties(&self) -> Vec<f32> {
        let mut vec = self.weights.clone();
        vec.push(self.bias);
        vec
    }
}

#[derive(Clone, Copy, Debug)]
enum ActivationFn {
    Unchanged,
    Relu,
}

impl ActivationFn {
    fn eval(self, x: f32) -> f32 {
        match self {
            Self::Unchanged => x,
            Self::Relu => x.max(0.0),
        }
    }
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
        nets.iter().map(|n| n.mutation_rate).sum::<f32>() / nets.len() as f32
    }
}

#[derive(Resource, Debug)]
struct Square(Handle<Mesh>);

#[derive(Resource, Debug)]
struct CellEntityMaterial(Handle<ColorMaterial>);

#[derive(Component)]
struct TextInput {
    on_submit: fn(&str, Commands) -> bool,
}

#[derive(Resource, Clone, Copy)]
struct GridSize(u16);

impl GridSize {
    fn cell_px(&self) -> f32 {
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
struct EntityCount(usize);

fn submit_entity_count(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(EntityCount(v));
    true
}

#[derive(Resource)]
struct TicksPerGen(u32);

fn submit_ticks_per_gen(text: &str, mut commands: Commands) -> bool {
    let Ok(v) = text.parse() else {
        return false;
    };
    commands.insert_resource(TicksPerGen(v));
    true
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

#[derive(Component)]
struct ActionButton {
    on_press: fn(Commands),
}

fn start_sim(mut commands: Commands) {
    commands.set_state(AppState::Sim);
}

#[derive(Component)]
struct Focus;

#[derive(Component)]
struct Focusable {
    order: u8,
}

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

    commands.insert_resource(Square(meshes.add(Rectangle::default())));
    commands.insert_resource(CellEntityMaterial(
        materials.add(Color::srgb(0.8, 0.2, 0.2)),
    ));
}

fn setup_settings(mut commands: Commands) {
    let default_grid_size = 50;
    let default_entity_count = 100;
    let default_ticks_per_gen = 100;
    commands.insert_resource(GridSize(default_grid_size));
    commands.insert_resource(EntityCount(default_entity_count));
    commands.insert_resource(TicksPerGen(default_ticks_per_gen));

    commands.spawn((
        Text2d::new("Settings"),
        TextFont {
            font_size: 50.0,
            ..default()
        },
        Transform::from_xyz(0.0, WIN_HEIGHT / 2.0 - 30.0, 2.0),
        DespawnOnExit(AppState::Settings),
    ));

    commands.spawn((
        Text2d::new("Grid size: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-300.0, WIN_HEIGHT / 2.0 - 100.0, 2.0),
        DespawnOnExit(AppState::Settings),
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
        DespawnOnExit(AppState::Settings),
    ));

    commands.spawn((
        Text2d::new("Entity count: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-300.0, WIN_HEIGHT / 2.0 - 150.0, 2.0),
        DespawnOnExit(AppState::Settings),
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
        DespawnOnExit(AppState::Settings),
    ));

    commands.spawn((
        Text2d::new("Ticks per generation: "),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(-400.0, WIN_HEIGHT / 2.0 - 200.0, 2.0),
        DespawnOnExit(AppState::Settings),
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
        DespawnOnExit(AppState::Settings),
    ));

    commands.spawn((
        Focusable { order: 3 },
        ActionButton {
            on_press: start_sim,
        },
        Text2d::new("Start"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(0.0, WIN_HEIGHT / 2.0 - 270.0, 2.0),
        DespawnOnExit(AppState::Settings),
    ));
}

fn setup_sim(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    square: Res<Square>,
    entity_count: Res<EntityCount>,
    grid_size: Res<GridSize>,
) {
    let grid_size = *grid_size;

    let black = materials.add(Color::BLACK);
    let square_clone = square.0.clone();
    let black_clone = black.clone();
    commands.spawn_batch((0..grid_size.0 + 1).map(move |i| {
        let square = square_clone.clone();
        let black = black_clone.clone();
        (
            Mesh2d(square),
            MeshMaterial2d(black),
            Transform {
                translation: Vec3::new(
                    i as f32 * grid_size.cell_px() - WIN_WIDTH / 2.0,
                    (WIN_HEIGHT - grid_size.0 as f32 * grid_size.cell_px()) / 2.0,
                    1.0,
                ),
                scale: Vec3::new(LINE_WIDTH, grid_size.0 as f32 * grid_size.cell_px(), 1.0),
                ..default()
            },
        )
    }));
    let square_clone = square.0.clone();
    commands.spawn_batch((0..grid_size.0 + 1).map(move |i| {
        let square = square_clone.clone();
        (
            Mesh2d(square),
            MeshMaterial2d(black.clone()),
            Transform {
                translation: Vec3::new(
                    (WIN_WIDTH - grid_size.0 as f32 * grid_size.cell_px()) / -2.0,
                    WIN_HEIGHT / 2.0 - i as f32 * grid_size.cell_px(),
                    1.0,
                ),
                scale: Vec3::new(grid_size.0 as f32 * grid_size.cell_px(), LINE_WIDTH, 1.0),
                ..default()
            },
        )
    }));

    commands.insert_resource(Grid::new(grid_size.0));

    let mut rng = Rng(1);

    let mut pos = HashSet::with_capacity(entity_count.0);
    let mut ce = Vec::with_capacity(entity_count.0);
    assert!(entity_count.0 <= grid_size.0 as usize * grid_size.0 as usize);
    while pos.len() < entity_count.0 {
        let x = rng.u64() as u16 % grid_size.0;
        let y = rng.u64() as u16 % grid_size.0;
        if pos.insert((x, y)) {
            let net = Net::random(&[8, 6, 5], &mut rng);
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
    ));

    commands.spawn((
        SurvivorsLabel,
        Text2d(format!("Survivors: N/A / {}", entity_count.0)),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(340.0, WIN_HEIGHT / 2.0 - 80.0, 2.0),
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
    ));

    commands.spawn((
        MutationRateLabel,
        Text2d(MutationRateLabel::text(&nets)),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(400.0, WIN_HEIGHT / 2.0 - 180.0, 2.0),
    ));

    commands.spawn((
        Text2d::new("Desired TPS:"),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        Transform::from_xyz(250.0, WIN_HEIGHT / 2.0 - 230.0, 2.0),
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

fn get_btn_input(
    mut kb: MessageReader<KeyboardInput>,
    btn: Single<(Entity, &ActionButton, &Focusable), With<Focus>>,
    focusable: Query<(Entity, &Focusable), Without<Focus>>,
    mut commands: Commands,
) {
    let (btn_e, btn, current_focus) = btn.into_inner();
    for ki in kb.read() {
        if ki.state == ButtonState::Released {
            continue;
        }
        if ki.key_code == KeyCode::Enter {
            (btn.on_press)(commands.reborrow());
            return;
        } else if ki.key_code == KeyCode::ArrowUp {
            let next = current_focus.order.saturating_sub(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(btn_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if ki.key_code == KeyCode::ArrowDown {
            let next = current_focus.order.saturating_add(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(btn_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        }
    }
}

fn get_text_input(
    mut kb: MessageReader<KeyboardInput>,
    text_input: Single<(Entity, &mut Text2d, &TextInput, &Focusable), With<Focus>>,
    focusable: Query<(Entity, &Focusable), Without<Focus>>,
    mut commands: Commands,
) {
    let (in_e, mut text2d, text_input, current_focus) = text_input.into_inner();
    for ki in kb.read() {
        if ki.state == ButtonState::Released {
            continue;
        }

        if ki.key_code == KeyCode::Enter {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            return;
        } else if ki.key_code == KeyCode::Backspace {
            text2d.pop();
        } else if ki.key_code == KeyCode::ArrowUp {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            let next = current_focus.order.saturating_sub(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(in_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if ki.key_code == KeyCode::ArrowDown {
            if !(text_input.on_submit)(&text2d.0, commands.reborrow()) {
                text2d.clear();
            }
            let next = current_focus.order.saturating_add(1);
            for (e, focusable) in &focusable {
                if focusable.order == next {
                    commands.entity(in_e).remove::<Focus>();
                    commands.entity(e).insert(Focus);
                    return;
                }
            }
            return;
        } else if let Some(text) = &ki.text {
            text2d.push_str(text);
        }
    }
}

fn tick(
    mut ce_q: Query<&mut CellEntity>,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<Rng>,
    mut tick: ResMut<Tick>,
    ticks_per_gen: Res<TicksPerGen>,
    mut commands: Commands,
) {
    if tick.0 >= ticks_per_gen.0 {
        commands.trigger(FinishGeneration);
        return;
    }
    for mut ce in &mut ce_q {
        let x = ce.x as f32 / grid.size as f32;
        let y = ce.y as f32 / grid.size as f32;
        let mut inputs = vec![x, y, tick.0 as f32 / ticks_per_gen.0 as f32, rng.f32()];
        for dir in DIRS {
            inputs.push(
                if let Some((x, y)) = dir.apply(ce.x, ce.y, grid.size)
                    && grid.get(x, y) == Some(Cell::Empty)
                {
                    1.0
                } else {
                    0.0
                },
            );
        }
        ce.net.set_inputs(&inputs);
        ce.net.eval();
        let rand = rng.f32();
        let mut i = -1;
        let neurons = ce.net.layers.last().unwrap().neurons.len() as i32;
        let mut sum = 0.0;
        while sum < rand && i + 1 < neurons {
            i += 1;
            sum += ce.net.layers.last().unwrap().neurons[i as usize].value;
        }
        let Some(dir) = DIRS.get(i as usize) else {
            continue;
        };
        if let Some((x, y)) = dir.apply(ce.x, ce.y, grid.size)
            && grid.get(x, y) == Some(Cell::Empty)
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
        if ce.x > grid_size.0 / 2 {
            survivors.push(&ce.net);
        }
    }

    survivors_label.0 = format!("Survivors: {} / {}", survivors.len(), entity_count.0);

    dbg!(survivors.len());
    dbg!(
        survivors[0]
            .layers
            .last()
            .unwrap()
            .neurons
            .iter()
            .map(|n| n.value)
            .collect::<Vec<_>>()
    );

    let mut pos = HashSet::with_capacity(entity_count.0);
    let mut ce = Vec::with_capacity(entity_count.0);
    while pos.len() < entity_count.0 {
        let x = rng.u64() as u16 % grid_size.0;
        let y = rng.u64() as u16 % grid_size.0;
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

    commands.insert_resource(Grid::new(grid_size.0));

    ce.into_iter()
        .for_each(|ce| commands.run_system_cached_with(CellEntity::spawn, ce));

    tick.0 = 0;
}
