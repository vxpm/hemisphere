use hemisphere::{
    render::TexEnvStage,
    system::gpu::environment::{AlphaInputSrc, ColorChannel, ColorInputSrc},
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
        ColorChannel::Color0 => wesl::quote_expression! { in.diffuse },
        ColorChannel::Color1 => wesl::quote_expression! { in.specular },
        ColorChannel::Alpha0 => wesl::quote_expression! { in.diffuse.aaaa },
        ColorChannel::Alpha1 => wesl::quote_expression! { in.specular.aaaa },
        ColorChannel::ColorAlpha0 => wesl::quote_expression! { in.diffuse },
        ColorChannel::ColorAlpha1 => wesl::quote_expression! { in.specular },
        ColorChannel::Zero => todo!(),
        ColorChannel::AlphaBump => wesl::quote_expression! { base::PLACEHOLDER_RGBA },
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
        ColorInputSrc::Constant => wesl::quote_expression! { vec3f(0.5) }, // STUB
        ColorInputSrc::Zero => wesl::quote_expression! { vec3f(0.0) },
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
        AlphaInputSrc::Constant => wesl::quote_expression! { 0.5 }, // STUB
        AlphaInputSrc::Zero => wesl::quote_expression! { 0.0 },
    }
}
