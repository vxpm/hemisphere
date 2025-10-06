pub mod command;

use crate::system::System;
use command::Command;
use common::util::DataStream;
use strum::FromRepr;

/// A command processor register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum CpReg {
    Unknown = 0x20,

    TexMatIndexA = 0x30,
    TexMatIndexB = 0x40,

    // VCD
    VcdLow = 0x50,
    VcdHigh = 0x60,

    // VAT
    Vat0Format0 = 0x70,
    Vat0Format1 = 0x71,
    Vat0Format2 = 0x72,
    Vat0Format3 = 0x73,
    Vat0Format4 = 0x74,
    Vat0Format5 = 0x75,
    Vat0Format6 = 0x76,
    Vat0Format7 = 0x77,

    Vat1Format0 = 0x80,
    Vat1Format1 = 0x81,
    Vat1Format2 = 0x82,
    Vat1Format3 = 0x83,
    Vat1Format4 = 0x84,
    Vat1Format5 = 0x85,
    Vat1Format6 = 0x86,
    Vat1Format7 = 0x87,

    Vat2Format0 = 0x90,
    Vat2Format1 = 0x91,
    Vat2Format2 = 0x92,
    Vat2Format3 = 0x93,
    Vat2Format4 = 0x94,
    Vat2Format5 = 0x95,
    Vat2Format6 = 0x96,
    Vat2Format7 = 0x97,

    // Array Base
    VerticesPtr = 0xA0,
    NormalsPtr = 0xA1,
    Color0Ptr = 0xA2,
    Color1Ptr = 0xA3,
    Tex0CoordPtr = 0xA4,
    Tex1CoordPtr = 0xA5,
    Tex2CoordPtr = 0xA6,
    Tex3CoordPtr = 0xA7,
    Tex4CoordPtr = 0xA8,
    Tex5CoordPtr = 0xA9,
    Tex6CoordPtr = 0xAA,
    Tex7CoordPtr = 0xAB,
    GpArr0Ptr = 0xAC,
    GpArr1Ptr = 0xAD,
    GpArr2Ptr = 0xAE,
    GpArr3Ptr = 0xAF,

    // Array Stride
    VerticesStride = 0xB0,
    NormalsStride = 0xB1,
    Color0Stride = 0xB2,
    Color1Stride = 0xB3,
    Tex0CoordStride = 0xB4,
    Tex1CoordStride = 0xB5,
    Tex2CoordStride = 0xB6,
    Tex3CoordStride = 0xB7,
    Tex4CoordStride = 0xB8,
    Tex5CoordStride = 0xB9,
    Tex6CoordStride = 0xBA,
    Tex7CoordStride = 0xBB,
    GpArr0Stride = 0xBC,
    GpArr1Stride = 0xBD,
    GpArr2Stride = 0xBE,
    GpArr3Stride = 0xBF,
}

/// A bypass register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum BypassReg {
    GenMode = 0x00,
    GenFilter0 = 0x01,
    GenFilter1 = 0x02,
    GenFilter2 = 0x03,
    GenFilter3 = 0x04,
    BumpIMask = 0x0F,

    IndirectCmd0 = 0x10,
    IndirectCmd1 = 0x11,
    IndirectCmd2 = 0x12,
    IndirectCmd3 = 0x13,
    IndirectCmd4 = 0x14,
    IndirectCmd5 = 0x15,
    IndirectCmd6 = 0x16,
    IndirectCmd7 = 0x17,
    IndirectCmd8 = 0x18,
    IndirectCmd9 = 0x19,
    IndirectCmd10 = 0x1A,
    IndirectCmd11 = 0x1B,
    IndirectCmd12 = 0x1C,
    IndirectCmd13 = 0x1D,
    IndirectCmd14 = 0x1E,
    IndirectCmd15 = 0x1F,

    ScissorTopLeft = 0x20,
    ScissorBottomRight = 0x21,

    // Setup Unit and Rasterizer
    SetupLpSize = 0x22,
    SetupPerf = 0x23,
    RasterizerPerf = 0x24,
    RasterizerSs0 = 0x25,
    RasterizerSs1 = 0x26,

    RasterizerTexRef0 = 0x28,
    RasterizerTexRef1 = 0x29,
    RasterizerTexRef2 = 0x2A,
    RasterizerTexRef3 = 0x2B,
    RasterizerTexRef4 = 0x2C,
    RasterizerTexRef5 = 0x2D,
    RasterizerTexRef6 = 0x2E,
    RasterizerTexRef7 = 0x2F,

    SetupSsize0 = 0x30,
    SetupSsize1 = 0x32,
    SetupSsize2 = 0x34,
    SetupSsize3 = 0x36,
    SetupSsize4 = 0x38,
    SetupSsize5 = 0x3A,
    SetupSsize6 = 0x3C,
    SetupSsize7 = 0x3E,

    // Pixel Engine
    PixelZMode = 0x40,
    PixelMode0 = 0x41,
    PixelMode1 = 0x42,
    PixelControl = 0x43,
    PixelFieldMask = 0x44,
    PixelDone = 0x45,
    PixelRefresh = 0x46,
    PixelCopySrc = 0x49,
    PixelCopySrcSize = 0x4A,
    PixelCopyDstBase0 = 0x4B,
    PixelCopyDstBase1 = 0x4C,
    PixelCopyDstStride = 0x4D,
    PixelCopyScale = 0x4E,
    PixelCopyClearAr = 0x4F,
    PixelCopyClearGb = 0x50,
    PixelCopyClearZ = 0x51,
    PixelCopyCmd = 0x52,
    PixelCopyFilter0 = 0x53,
    PixelCopyFilter1 = 0x54,
    PixelXBound = 0x55,
    PixelYBound = 0x56,
    PixelPerfMode = 0x57,
    PixelChicken = 0x58,
    ScissorOffset = 0x59,

    // TX
    TxInvTags = 0x66,
    TxPerfMode = 0x67,
    TxFieldMode = 0x68,
    TxRefresh = 0x69,
    TxSetImage1I0 = 0x8C,
    TxSetImage1I1 = 0x8D,
    TxSetImage1I2 = 0x8E,
    TxSetImage1I3 = 0x8F,

    TxSetImage2I0 = 0x90,
    TxSetImage2I1 = 0x91,
    TxSetImage2I2 = 0x92,
    TxSetImage2I3 = 0x93,

    TxSetImage3I0 = 0x94,
    TxSetImage3I1 = 0x95,
    TxSetImage3I2 = 0x96,
    TxSetImage3I3 = 0x97,

    TxSetImage1I4 = 0xAC,
    TxSetImage1I5 = 0xAD,
    TxSetImage1I6 = 0xAE,
    TxSetImage1I7 = 0xAF,

    TxSetImage2I4 = 0xB0,
    TxSetImage2I5 = 0xB1,
    TxSetImage2I6 = 0xB2,
    TxSetImage2I7 = 0xB3,

    TxSetImage3I4 = 0xB4,
    TxSetImage3I5 = 0xB5,
    TxSetImage3I6 = 0xB6,
    TxSetImage3I7 = 0xB7,

    // TEV
    TevColor0 = 0xC0,
    TevAlpha0 = 0xC1,
    TevColor1 = 0xC2,
    TevAlpha1 = 0xC3,
    TevColor2 = 0xC4,
    TevAlpha2 = 0xC5,
    TevColor3 = 0xC6,
    TevAlpha3 = 0xC7,
    TevColor4 = 0xC8,
    TevAlpha4 = 0xC9,
    TevColor5 = 0xCA,
    TevAlpha5 = 0xCB,
    TevColor6 = 0xCC,
    TevAlpha6 = 0xCD,
    TevColor7 = 0xCE,
    TevAlpha7 = 0xCF,
    TevColor8 = 0xD0,
    TevAlpha8 = 0xD1,
    TevColor9 = 0xD2,
    TevAlpha9 = 0xD3,
    TevColor10 = 0xD4,
    TevAlpha10 = 0xD5,
    TevColor11 = 0xD6,
    TevAlpha11 = 0xD7,
    TevColor12 = 0xD8,
    TevAlpha12 = 0xD9,
    TevColor13 = 0xDA,
    TevAlpha13 = 0xDB,
    TevColor14 = 0xDC,
    TevAlpha14 = 0xDD,
    TevColor15 = 0xDE,
    TevAlpha15 = 0xDF,

    TevFogRange = 0xE8,
    TevFog0 = 0xEE,
    TevFog1 = 0xEF,
    TevFog2 = 0xF0,
    TevFog3 = 0xF1,
    TevFogColor = 0xF2,

    TevAlphaFunc = 0xF3,
    TevZ0 = 0xF4,
    TevZ1 = 0xF5,
    TevKSel0 = 0xF6,
    TevKSel1 = 0xF7,
    TevKSel2 = 0xF8,
    TevKSel3 = 0xF9,
    TevKSel4 = 0xFA,
    TevKSel5 = 0xFB,
    TevKSel6 = 0xFC,
    TevKSel7 = 0xFD,

    // BP
    BypassMask = 0xFE,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub command: command::Interface,
    pub command_queue: DataStream,
}

/// GX subsystem
impl System {
    /// Pops a value from the CP FIFO in memory.
    fn cp_fifo_pop(&mut self) -> u8 {
        assert!(self.bus.gpu.command.fifo_count > 0);

        let data = self.read::<u8>(self.bus.gpu.command.fifo_read_ptr);
        self.bus.gpu.command.fifo_pop();

        data
    }

    pub fn cp_process(&mut self) {
        loop {
            if self.bus.gpu.command_queue.is_empty() {
                return;
            }

            let Some(cmd) = Command::read(&mut self.bus.gpu.command_queue) else {
                return;
            };

            tracing::debug!("{:02X?}", cmd);
        }
    }

    /// Consumes commands available in the CP FIFO and processes them.
    pub fn cp_update(&mut self) {
        while self.bus.gpu.command.fifo_count > 0 {
            let data = self.cp_fifo_pop();
            self.bus.gpu.command_queue.push_be(data);
        }

        self.cp_process();
    }
}
