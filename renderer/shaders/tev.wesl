alias Input = u32;

alias Reg = u32;
const R3: Reg = 0;
const R0: Reg = 1;
const R1: Reg = 2;
const R2: Reg = 3;

// Defines the inputs and outputs of a TEV stage.
struct InOut {
    input_a: Input,
    input_b: Input, 
    input_c: Input,
    input_d: Input,
    output: Reg,

    // pad to 32 bytes
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

// 48 bytes
struct Config {
    in_out: InOut,
    sign: f32,
    bias: f32,
    scale: f32,
    clamp: u32,
};

// 96 bytes
struct Stage {
    color: Config, 
    alpha: Config, 
};

struct Stages {
    count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    stages: array<Stage, 8>,
};
