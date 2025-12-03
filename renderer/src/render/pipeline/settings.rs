use hemisphere::{
    render::TexEnvStage,
    system::gx::{
        tev::{AlphaCompare, AlphaLogic},
        transform::BaseTexGen,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlendSettings {
    pub enabled: bool,
    pub src: wgpu::BlendFactor,
    pub dst: wgpu::BlendFactor,
    pub op: wgpu::BlendOperation,

    pub color_write: bool,
    pub alpha_write: bool,
}

impl Default for BlendSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            src: wgpu::BlendFactor::Src,
            dst: wgpu::BlendFactor::Dst,
            op: wgpu::BlendOperation::Add,

            color_write: true,
            alpha_write: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DepthSettings {
    pub enabled: bool,
    pub compare: wgpu::CompareFunction,
    pub write: bool,
}

impl Default for DepthSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            compare: wgpu::CompareFunction::Less,
            write: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AlphaFunctionSettings {
    pub comparison: [AlphaCompare; 2],
    pub logic: AlphaLogic,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TexEnvSettings {
    pub stages: Vec<TexEnvStage>,
    pub alpha_function: AlphaFunctionSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TexGenStageSettings {
    pub base: BaseTexGen,
    pub normalize: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TexGenSettings {
    pub stages: Vec<TexGenStageSettings>,
}
