use crate::Rng;

#[derive(Clone, Debug)]
pub struct Net {
    layers: Vec<Layer>,
    temperature: f32,
    mutation_rate: f32,
}

impl Net {
    pub fn random(layers: &[usize], rng: &mut Rng) -> Net {
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

    pub fn mutation_rate(&self) -> f32 {
        self.mutation_rate
    }

    pub fn mix(&self, other: &Net, rng: &mut Rng) -> Net {
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

    pub fn mutate(&mut self, rng: &mut Rng) {
        self.layers
            .iter_mut()
            .for_each(|l| l.mutate(self.mutation_rate, rng));
        self.mutation_rate *= rng.f32() * 0.2 + 0.9;
        if self.mutation_rate < 0.001 {
            self.mutation_rate = 0.001;
        }
    }

    pub fn avg(nets: &[&Net]) -> Net {
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

    pub fn set_inputs(&mut self, inputs: &[f32]) {
        assert_eq!(self.layers[0].neurons.len(), inputs.len());
        for (neuron, input) in self.layers[0].neurons.iter_mut().zip(inputs) {
            neuron.value = *input;
        }
    }

    pub fn eval(&mut self) {
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

    pub fn output(&self) -> Vec<f32> {
        self.layers
            .last()
            .unwrap()
            .neurons
            .iter()
            .map(|n| n.value)
            .collect()
    }

    pub fn flattened_properties(&self) -> Vec<f32> {
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
