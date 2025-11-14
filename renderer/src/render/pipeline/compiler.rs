mod texenv;
mod texgen;

use hemisphere::render::{TexEnvConfig, TexGenConfig};
use wesl::{VirtualResolver, Wesl};

fn base_module() -> wesl::syntax::TranslationUnit {
    use wesl::syntax::*;
    wesl::quote_module! {
        alias MatIdx = u32;

        const PLACEHOLDER_RGB: vec3f = vec3f(1.0, 0.0, 0.8627);
        const PLACEHOLDER_RGBA: vec4f = vec4f(1.0, 0.0, 0.8627, 0.5);

        // A primitive vertex
        struct Vertex {
            position: vec3f, // 12 bytes
            position_mat: MatIdx, // 4 bytes

            normal: vec3f, // 12 bytes
            normal_mat: MatIdx, // 4 bytes

            diffuse: vec4f, // 16 bytes
            specular: vec4f, // 16 bytes

            tex_coord: array<vec2f, 8>, // 8 * 8 = 64 bytes
            tex_coord_mat: array<MatIdx, 8>, // 4 * 8 = 32 bytes

            projection: MatIdx, // 4 bytes

            // pad to 16 bytes
            _pad0: u32,
            _pad1: u32,
            _pad2: u32,
        };

        // Primitives group
        @group(0) @binding(0) var<storage> matrices: array<mat4x4f>;
        @group(0) @binding(1) var<storage> vertices: array<Vertex>;

        // Textures group
        @group(1) @binding(0) var texture0: texture_2d<f32>;
        @group(1) @binding(1) var sampler0: sampler;
        @group(1) @binding(2) var texture1: texture_2d<f32>;
        @group(1) @binding(3) var sampler1: sampler;
        @group(1) @binding(4) var texture2: texture_2d<f32>;
        @group(1) @binding(5) var sampler2: sampler;
        @group(1) @binding(6) var texture3: texture_2d<f32>;
        @group(1) @binding(7) var sampler3: sampler;

        @group(1) @binding(8) var texture4: texture_2d<f32>;
        @group(1) @binding(9) var sampler4: sampler;
        @group(1) @binding(10) var texture5: texture_2d<f32>;
        @group(1) @binding(11) var sampler5: sampler;
        @group(1) @binding(12) var texture6: texture_2d<f32>;
        @group(1) @binding(13) var sampler6: sampler;
        @group(1) @binding(14) var texture7: texture_2d<f32>;
        @group(1) @binding(15) var sampler7: sampler;

        struct VertexOutput {
            @builtin(position) clip: vec4f,
            @location(0) diffuse: vec4f,
            @location(1) specular: vec4f,
            @location(2) tex_coord0: vec3f,
            @location(3) tex_coord1: vec3f,
            @location(4) tex_coord2: vec3f,
            @location(5) tex_coord3: vec3f,
            @location(6) tex_coord4: vec3f,
            @location(7) tex_coord5: vec3f,
            @location(8) tex_coord6: vec3f,
            @location(9) tex_coord7: vec3f,
        };
    }
}

fn vertex_stage(texgen: &TexGenConfig) -> wesl::syntax::GlobalDeclaration {
    use wesl::syntax::*;

    let mut stages = vec![];
    for (index, stage) in texgen.stages.iter().enumerate() {
        let index = index as u32;

        let source = texgen::get_source(stage.base.source());
        let input = texgen::get_input(stage.base.input_kind(), source);
        let transformed = texgen::transform(stage.base.kind(), input);
        let output = texgen::get_output(stage.base.output_kind(), transformed);
        let normalized = texgen::normalize(stage.normalize, output);
        let result = texgen::post_transform(&stage.post_matrix, normalized);

        stages.push(wesl::quote_statement! {
            {
                let matrix = base::matrices[vertex.tex_coord_mat[#index]];
                tex_coords[#index] = #result;
            }
        });
    }

    stages.resize(16, wesl::quote_statement!({}));
    let [
        s0,
        s1,
        s2,
        s3,
        s4,
        s5,
        s6,
        s7,
        s8,
        s9,
        s10,
        s11,
        s12,
        s13,
        s14,
        s15,
    ] = stages.try_into().unwrap();

    let compute_stages = wesl::quote_statement!({
        @#s0 {}
        @#s1 {}
        @#s2 {}
        @#s3 {}
        @#s4 {}
        @#s5 {}
        @#s6 {}
        @#s7 {}
        @#s8 {}
        @#s9 {}
        @#s10 {}
        @#s11 {}
        @#s12 {}
        @#s13 {}
        @#s14 {}
        @#s15 {}
    });

    wesl::quote_declaration! {
        @vertex
        fn vs_main(@builtin(vertex_index) index: u32) -> base::VertexOutput {
            var out: base::VertexOutput;

            let vertex = base::vertices[index];
            let pos = vec4f(vertex.position, 1.0);
            let projection = base::matrices[vertex.projection];
            let view = base::matrices[vertex.position_mat];
            out.clip = projection * view * pos;
            out.clip.z += out.clip.w;
            out.clip.z /= 2.0;

            out.diffuse = vertex.diffuse;
            out.specular = vertex.specular;

            var tex_coords: array<vec3f, 8>;
            @#compute_stages {}

            out.tex_coord0 = tex_coords[0];
            out.tex_coord1 = tex_coords[1];
            out.tex_coord2 = tex_coords[2];
            out.tex_coord3 = tex_coords[3];
            out.tex_coord4 = tex_coords[4];
            out.tex_coord5 = tex_coords[5];
            out.tex_coord6 = tex_coords[6];
            out.tex_coord7 = tex_coords[7];

            return out;
        }
    }
}

fn fragment_stage(texenv: &TexEnvConfig) -> wesl::syntax::GlobalDeclaration {
    use wesl::syntax::*;

    let constant = |i: usize| {
        let r = texenv.constants[i].r;
        let g = texenv.constants[i].g;
        let b = texenv.constants[i].b;
        let a = texenv.constants[i].a;
        wesl::quote_expression! { vec4f(#r, #g, #b, #a) }
    };

    let const_0 = constant(0);
    let const_1 = constant(1);
    let const_2 = constant(2);
    let const_3 = constant(3);

    let mut stages = vec![];
    for stage in texenv.stages.iter() {
        let input_a = texenv::get_color_input(stage, stage.ops.color.input_a());
        let input_b = texenv::get_color_input(stage, stage.ops.color.input_b());
        let input_c = texenv::get_color_input(stage, stage.ops.color.input_c());
        let input_d = texenv::get_color_input(stage, stage.ops.color.input_d());

        let sign = if stage.ops.color.negate() { -1.0 } else { 1.0 };
        let bias = stage.ops.color.bias().value();
        let scale = stage.ops.color.scale().value();
        let output = stage.ops.color.output() as u32;

        let color_compute = wesl::quote_statement! {
            {
                let color_lerp = #sign * (#input_a * (1.0 - #input_c) + #input_b * #input_c);
                let color_result = #scale * (color_lerp + #input_d + #bias);
                regs[#output] = vec4f(color_result, regs[#output].a);
                last_color_output = #output;
            }
        };

        let input_a = texenv::get_alpha_input(stage, stage.ops.alpha.input_a());
        let input_b = texenv::get_alpha_input(stage, stage.ops.alpha.input_b());
        let input_c = texenv::get_alpha_input(stage, stage.ops.alpha.input_c());
        let input_d = texenv::get_alpha_input(stage, stage.ops.alpha.input_d());

        let sign = if stage.ops.alpha.negate() { -1.0 } else { 1.0 };
        let bias = stage.ops.alpha.bias().value();
        let scale = stage.ops.alpha.scale().value();
        let output = stage.ops.alpha.output() as u32;

        let alpha_compute = wesl::quote_statement! {
            {
                let alpha_lerp = #sign * (#input_a * (1.0 - #input_c) + #input_b * #input_c);
                let alpha_result = #scale * (alpha_lerp + #input_d + #bias);
                regs[#output] = vec4f(regs[#output].rgb, alpha_result);
                last_alpha_output = #output;
            }
        };

        stages.push(wesl::quote_statement! {
            {
                @#color_compute {}
                @#alpha_compute {}
            }
        });
    }

    stages.resize(16, wesl::quote_statement!({}));
    let [
        s0,
        s1,
        s2,
        s3,
        s4,
        s5,
        s6,
        s7,
        s8,
        s9,
        s10,
        s11,
        s12,
        s13,
        s14,
        s15,
    ] = stages.try_into().unwrap();

    let compute_stages = wesl::quote_statement!({
        @#s0 {}
        @#s1 {}
        @#s2 {}
        @#s3 {}
        @#s4 {}
        @#s5 {}
        @#s6 {}
        @#s7 {}
        @#s8 {}
        @#s9 {}
        @#s10 {}
        @#s11 {}
        @#s12 {}
        @#s13 {}
        @#s14 {}
        @#s15 {}
    });

    wesl::quote_declaration! {
        @fragment
        fn fs_main(in: base::VertexOutput) -> @location(0) vec4f {
            const R3: u32 = 0;
            const R0: u32 = 1;
            const R1: u32 = 2;
            const R2: u32 = 3;

            var last_color_output = R3;
            var last_alpha_output = R3;
            var regs: array<vec4f, 4>;

            regs[R0] = #const_0;
            regs[R1] = #const_1;
            regs[R2] = #const_2;
            regs[R3] = #const_3;

            @#compute_stages {}

            return vec4f(regs[last_color_output].rgb, regs[last_alpha_output].a);
        }
    }
}

fn main_module(texenv: &TexEnvConfig, texgen: &TexGenConfig) -> wesl::syntax::TranslationUnit {
    use wesl::syntax::*;

    let vertex = vertex_stage(texgen);
    let fragment = fragment_stage(texenv);

    wesl::quote_module! {
        import package::base;

        const #vertex = 0;
        const #fragment = 0;
    }
}

pub fn compile(texenv: &TexEnvConfig, texgen: &TexGenConfig) -> String {
    let mut resolver = VirtualResolver::new();
    resolver.add_translation_unit("package::base".parse().unwrap(), base_module());
    resolver.add_translation_unit(
        "package::main".parse().unwrap(),
        main_module(texenv, texgen),
    );

    let mut wesl = Wesl::new("shaders").set_custom_resolver(resolver);
    wesl.use_sourcemap(true);
    wesl.set_options(wesl::CompileOptions {
        imports: true,
        condcomp: false,
        generics: false,
        strip: true,
        lower: true,
        validate: true,
        ..Default::default()
    });

    let compiled = match wesl.compile(&"package::main".parse().unwrap()) {
        Ok(ok) => ok,
        Err(e) => {
            panic!("{e}");
        }
    };

    let code = compiled.syntax.to_string();
    if texenv.stages.len() > 1 {
        println!("{}", code);
    }

    code
}
