use hemisphere::render::{TexEnvConfig, TexGenConfig};
use wesl::{VirtualResolver, Wesl};

fn base_module() -> wesl::syntax::TranslationUnit {
    use wesl::syntax::*;
    wesl::quote_module! {
        alias MatIdx = u32;

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
            @location(0) diffuse_color: vec4f,
            @location(1) specular_color: vec4f,
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

            out.diffuse_color = vertex.diffuse;
            out.specular_color = vertex.specular;

            var tex_coords: array<vec3f, 8>;
            // for (var i = 0u; i < config.texgen.count; i ++) {
            //     let texgen = config.texgen.texgens[i];
            //     let matrix = matrices[vertex.tex_coord_mat[i]];
            //     tex_coords[i] = texgen::compute(vertex, texgen, matrix);
            // }

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

fn fragment_stage() -> wesl::syntax::GlobalDeclaration {
    use wesl::syntax::*;
    wesl::quote_declaration! {
        @fragment
        fn fs_main(in: base::VertexOutput) -> @location(0) vec4f {
            return in.diffuse_color;
        }
    }
}

fn main_module(texenv: &TexEnvConfig, texgen: &TexGenConfig) -> wesl::syntax::TranslationUnit {
    use wesl::syntax::*;

    let vertex = vertex_stage(texgen);
    let fragment = fragment_stage();

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

    compiled.syntax.to_string()
}
