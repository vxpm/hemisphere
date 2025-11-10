import package::tev::{input, input::Input};
import package::uber;

const PLACEHOLDER_RGB: vec3f = vec3f(1.0, 0.0, 0.8627);
const PLACEHOLDER_RGBA: vec4f = vec4f(1.0, 0.0, 0.8627, 0.5);

alias Reg = u32;
const R0: Reg = 1;
const R1: Reg = 2;
const R2: Reg = 3;
const R3: Reg = 0;

// Configuration for a TEV operation.
struct OpConfig {
    input_a: Input,
    input_b: Input, 
    input_c: Input,
    input_d: Input,
    output: Reg,

    sign: f32,
    bias: f32,
    scale: f32,
    clamp: u32,
};

struct Refs {
    map: u32,
    coord: u32,
    color: u32,
};

const COLOR_REF_DIFFUSE_COLOR: u32 = 0x0;
const COLOR_REF_SPECULAR_COLOR: u32 = 0x1;
const COLOR_REF_DIFFUSE_ALPHA: u32 = 0x2;
const COLOR_REF_SPECULAR_ALPHA: u32 = 0x3;
const COLOR_REF_DIFFUSE_COLOR_ALPHA: u32 = 0x4;
const COLOR_REF_SPECULAR_COLOR_ALPHA: u32 = 0x5;
const COLOR_REF_ZERO: u32 = 0x6;

// Configuration for a TEV stage, both color and alpha operations.
struct Stage {
    color: OpConfig, 
    alpha: OpConfig,
    refs: Refs,
};

struct Constants {
    color0: vec4f,
    color1: vec4f, 
    color2: vec4f, 
    color3: vec4f, 
};

/// All TEV stage configurations.
struct Config {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    constants: Constants,
    stages: array<Stage, 16>,
};

struct State {
    regs: array<vec4f, 4>,
    diffuse: vec4f,
    specular: vec4f,
    coords: array<vec3f, 8>,
    constants: array<vec4f, 4>,
    
    // per stage
    current_map: u32,
    current_coord: u32,
    current_color: u32,
}

fn sample_tex(state: ptr<function, State>) -> vec4f {
    let coord = state.coords[state.current_coord].xy;
    var sample: vec4f;
    switch state.current_map {
        case 0: {
            sample = textureSample(uber::texture0, uber::sampler0, coord);
        }
        case 1: {
            sample = textureSample(uber::texture1, uber::sampler1, coord);
        }
        case 2: {
            sample = textureSample(uber::texture2, uber::sampler2, coord);
        }
        case 3: {
            sample = textureSample(uber::texture3, uber::sampler3, coord);
        }
        case 4: {
            sample = textureSample(uber::texture4, uber::sampler4, coord);
        }
        case 5: {
            sample = textureSample(uber::texture5, uber::sampler5, coord);
        }
        case 6: {
            sample = textureSample(uber::texture6, uber::sampler6, coord);
        }
        case 7: {
            sample = textureSample(uber::texture7, uber::sampler7, coord);
        }
        default: {
            sample = PLACEHOLDER_RGBA;
        }
    }

    return sample;
}

fn get_color(state: ptr<function, State>) -> vec4f {
    var color: vec4f;

    // TODO: figure out these values, they look wrong and libogc does weird stuff
    switch state.current_color {
        case COLOR_REF_DIFFUSE_COLOR: {
            // color = vec4f(state.diffuse.rgb, 1.0);
            color = state.diffuse;
        }
        case COLOR_REF_SPECULAR_COLOR: {
            // color = vec4f(state.specular.rgb, 1.0);
            color = state.specular;
        }
        case COLOR_REF_DIFFUSE_ALPHA: {
            color = state.diffuse.aaaa;
        }
        case COLOR_REF_SPECULAR_ALPHA: {
            color = state.specular.aaaa;
        }
        case COLOR_REF_DIFFUSE_COLOR_ALPHA: {
            color = state.diffuse;
        }
        case COLOR_REF_SPECULAR_COLOR_ALPHA: {
            color = state.specular;
        }
        default: {
            color = PLACEHOLDER_RGBA;
        }
    }

    return color;
}

/// Returns the value for the given input in the given state.
fn get_color_input(state: ptr<function, State>, value: Input) -> vec3f {
    switch value {
        // Registers
        case input::COLOR_R3C: {
            return state.regs[R3].rgb;
        }
        case input::COLOR_R3A: {
            return state.regs[R3].aaa;
        }
        case input::COLOR_R0C: {
            return state.regs[R0].rgb;
        }
        case input::COLOR_R0A: {
            return state.regs[R0].aaa;
        }
        case input::COLOR_R1C: {
            return state.regs[R1].rgb;
        }
        case input::COLOR_R1A: {
            return state.regs[R1].aaa;
        }
        case input::COLOR_R2C: {
            return state.regs[R2].rgb;
        }
        case input::COLOR_R2A: {
            return state.regs[R2].aaa;
        }

        // Texture colors
        case input::COLOR_TEX_COLOR: {
            return sample_tex(state).rgb;
        }
        case input::COLOR_TEX_ALPHA: {
            return sample_tex(state).aaa;
        }

        // Rasterizer colors
        case input::COLOR_RAS_COLOR: {
            return get_color(state).rgb;
        }
        case input::COLOR_RAS_ALPHA: {
            return get_color(state).aaa;
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

/// Returns the value for the given input in the given state.
fn get_alpha_input(state: ptr<function, State>, value: Input) -> f32 {
    switch value {
        // Registers
        case input::ALPHA_R3: {
            return state.regs[R3].a;
        }
        case input::ALPHA_R0: {
            return state.regs[R0].a;
        }
        case input::ALPHA_R1: {
            return state.regs[R1].a;
        }
        case input::ALPHA_R2: {
            return state.regs[R2].a;
        }

        // Texture colors
        case input::ALPHA_TEX: {
            return sample_tex(state).a;
        }

        // Rasterizer colors
        case input::ALPHA_RAS: {
            return get_color(state).a;
        }

        // Constants
        case input::ALPHA_CONSTANT: {
            return PLACEHOLDER_RGBA.a;
        }
        case input::ALPHA_ZERO: {
            return 0.0;
        }

        default: {
            return PLACEHOLDER_RGBA.a;
        }
    }
}

fn compute_stage_color(a: vec3f, b: vec3f, c: vec3f, d: vec3f, sign: f32, bias: f32, scale: f32, clamp: u32) -> vec3f {
    let lerp = sign * (a * (vec3f(1.0) - c) + b * c);
    let result = (lerp + d + bias) * scale;

    if clamp != 0 {
        return clamp(result, vec3f(0.0), vec3f(1.0));
    } else {
        return result;
    }
}

fn compute_stage_alpha(a: f32, b: f32, c: f32, d: f32, sign: f32, bias: f32, scale: f32, clamp: u32) -> f32 {
    let lerp = sign * (a * (1.0 - c) + b * c);
    let result = (lerp + d + bias) * scale;
    
    if clamp != 0 {
        return clamp(result, 0.0, 1.0);
    } else {
        return result;
    }
}

fn compute(config: Config, diffuse: vec4f, specular: vec4f, coords: array<vec3f, 8>) -> vec4f {
    var state: State;
    state.diffuse = diffuse;
    state.specular = specular;
    state.coords = coords;
    state.constants[R0] = config.constants.color0;
    state.constants[R1] = config.constants.color1;
    state.constants[R2] = config.constants.color2;
    state.constants[R3] = config.constants.color3;
    state.regs = state.constants;

    var last_color_output = PLACEHOLDER_RGB;
    var last_alpha_output = 1.0;

    for (var i = 0u; i < config.count; i++) {
        let stage = config.stages[i];
        state.current_map = stage.refs.map;
        state.current_coord = stage.refs.coord;
        state.current_color = stage.refs.color;

        let color_a = get_color_input(&state, stage.color.input_a);
        let color_b = get_color_input(&state, stage.color.input_b);
        let color_c = get_color_input(&state, stage.color.input_c);
        let color_d = get_color_input(&state, stage.color.input_d);
        let color = compute_stage_color(
            color_a,
            color_b,
            color_c,
            color_d,
            stage.color.sign,
            stage.color.bias,
            stage.color.scale,
            stage.color.clamp,
        );

        state.regs[stage.color.output].r = color.r;
        state.regs[stage.color.output].g = color.g;
        state.regs[stage.color.output].b = color.b;
        last_color_output = color;

        let alpha_a = get_alpha_input(&state, stage.alpha.input_a);
        let alpha_b = get_alpha_input(&state, stage.alpha.input_b);
        let alpha_c = get_alpha_input(&state, stage.alpha.input_c);
        let alpha_d = get_alpha_input(&state, stage.alpha.input_d);
        let alpha = compute_stage_alpha(
            alpha_a,
            alpha_b,
            alpha_c,
            alpha_d,
            stage.alpha.sign,
            stage.alpha.bias,
            stage.alpha.scale,
            stage.alpha.clamp,
        );

        state.regs[stage.alpha.output].a = alpha;
        last_alpha_output = alpha;
    }

    return vec4f(last_color_output, last_alpha_output);
}