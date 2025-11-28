use hemisphere::{
    render::TexEnvStage,
    system::gpu::environment::{AlphaInputSrc, ColorChannel, ColorInputSrc, Constant},
};

fn sample_tex(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    let map = stage.refs.map().value();
    let tex_ident = wesl::syntax::Ident::new(format!("base::texture{map}"));
    let sampler_ident = wesl::syntax::Ident::new(format!("base::sampler{map}"));
    let coord_ident = wesl::syntax::Ident::new(format!("in.tex_coord{map}"));

    wesl::quote_expression! {
        textureSample(#tex_ident, #sampler_ident, #coord_ident.xy)
    }
}

fn get_color_channel(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.refs.color() {
        ColorChannel::Color0 => wesl::quote_expression! { in.chan0 },
        ColorChannel::Color1 => wesl::quote_expression! { in.chan1 },
        ColorChannel::Alpha0 => wesl::quote_expression! { in.chan0.aaaa },
        ColorChannel::Alpha1 => wesl::quote_expression! { in.chan1.aaaa },
        ColorChannel::ColorAlpha0 => wesl::quote_expression! { in.chan0 },
        ColorChannel::ColorAlpha1 => wesl::quote_expression! { in.chan1 },
        ColorChannel::Zero => todo!(),
        ColorChannel::AlphaBump => wesl::quote_expression! { vec4f(base::PLACEHOLDER_RGB, 0.0) },
    }
}

fn get_color_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.color_const {
        Constant::One => wesl::quote_expression! { vec4f(1.0) },
        Constant::SevenEights => wesl::quote_expression! { vec4f(7.0 / 8.0) },
        Constant::SixEights => wesl::quote_expression! { vec4f(6.0 / 8.0) },
        Constant::FiveEights => wesl::quote_expression! { vec4f(5.0 / 8.0) },
        Constant::FourEights => wesl::quote_expression! { vec4f(4.0 / 8.0) },
        Constant::ThreeEights => wesl::quote_expression! { vec4f(3.0 / 8.0) },
        Constant::TwoEights => wesl::quote_expression! { vec4f(2.0 / 8.0) },
        Constant::OneEight => wesl::quote_expression! { vec4f(1.0 / 8.0) },
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

pub fn get_color_input(stage: &TexEnvStage, input: ColorInputSrc) -> wesl::syntax::Expression {
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
        ColorInputSrc::One => wesl::quote_expression! { vec3f(1.0) },
        ColorInputSrc::Half => wesl::quote_expression! { vec3f(0.5) },
        ColorInputSrc::Constant => {
            let constant = get_color_const(stage);
            wesl::quote_expression! { #constant.rgb }
        }
        ColorInputSrc::Zero => wesl::quote_expression! { vec3f(0.0) },
    }
}

fn get_alpha_const(stage: &TexEnvStage) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match stage.alpha_const {
        Constant::One => wesl::quote_expression! { 1.0 },
        Constant::SevenEights => wesl::quote_expression! { (7.0 / 8.0) },
        Constant::SixEights => wesl::quote_expression! { (6.0 / 8.0) },
        Constant::FiveEights => wesl::quote_expression! { (5.0 / 8.0) },
        Constant::FourEights => wesl::quote_expression! { (4.0 / 8.0) },
        Constant::ThreeEights => wesl::quote_expression! { (3.0 / 8.0) },
        Constant::TwoEights => wesl::quote_expression! { (2.0 / 8.0) },
        Constant::OneEight => wesl::quote_expression! { (1.0 / 8.0) },
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

pub fn get_alpha_input(stage: &TexEnvStage, input: AlphaInputSrc) -> wesl::syntax::Expression {
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
        AlphaInputSrc::Zero => wesl::quote_expression! { 0.0 },
    }
}
