import package::tev::{input, input::Input};

const PLACEHOLDER_RGB: vec3f = vec3f(1.0, 0.0, 0.8627);
const PLACEHOLDER_RGBA: vec4f = vec4f(1.0, 0.0, 0.8627, 0.5);

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
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

// Configuration for a TEV stage step.
struct Config {
    in_out: InOut,
    sign: f32,
    bias: f32,
    scale: f32,
    clamp: u32,
};

// Configuration for a TEV stage, both color and alpha steps.
struct Stage {
    color: Config, 
    alpha: Config, 
};

/// All TEV stage configurations.
struct Stages {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    stages: array<Stage, 8>,
};

struct State {
    regs: array<vec4f, 4>,
}

/// Returns the value for the given input in the given state.
fn get_color_input(state: ptr<function, State>, value: Input) -> vec3f {
    switch value {
        // Registers
        case input::COLOR_R3C: {
            return state.regs[3].rgb;
        }
        case input::COLOR_R3A: {
            return state.regs[3].aaa;
        }
        case input::COLOR_R0C: {
            return state.regs[0].rgb;
        }
        case input::COLOR_R0A: {
            return state.regs[0].aaa;
        }
        case input::COLOR_R1C: {
            return state.regs[1].rgb;
        }
        case input::COLOR_R1A: {
            return state.regs[1].aaa;
        }
        case input::COLOR_R2C: {
            return state.regs[2].rgb;
        }
        case input::COLOR_R2A: {
            return state.regs[2].aaa;
        }

        // Texture colors
        case input::COLOR_TEX_COLOR: {
            return PLACEHOLDER_RGB;
        }
        case input::COLOR_TEX_ALPHA: {
            return PLACEHOLDER_RGBA.aaa;
        }

        // Rasterizer colors
        case input::COLOR_RAS_COLOR: {
            return PLACEHOLDER_RGB;
        }
        case input::COLOR_RAS_ALPHA: {
            return PLACEHOLDER_RGBA.aaa;
        }

        // Constants
        case input::COLOR_ONE: {
            return vec3f(1.0);
        }
        case input::COLOR_HALF: {
            return vec3f(0.5);
        }
        case input::COLOR_CONSTANT: {
            return PLACEHOLDER_RGB;
        }
        case input::COLOR_ZERO: {
            return vec3f(0.0);
        }

        default: {
            return PLACEHOLDER_RGB;
        }
    }
}

// Computes the output of a TEV stage with the given parameters.
fn compute(a: vec3f, b: vec3f, c: vec3f, d: vec3f, sign: f32, bias: f32, scale: f32) -> vec3f {
    let lerp = sign * mix(a, b, c);
    return (lerp + d + bias) * scale;
}