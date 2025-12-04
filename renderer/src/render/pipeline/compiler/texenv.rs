use hemisphere::{
    render::TexEnvStage,
    system::gx::tev::{
        AlphaCompare, AlphaInputSrc, AlphaLogic, ColorChannel, ColorInputSrc, Constant,
    },
};

use crate::render::pipeline::AlphaFunctionSettings;

fn sample_tex(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    let map = stage.refs.map().value();
    let tex_ident = wesl::syntax::Ident::new(format!("base::texture{map}"));
    let sampler_ident = wesl::syntax::Ident::new(format!("base::sampler{map}"));
    let coord_ident = wesl::syntax::Ident::new(format!("in.tex_coord{map}"));

    wesl::quote_expression! {
        textureSample(#tex_ident, #sampler_ident, #coord_ident.xy / #coord_ident.z)
    }
}

fn get_color_channel(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.refs.color() {
        ColorChannel::Channel0 => wesl::quote_expression! { in.chan0 },
        ColorChannel::Channel1 => wesl::quote_expression! { in.chan1 },
        ColorChannel::AlphaBump => wesl::quote_expression! { vec4f(base::PLACEHOLDER_RGB, 0f) },
        ColorChannel::AlphaBumpNormalized => {
            wesl::quote_expression! { vec4f(base::PLACEHOLDER_RGB, 0f) }
        }
        ColorChannel::Zero => wesl::quote_expression! { vec4f(0f) },
        _ => panic!("reserved color channel"),
    }
}

fn get_color_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.color_const {
        Constant::One => wesl::quote_expression! { vec4f(1f) },
        Constant::SevenEights => wesl::quote_expression! { vec4f(7f / 8f) },
        Constant::SixEights => wesl::quote_expression! { vec4f(6f / 8f) },
        Constant::FiveEights => wesl::quote_expression! { vec4f(5f / 8f) },
        Constant::FourEights => wesl::quote_expression! { vec4f(4f / 8f) },
        Constant::ThreeEights => wesl::quote_expression! { vec4f(3f / 8f) },
        Constant::TwoEights => wesl::quote_expression! { vec4f(2f / 8f) },
        Constant::OneEight => wesl::quote_expression! { vec4f(1f / 8f) },
        Constant::Const0 => wesl::quote_expression! { consts[R0] },
        Constant::Const1 => wesl::quote_expression! { consts[R1] },
        Constant::Const2 => wesl::quote_expression! { consts[R2] },
        Constant::Const3 => wesl::quote_expression! { consts[R3] },
        Constant::Const0R => wesl::quote_expression! { consts[R0].rrrr },
        Constant::Const1R => wesl::quote_expression! { consts[R1].rrrr },
        Constant::Const2R => wesl::quote_expression! { consts[R2].rrrr },
        Constant::Const3R => wesl::quote_expression! { consts[R3].rrrr },
        Constant::Const0G => wesl::quote_expression! { consts[R0].gggg },
        Constant::Const1G => wesl::quote_expression! { consts[R1].gggg },
        Constant::Const2G => wesl::quote_expression! { consts[R2].gggg },
        Constant::Const3G => wesl::quote_expression! { consts[R3].gggg },
        Constant::Const0B => wesl::quote_expression! { consts[R0].bbbb },
        Constant::Const1B => wesl::quote_expression! { consts[R1].bbbb },
        Constant::Const2B => wesl::quote_expression! { consts[R2].bbbb },
        Constant::Const3B => wesl::quote_expression! { consts[R3].bbbb },
        Constant::Const0A => wesl::quote_expression! { consts[R0].aaaa },
        Constant::Const1A => wesl::quote_expression! { consts[R1].aaaa },
        Constant::Const2A => wesl::quote_expression! { consts[R2].aaaa },
        Constant::Const3A => wesl::quote_expression! { consts[R3].aaaa },
        _ => panic!("reserved color constant"),
    }
}

fn get_color_input(stage: &TexEnvStage, input: ColorInputSrc) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match input {
        ColorInputSrc::R3Color => wesl::quote_expression! { regs[R3].rgb },
        ColorInputSrc::R3Alpha => wesl::quote_expression! { regs[R3].aaa },
        ColorInputSrc::R0Color => wesl::quote_expression! { regs[R0].rgb },
        ColorInputSrc::R0Alpha => wesl::quote_expression! { regs[R0].aaa },
        ColorInputSrc::R1Color => wesl::quote_expression! { regs[R1].rgb },
        ColorInputSrc::R1Alpha => wesl::quote_expression! { regs[R1].aaa },
        ColorInputSrc::R2Color => wesl::quote_expression! { regs[R2].rgb },
        ColorInputSrc::R2Alpha => wesl::quote_expression! { regs[R2].aaa },
        ColorInputSrc::TexColor => {
            let tex = sample_tex(stage);
            wesl::quote_expression! { #tex.rgb }
        }
        ColorInputSrc::TexAlpha => {
            let tex = sample_tex(stage);
            wesl::quote_expression! { #tex.aaa }
        }
        ColorInputSrc::RasterColor => {
            let color = get_color_channel(stage);
            wesl::quote_expression! { #color.rgb }
        }
        ColorInputSrc::RasterAlpha => {
            let color = get_color_channel(stage);
            wesl::quote_expression! { #color.aaa }
        }
        ColorInputSrc::One => wesl::quote_expression! { vec3f(1f) },
        ColorInputSrc::Half => wesl::quote_expression! { vec3f(0.5f) },
        ColorInputSrc::Constant => {
            let constant = get_color_const(stage);
            wesl::quote_expression! { #constant.rgb }
        }
        ColorInputSrc::Zero => wesl::quote_expression! { vec3f(0f) },
    }
}

pub fn color_stage(stage: &TexEnvStage) -> wesl::syntax::Statement {
    use wesl::syntax::*;

    let input_a = get_color_input(stage, stage.ops.color.input_a());
    let input_b = get_color_input(stage, stage.ops.color.input_b());
    let input_c = get_color_input(stage, stage.ops.color.input_c());
    let input_d = get_color_input(stage, stage.ops.color.input_d());

    let sign = if stage.ops.color.negate() { -1.0 } else { 1.0 };
    let bias = stage.ops.color.bias().value();
    let scale = stage.ops.color.scale().value();
    let clamp = stage.ops.color.clamp();
    let output = stage.ops.color.output() as u32;

    let clamped = if clamp {
        wesl::quote_expression! { color_add_mul }
    } else {
        wesl::quote_expression! { clamp(color_add_mul, vec3f(0f), vec3f(1f)) }
    };

    wesl::quote_statement! {
        {
            let input_a = #input_a;
            let input_b = #input_b;
            let input_c = #input_c;
            let input_d = #input_d;
            let sign = #sign;
            let bias = #bias;
            let scale = #scale;

            let color_interpolation = sign * mix(input_a, input_b, input_c);
            let color_add_mul = scale * (color_interpolation + input_d + bias);
            let color_result = #clamped;

            regs[#output] = vec4f(color_result, regs[#output].a);
            last_color_output = #output;
        }
    }
}

fn get_alpha_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.alpha_const {
        Constant::One => wesl::quote_expression! { 1f },
        Constant::SevenEights => wesl::quote_expression! { (7f / 8f) },
        Constant::SixEights => wesl::quote_expression! { (6f / 8f) },
        Constant::FiveEights => wesl::quote_expression! { (5f / 8f) },
        Constant::FourEights => wesl::quote_expression! { (4f / 8f) },
        Constant::ThreeEights => wesl::quote_expression! { (3f / 8f) },
        Constant::TwoEights => wesl::quote_expression! { (2f / 8f) },
        Constant::OneEight => wesl::quote_expression! { (1f / 8f) },
        Constant::Const0 => wesl::quote_expression! { consts[R0].a },
        Constant::Const1 => wesl::quote_expression! { consts[R1].a },
        Constant::Const2 => wesl::quote_expression! { consts[R2].a },
        Constant::Const3 => wesl::quote_expression! { consts[R3].a },
        Constant::Const0R => wesl::quote_expression! { consts[R0].r },
        Constant::Const1R => wesl::quote_expression! { consts[R1].r },
        Constant::Const2R => wesl::quote_expression! { consts[R2].r },
        Constant::Const3R => wesl::quote_expression! { consts[R3].r },
        Constant::Const0G => wesl::quote_expression! { consts[R0].g },
        Constant::Const1G => wesl::quote_expression! { consts[R1].g },
        Constant::Const2G => wesl::quote_expression! { consts[R2].g },
        Constant::Const3G => wesl::quote_expression! { consts[R3].g },
        Constant::Const0B => wesl::quote_expression! { consts[R0].b },
        Constant::Const1B => wesl::quote_expression! { consts[R1].b },
        Constant::Const2B => wesl::quote_expression! { consts[R2].b },
        Constant::Const3B => wesl::quote_expression! { consts[R3].b },
        Constant::Const0A => wesl::quote_expression! { consts[R0].a },
        Constant::Const1A => wesl::quote_expression! { consts[R1].a },
        Constant::Const2A => wesl::quote_expression! { consts[R2].a },
        Constant::Const3A => wesl::quote_expression! { consts[R3].a },
        _ => panic!("reserved alpha constant"),
    }
}

fn get_alpha_input(stage: &TexEnvStage, input: AlphaInputSrc) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match input {
        AlphaInputSrc::R3Alpha => wesl::quote_expression! { regs[R3].a },
        AlphaInputSrc::R0Alpha => wesl::quote_expression! { regs[R0].a },
        AlphaInputSrc::R1Alpha => wesl::quote_expression! { regs[R1].a },
        AlphaInputSrc::R2Alpha => wesl::quote_expression! { regs[R2].a },
        AlphaInputSrc::TexAlpha => {
            let tex = sample_tex(stage);
            wesl::quote_expression! { #tex.a }
        }
        AlphaInputSrc::RasterAlpha => {
            let color = get_color_channel(stage);
            wesl::quote_expression! { #color.a }
        }
        AlphaInputSrc::Constant => {
            let constant = get_alpha_const(stage);
            wesl::quote_expression! { (#constant) }
        }
        AlphaInputSrc::Zero => wesl::quote_expression! { 0f },
    }
}

pub fn alpha_stage(stage: &TexEnvStage) -> wesl::syntax::Statement {
    use wesl::syntax::*;

    let input_a = get_alpha_input(stage, stage.ops.alpha.input_a());
    let input_b = get_alpha_input(stage, stage.ops.alpha.input_b());
    let input_c = get_alpha_input(stage, stage.ops.alpha.input_c());
    let input_d = get_alpha_input(stage, stage.ops.alpha.input_d());

    let sign = if stage.ops.alpha.negate() { -1.0 } else { 1.0 };
    let bias = stage.ops.alpha.bias().value();
    let scale = stage.ops.alpha.scale().value();
    let clamp = stage.ops.alpha.clamp();
    let output = stage.ops.alpha.output() as u32;

    let clamped = if clamp {
        wesl::quote_expression! { alpha_add_mul }
    } else {
        wesl::quote_expression! { clamp(alpha_add_mul, 0f, 1f) }
    };

    wesl::quote_statement! {
        {
            let input_a = #input_a;
            let input_b = #input_b;
            let input_c = #input_c;
            let input_d = #input_d;
            let sign = #sign;
            let bias = #bias;
            let scale = #scale;

            let alpha_interpolation = sign * mix(input_a, input_b, input_c);
            let alpha_add_mul = scale * (alpha_interpolation + input_d + bias);
            let alpha_result = #clamped;

            regs[#output] = vec4f(regs[#output].rgb, alpha_result);
            last_alpha_output = #output;
        }
    }
}

fn get_alpha_comparison_helper(compare: AlphaCompare, idx: usize) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    let alpha_ref = wesl::syntax::Ident::new(format!("alpha_ref{idx}"));
    match compare {
        AlphaCompare::Never => wesl::quote_expression! { false },
        AlphaCompare::Less => wesl::quote_expression! { alpha < #alpha_ref },
        AlphaCompare::Equal => wesl::quote_expression! { alpha == #alpha_ref },
        AlphaCompare::LessOrEqual => wesl::quote_expression! { alpha <= #alpha_ref },
        AlphaCompare::Greater => wesl::quote_expression! { alpha > #alpha_ref },
        AlphaCompare::NotEqual => wesl::quote_expression! { alpha != #alpha_ref },
        AlphaCompare::GreaterOrEqual => wesl::quote_expression! { alpha >= #alpha_ref },
        AlphaCompare::Always => wesl::quote_expression! { true },
    }
}

pub fn get_alpha_comparison(settings: &AlphaFunctionSettings) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    let a = get_alpha_comparison_helper(settings.comparison[0], 0);
    let b = get_alpha_comparison_helper(settings.comparison[1], 1);

    match settings.logic {
        AlphaLogic::And => wesl::quote_expression! { (#a) && (#b) },
        AlphaLogic::Or => wesl::quote_expression! { (#a) || (#b) },
        AlphaLogic::Xor => wesl::quote_expression! { (#a) ^ (#b) },
        AlphaLogic::Xnor => wesl::quote_expression! { !((#a) ^ (#b)) },
    }
}
