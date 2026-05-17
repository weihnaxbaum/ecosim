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
            (tick, CellEntity::update_tf)
                .chain()
                .run_if(on_timer(Duration::from_secs_f32(0.0))),
        )
        .add_observer(finish_generation)
        .run()
}

const WIN_WIDTH: f32 = 1920.0;
const WIN_HEIGHT: f32 = 1080.0;
const GRID_SIZE: u16 = 50;
const CELL_PX: f32 = 20.0;
const LINE_WIDTH: f32 = 3.0;
const ENTITY_COUNT: usize = 100;
const TICKS: u32 = 100;

#[derive(Resource, Clone, Debug)]
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

const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

#[derive(Clone, Copy, Debug)]
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
    ) {
        let (x, y) = (ce.x, ce.y);
        let id = commands
            .spawn((
                ce,
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
        for (mut tf, ce) in &mut q {
            let pos = Grid::world_pos_from_grid_pos(ce.x, ce.y);
            tf.translation.x = pos.x;
            tf.translation.y = pos.y;
        }
    }
}

#[derive(Clone, Debug)]
struct Net {
    layers: Vec<Layer>,
    temperature: f32,
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
        }
    }

    fn mutate(&mut self, amount: f32, rng: &mut Rng) {
        self.layers.iter_mut().for_each(|l| l.mutate(amount, rng));
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

    fn eval(&mut self, prev: &Self) {
        for neuron in &mut self.neurons {
            neuron.eval(prev, self.activation_fn);
        }
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

    fn eval(&mut self, prev: &Layer, activation_fn: ActivationFn) {
        let sum = prev
            .neurons
            .iter()
            .enumerate()
            .map(|(i, n)| n.value * self.weights[i])
            .sum::<f32>();
        self.value = activation_fn.eval(sum + self.bias);
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

#[derive(Resource, Debug)]
struct Square(Handle<Mesh>);

#[derive(Resource, Debug)]
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

    let mut rng = Rng(1);

    let mut pos = HashSet::with_capacity(ENTITY_COUNT);
    for _ in 0..ENTITY_COUNT {
        let x = rng.u64() as u16 % GRID_SIZE;
        let y = rng.u64() as u16 % GRID_SIZE;
        if pos.insert((x, y)) {
            let net = Net::random(&[8, 6, 5], &mut rng);
            commands.run_system_cached_with(CellEntity::spawn, CellEntity { x, y, net });
        }
    }

    commands.insert_resource(rng);
    commands.insert_resource(Tick(0));
    commands.insert_resource(Generation(0));

    commands.spawn((
        GenerationLabel,
        Text2d::new("Generation 0"),
        TextFont {
            font_size: 60.0,
            ..default()
        },
        Transform::from_xyz(0.0, WIN_HEIGHT - 100.0, 2.0),
    ));
}

fn tick(
    mut ce_q: Query<&mut CellEntity>,
    mut grid: ResMut<Grid>,
    mut rng: ResMut<Rng>,
    mut tick: ResMut<Tick>,
    mut commands: Commands,
) {
    if tick.0 >= TICKS {
        commands.trigger(FinishGeneration);
        return;
    }
    for mut ce in &mut ce_q {
        let x = ce.x as f32 / GRID_SIZE as f32;
        let y = ce.y as f32 / GRID_SIZE as f32;
        let mut inputs = vec![x, y, tick.0 as f32 / TICKS as f32, rng.f32()];
        for dir in DIRS {
            inputs.push(
                if let Some((x, y)) = dir.apply(ce.x, ce.y)
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
        let mut sum = 0.0;
        while sum < rand {
            i += 1;
            sum += ce.net.layers.last().unwrap().neurons[i as usize].value;
        }
        let Some(dir) = DIRS.get(i as usize) else {
            continue;
        };
        if let Some((x, y)) = dir.apply(ce.x, ce.y)
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
    ce_q: Query<(Entity, &CellEntity)>,
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    mut tick: ResMut<Tick>,
) {
    generation.0 += 1;
    gen_label.0 = format!("Generation {}", generation.0);

    let mut survivers = vec![];
    for (e, ce) in &ce_q {
        commands.entity(e).despawn();
        if ce.x > GRID_SIZE / 2 {
            survivers.push(&ce.net);
        }
    }
    dbg!(survivers.len());
    dbg!(
        survivers[0]
            .layers
            .last()
            .unwrap()
            .neurons
            .iter()
            .map(|n| n.value)
            .collect::<Vec<_>>()
    );
    let mut pos = HashSet::with_capacity(ENTITY_COUNT);
    for _ in 0..ENTITY_COUNT {
        let x = rng.u64() as u16 % GRID_SIZE;
        let y = rng.u64() as u16 % GRID_SIZE;
        if !pos.insert((x, y)) {
            continue;
        }
        let net1 = rng.u64() as usize % survivers.len();
        let net2 = rng.u64() as usize % survivers.len();
        let mut net = survivers[net1].mix(survivers[net2], &mut rng);
        net.mutate(0.05, &mut rng);
        commands.run_system_cached_with(CellEntity::spawn, CellEntity { x, y, net });
    }
    tick.0 = 0;
}
