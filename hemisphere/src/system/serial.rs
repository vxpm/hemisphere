use crate::system::System;
use bitos::{
    bitos,
    integer::{u2, u6, u7, u10},
};
use strum::FromRepr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
#[repr(u8)]
enum Command {
    Info = 0x00,
    Poll = 0x40,
    GetOrigin = 0x41,
    Calibrate = 0x42,
}

/// Decive polling configuration.
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

pub struct Interface {
    pub poll: Poll,
    pub comm_control: CommControl,
    pub status: Status,
    pub buffer: [u8; 128],
}

impl Interface {
    pub fn any_interrupt(&self) -> bool {
        let transfer =
            self.comm_control.transfer_interrupt() & self.comm_control.transfer_interrupt_mask();
        transfer
    }
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            poll: Default::default(),
            comm_control: Default::default(),
            status: Default::default(),
            buffer: [0; 128],
        }
    }
}

fn do_transfer(sys: &mut System) {
    let mut i = 0;
    let mut read = || {
        let value = sys.serial.buffer[i];
        i += 1;
        value
    };

    dbg!(sys.serial.comm_control);
    tracing::debug!("transfer");

    let cmd = read();
    let Some(cmd) = Command::from_repr(cmd) else {
        todo!("unknown SI command {cmd:?}")
    };

    match cmd {
        Command::Info => {
            tracing::debug!("info");
            assert_eq!(sys.serial.comm_control.output_length().value(), 1);
            assert_eq!(sys.serial.comm_control.input_length().value(), 3);
            sys.serial.buffer[..3].copy_from_slice(&[0x09, 0x00, 0x00]);
        }
        Command::Poll => {
            todo!("si poll")
        }
        Command::GetOrigin => {
            tracing::debug!("get_origin");
            assert_eq!(sys.serial.comm_control.output_length().value(), 1);
            assert_eq!(sys.serial.comm_control.input_length().value(), 10);
            sys.serial.buffer[..10]
                .copy_from_slice(&[0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00]);
        }
        Command::Calibrate => {
            tracing::debug!("calibrate");
            assert_eq!(sys.serial.comm_control.output_length().value(), 3);
            assert_eq!(sys.serial.comm_control.input_length().value(), 10);
            sys.serial.buffer[..10]
                .copy_from_slice(&[0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00]);
        }
    }

    sys.serial.comm_control.set_transfer_start(false);
    sys.serial.comm_control.set_transfer_interrupt(true);
    sys.pi_check_interrupts();
}

pub fn write_comm_control(sys: &mut System, value: CommControl) {
    sys.serial
        .comm_control
        .set_transfer_start(value.transfer_start());
    sys.serial.comm_control.set_channel(value.channel());
    sys.serial
        .comm_control
        .set_enable_callback(value.enable_callback());
    sys.serial
        .comm_control
        .set_enable_command(value.enable_command());
    sys.serial
        .comm_control
        .set_input_length(value.input_length());
    sys.serial
        .comm_control
        .set_output_length(value.output_length());
    sys.serial
        .comm_control
        .set_enable_channel(value.enable_channel());
    sys.serial
        .comm_control
        .set_channel_number(value.channel_number());

    sys.serial
        .comm_control
        .set_read_interrupt_mask(value.read_interrupt_mask());
    sys.serial
        .comm_control
        .set_read_interrupt(sys.serial.comm_control.read_interrupt() & !value.read_interrupt());
    sys.serial
        .comm_control
        .set_transfer_interrupt_mask(value.transfer_interrupt_mask());
    sys.serial.comm_control.set_transfer_interrupt(
        sys.serial.comm_control.transfer_interrupt() & !value.transfer_interrupt(),
    );

    if value.transfer_start() {
        sys.scheduler.schedule(200, do_transfer);
    }
}
