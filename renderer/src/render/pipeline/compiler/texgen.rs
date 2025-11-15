use glam::Mat4;
use hemisphere::system::gpu::transform::{
    TexGenInputKind, TexGenKind, TexGenOutputKind, TexGenSource,
};

pub fn get_source(source: TexGenSource) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match source {
        TexGenSource::Position => wesl::quote_expression! { vertex.position },
        TexGenSource::Normal => wesl::quote_expression! { vertex.normal },
        TexGenSource::Color => todo!(),
        TexGenSource::TexCoord0 => wesl::quote_expression! { vec3f(vertex.tex_coord[0], 0.0) },
        TexGenSource::TexCoord1 => wesl::quote_expression! { vec3f(vertex.tex_coord[1], 0.0) },
        TexGenSource::TexCoord2 => wesl::quote_expression! { vec3f(vertex.tex_coord[2], 0.0) },
        TexGenSource::TexCoord3 => wesl::quote_expression! { vec3f(vertex.tex_coord[3], 0.0) },
        TexGenSource::TexCoord4 => wesl::quote_expression! { vec3f(vertex.tex_coord[4], 0.0) },
        TexGenSource::TexCoord5 => wesl::quote_expression! { vec3f(vertex.tex_coord[5], 0.0) },
        TexGenSource::TexCoord6 => wesl::quote_expression! { vec3f(vertex.tex_coord[6], 0.0) },
        TexGenSource::TexCoord7 => wesl::quote_expression! { vec3f(vertex.tex_coord[7], 0.0) },
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
        TexGenInputKind::AB11 => wesl::quote_expression! { vec4f(#source.xy, 1.0, 1.0) },
        TexGenInputKind::ABC1 => wesl::quote_expression! { vec4f(#source, 1.0) },
    }
}

pub fn transform(kind: TexGenKind, input: wesl::syntax::Expression) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match kind {
        TexGenKind::Transform => wesl::quote_expression! { (matrix * #input).xyz },
        TexGenKind::Emboss => todo!(),
        TexGenKind::ColorDiffuse => todo!(),
        TexGenKind::ColorSpecular => todo!(),
    }
}

pub fn get_output(
    format: TexGenOutputKind,
    transformed: wesl::syntax::Expression,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    match format {
        TexGenOutputKind::Vec2 => wesl::quote_expression! { vec3f(#transformed.xy, 0.0) },
        TexGenOutputKind::Vec3 => transformed,
    }
}

pub fn normalize(normalize: bool, output: wesl::syntax::Expression) -> wesl::syntax::Expression {
    use wesl::syntax::*;
    if normalize {
        wesl::quote_expression! { normalize(#output) }
    } else {
        output
    }
}

pub fn post_transform(
    matrix: &Mat4,
    normalized: wesl::syntax::Expression,
) -> wesl::syntax::Expression {
    use wesl::syntax::*;

    let [
        [m00, m01, m02, m03],
        [m10, m11, m12, m13],
        [m20, m21, m22, m23],
        [m30, m31, m32, m33],
    ] = matrix.to_cols_array_2d();

    let matrix = wesl::quote_expression! { mat4x4f(
        vec4f(#m00, #m01, #m02, #m03),
        vec4f(#m10, #m11, #m12, #m13),
        vec4f(#m20, #m21, #m22, #m23),
        vec4f(#m30, #m31, #m32, #m33),
    ) };

    wesl::quote_expression! { (#matrix * vec4f(#normalized, 1.0)).xyz }
}
