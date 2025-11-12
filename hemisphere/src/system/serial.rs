use bitos::{
    bitos,
    integer::{u2, u6, u7, u10},
};

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Poll {
    #[bits(0..4)]
    pub copy_mode: [bool; 4],
    #[bits(4..8)]
    pub port_enable: [bool; 4],
    #[bits(8..16)]
    pub poll_per_frame: u8,
    #[bits(16..26)]
    pub x_lines: u10,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CommControl {
    #[bits(0)]
    pub transfer_start: bool,
    #[bits(1..3)]
    pub channel: u2,
    #[bits(6)]
    pub enable_callback: bool,
    #[bits(7)]
    pub enable_command: bool,
    #[bits(8..15)]
    pub input_length: u7,
    #[bits(16..23)]
    pub output_length: u7,
    #[bits(24)]
    pub enable_channel: bool,
    #[bits(25..27)]
    pub channel_number: u2,

    #[bits(27)]
    pub read_interrupt_mask: bool,
    #[bits(28)]
    pub read_interrupt: bool,
    #[bits(29)]
    pub communication_error: bool,
    #[bits(30)]
    pub transfer_interrupt_mask: bool,
    #[bits(31)]
    pub transfer_interrupt: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    #[bits(0..6)]
    pub channel3: u6,
    #[bits(8..14)]
    pub channel2: u6,
    #[bits(16..22)]
    pub channel1: u6,

    #[bits(24)]
    pub underrun_channel0: bool,
    #[bits(25)]
    pub overrun_channel0: bool,
    #[bits(26)]
    pub collision_channel0: bool,
    #[bits(27)]
    pub no_response_channel0: bool,
    #[bits(28)]
    pub buffer_not_copied: bool,
    #[bits(29)]
    pub data_empty: bool,
    #[bits(31)]
    pub idk_really: bool,
}

#[derive(Default)]
pub struct Interface {
    pub poll: Poll,
    pub comm_control: CommControl,
    pub status: Status,
}
