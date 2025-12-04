use hemisphere::{
    render::TexEnvStage,
    system::gx::tev::{
        AlphaCompare, AlphaInputSrc, AlphaLogic, Bias, ColorChannel, ColorInputSrc, CompareTarget,
        Constant,
    },
};
use wesl::quote_expression;

use crate::render::pipeline::AlphaFunctionSettings;

fn sample_tex(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    let map = stage.refs.map().value();
    let tex_ident = wesl::syntax::Ident::new(format!("base::texture{map}"));
    let sampler_ident = wesl::syntax::Ident::new(format!("base::sampler{map}"));
    let coord_ident = wesl::syntax::Ident::new(format!("in.tex_coord{map}"));

    quote_expression! {
        textureSample(#tex_ident, #sampler_ident, #coord_ident.xy / #coord_ident.z)
    }
}

fn get_color_channel(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.refs.color() {
        ColorChannel::Channel0 => quote_expression! { in.chan0 },
        ColorChannel::Channel1 => quote_expression! { in.chan1 },
        ColorChannel::AlphaBump => quote_expression! { vec4f(base::PLACEHOLDER_RGB, 0f) },
        ColorChannel::AlphaBumpNormalized => {
            quote_expression! { vec4f(base::PLACEHOLDER_RGB, 0f) }
        }
        ColorChannel::Zero => quote_expression! { vec4f(0f) },
        _ => panic!("reserved color channel"),
    }
}

fn get_color_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.color_const {
        Constant::One => quote_expression! { vec4f(1f) },
        Constant::SevenEights => quote_expression! { vec4f(7f / 8f) },
        Constant::SixEights => quote_expression! { vec4f(6f / 8f) },
        Constant::FiveEights => quote_expression! { vec4f(5f / 8f) },
        Constant::FourEights => quote_expression! { vec4f(4f / 8f) },
        Constant::ThreeEights => quote_expression! { vec4f(3f / 8f) },
        Constant::TwoEights => quote_expression! { vec4f(2f / 8f) },
        Constant::OneEight => quote_expression! { vec4f(1f / 8f) },
        Constant::Const0 => quote_expression! { consts[R0] },
        Constant::Const1 => quote_expression! { consts[R1] },
        Constant::Const2 => quote_expression! { consts[R2] },
        Constant::Const3 => quote_expression! { consts[R3] },
        Constant::Const0R => quote_expression! { consts[R0].rrrr },
        Constant::Const1R => quote_expression! { consts[R1].rrrr },
        Constant::Const2R => quote_expression! { consts[R2].rrrr },
        Constant::Const3R => quote_expression! { consts[R3].rrrr },
        Constant::Const0G => quote_expression! { consts[R0].gggg },
        Constant::Const1G => quote_expression! { consts[R1].gggg },
        Constant::Const2G => quote_expression! { consts[R2].gggg },
        Constant::Const3G => quote_expression! { consts[R3].gggg },
        Constant::Const0B => quote_expression! { consts[R0].bbbb },
        Constant::Const1B => quote_expression! { consts[R1].bbbb },
        Constant::Const2B => quote_expression! { consts[R2].bbbb },
        Constant::Const3B => quote_expression! { consts[R3].bbbb },
        Constant::Const0A => quote_expression! { consts[R0].aaaa },
        Constant::Const1A => quote_expression! { consts[R1].aaaa },
        Constant::Const2A => quote_expression! { consts[R2].aaaa },
        Constant::Const3A => quote_expression! { consts[R3].aaaa },
        _ => panic!("reserved color constant"),
    }
}

fn get_color_input(stage: &TexEnvStage, input: ColorInputSrc) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match input {
        ColorInputSrc::R3Color => quote_expression! { regs[R3].rgb },
        ColorInputSrc::R3Alpha => quote_expression! { regs[R3].aaa },
        ColorInputSrc::R0Color => quote_expression! { regs[R0].rgb },
        ColorInputSrc::R0Alpha => quote_expression! { regs[R0].aaa },
        ColorInputSrc::R1Color => quote_expression! { regs[R1].rgb },
        ColorInputSrc::R1Alpha => quote_expression! { regs[R1].aaa },
        ColorInputSrc::R2Color => quote_expression! { regs[R2].rgb },
        ColorInputSrc::R2Alpha => quote_expression! { regs[R2].aaa },
        ColorInputSrc::TexColor => {
            let tex = sample_tex(stage);
            quote_expression! { #tex.rgb }
        }
        ColorInputSrc::TexAlpha => {
            let tex = sample_tex(stage);
            quote_expression! { #tex.aaa }
        }
        ColorInputSrc::RasterColor => {
            let color = get_color_channel(stage);
            quote_expression! { #color.rgb }
        }
        ColorInputSrc::RasterAlpha => {
            let color = get_color_channel(stage);
            quote_expression! { #color.aaa }
        }
        ColorInputSrc::One => quote_expression! { vec3f(1f) },
        ColorInputSrc::Half => quote_expression! { vec3f(0.5f) },
        ColorInputSrc::Constant => {
            let constant = get_color_const(stage);
            quote_expression! { #constant.rgb }
        }
        ColorInputSrc::Zero => quote_expression! { vec3f(0f) },
    }
}

fn get_compare_target(
    input: wesl::syntax::Expression,
    target: CompareTarget,
    alpha: bool,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    match target {
        CompareTarget::R8 => quote_expression! { (#input).r },
        CompareTarget::GR16 => quote_expression! { pack4xU8(vec3u((#input).r, (#input).g, 0, 0)) },
        CompareTarget::BGR16 => {
            quote_expression! { pack4xU8(vec3u((#input).r, (#input).g, (#input).b, 0)) }
        }
        CompareTarget::Component => {
            if alpha {
                quote_expression! { (#input).a }
            } else {
                quote_expression! { (#input).rgb }
            }
        }
    }
}

fn comparative_color_stage(stage: &TexEnvStage) -> wesl::syntax::Statement {
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
        quote_expression! { color_add_mul }
    } else {
        quote_expression! { clamp(color_add_mul, vec3f(0f), vec3f(1f)) }
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

fn regular_color_stage(stage: &TexEnvStage) -> wesl::syntax::Statement {
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
        quote_expression! { color_add_mul }
    } else {
        quote_expression! { clamp(color_add_mul, vec3f(0f), vec3f(1f)) }
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

pub fn color_stage(stage: &TexEnvStage) -> wesl::syntax::Statement {
    if stage.ops.color.bias() == Bias::Comparative {
        comparative_color_stage(stage)
    } else {
        regular_color_stage(stage)
    }
}

fn get_alpha_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.alpha_const {
        Constant::One => quote_expression! { 1f },
        Constant::SevenEights => quote_expression! { (7f / 8f) },
        Constant::SixEights => quote_expression! { (6f / 8f) },
        Constant::FiveEights => quote_expression! { (5f / 8f) },
        Constant::FourEights => quote_expression! { (4f / 8f) },
        Constant::ThreeEights => quote_expression! { (3f / 8f) },
        Constant::TwoEights => quote_expression! { (2f / 8f) },
        Constant::OneEight => quote_expression! { (1f / 8f) },
        Constant::Const0 => quote_expression! { consts[R0].a },
        Constant::Const1 => quote_expression! { consts[R1].a },
        Constant::Const2 => quote_expression! { consts[R2].a },
        Constant::Const3 => quote_expression! { consts[R3].a },
        Constant::Const0R => quote_expression! { consts[R0].r },
        Constant::Const1R => quote_expression! { consts[R1].r },
        Constant::Const2R => quote_expression! { consts[R2].r },
        Constant::Const3R => quote_expression! { consts[R3].r },
        Constant::Const0G => quote_expression! { consts[R0].g },
        Constant::Const1G => quote_expression! { consts[R1].g },
        Constant::Const2G => quote_expression! { consts[R2].g },
        Constant::Const3G => quote_expression! { consts[R3].g },
        Constant::Const0B => quote_expression! { consts[R0].b },
        Constant::Const1B => quote_expression! { consts[R1].b },
        Constant::Const2B => quote_expression! { consts[R2].b },
        Constant::Const3B => quote_expression! { consts[R3].b },
        Constant::Const0A => quote_expression! { consts[R0].a },
        Constant::Const1A => quote_expression! { consts[R1].a },
        Constant::Const2A => quote_expression! { consts[R2].a },
        Constant::Const3A => quote_expression! { consts[R3].a },
        _ => panic!("reserved alpha constant"),
    }
}

fn get_alpha_input(stage: &TexEnvStage, input: AlphaInputSrc) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match input {
        AlphaInputSrc::R3Alpha => quote_expression! { regs[R3].a },
        AlphaInputSrc::R0Alpha => quote_expression! { regs[R0].a },
        AlphaInputSrc::R1Alpha => quote_expression! { regs[R1].a },
        AlphaInputSrc::R2Alpha => quote_expression! { regs[R2].a },
        AlphaInputSrc::TexAlpha => {
            let tex = sample_tex(stage);
            quote_expression! { #tex.a }
        }
        AlphaInputSrc::RasterAlpha => {
            let color = get_color_channel(stage);
            quote_expression! { #color.a }
        }
        AlphaInputSrc::Constant => {
            let constant = get_alpha_const(stage);
            quote_expression! { (#constant) }
        }
        AlphaInputSrc::Zero => quote_expression! { 0f },
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
        quote_expression! { alpha_add_mul }
    } else {
        quote_expression! { clamp(alpha_add_mul, 0f, 1f) }
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
        AlphaCompare::Never => quote_expression! { false },
        AlphaCompare::Less => quote_expression! { alpha < #alpha_ref },
        AlphaCompare::Equal => quote_expression! { alpha == #alpha_ref },
        AlphaCompare::LessOrEqual => quote_expression! { alpha <= #alpha_ref },
        AlphaCompare::Greater => quote_expression! { alpha > #alpha_ref },
        AlphaCompare::NotEqual => quote_expression! { alpha != #alpha_ref },
        AlphaCompare::GreaterOrEqual => quote_expression! { alpha >= #alpha_ref },
        AlphaCompare::Always => quote_expression! { true },
    }
}

pub fn get_alpha_comparison(settings: &AlphaFunctionSettings) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    let a = get_alpha_comparison_helper(settings.comparison[0], 0);
    let b = get_alpha_comparison_helper(settings.comparison[1], 1);

    match settings.logic {
        AlphaLogic::And => quote_expression! { (#a) && (#b) },
        AlphaLogic::Or => quote_expression! { (#a) || (#b) },
        AlphaLogic::Xor => quote_expression! { (#a) ^ (#b) },
        AlphaLogic::Xnor => quote_expression! { !((#a) ^ (#b)) },
    }
}
