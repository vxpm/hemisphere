use hemisphere::system::gx::xf::{TexGenInputKind, TexGenKind, TexGenOutputKind, TexGenSource};

pub fn get_source(source: TexGenSource) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match source {
        TexGenSource::Position => wesl_quote::quote_expression! { vertex.position },
        TexGenSource::Normal => wesl_quote::quote_expression! { vertex.normal },
        // TODO: terrible stub
        TexGenSource::Color => wesl_quote::quote_expression! { vertex.chan0 },
        TexGenSource::TexCoord0 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[0], 0.0) },
        TexGenSource::TexCoord1 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[1], 0.0) },
        TexGenSource::TexCoord2 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[2], 0.0) },
        TexGenSource::TexCoord3 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[3], 0.0) },
        TexGenSource::TexCoord4 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[4], 0.0) },
        TexGenSource::TexCoord5 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[5], 0.0) },
        TexGenSource::TexCoord6 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[6], 0.0) },
        TexGenSource::TexCoord7 => wesl_quote::quote_expression! { vec3f(vertex.tex_coord[7], 0.0) },
        TexGenSource::BinormalT => todo!(),
        TexGenSource::BinormalB => todo!(),
        _ => panic!("reserved texgen source"),
    }
}

pub fn get_input(
    format: TexGenInputKind,
    source: wesl::syntax::Expression,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match format {
        TexGenInputKind::AB11 => wesl_quote::quote_expression! { vec4f(#source.xy, 1.0, 1.0) },
        TexGenInputKind::ABC1 => wesl_quote::quote_expression! { vec4f(#source, 1.0) },
    }
}

pub fn transform(kind: TexGenKind, input: wesl::syntax::Expression) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match kind {
        TexGenKind::Transform => wesl_quote::quote_expression! { (matrix * #input).xyz },
        // TODO: terrible stubs
        TexGenKind::Emboss => wesl_quote::quote_expression! { (#input).xyz },
        TexGenKind::ColorDiffuse => wesl_quote::quote_expression! { (#input).xyz },
        TexGenKind::ColorSpecular => todo!(),
    }
}

pub fn get_output(
    format: TexGenOutputKind,
    transformed: wesl::syntax::Expression,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match format {
        TexGenOutputKind::Vec2 => wesl_quote::quote_expression! { vec3f(#transformed.xy, 1.0) },
        TexGenOutputKind::Vec3 => transformed,
    }
}

pub fn normalize(normalize: bool, output: wesl::syntax::Expression) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    if normalize {
        wesl_quote::quote_expression! { normalize(#output) }
    } else {
        output
    }
}

pub fn post_transform(
    stage_index: u32,
    normalized: wesl::syntax::Expression,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    wesl_quote::quote_expression! { (config.post_transform_mat[#stage_index] * vec4f(#normalized, 1.0)).xyz }
}
