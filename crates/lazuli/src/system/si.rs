//! Serial interface (SI).
use crate::system::{System, pi};
use bitos::{
    BitUtils, bitos,
    integer::{u2, u7, u10},
};
use strum::FromRepr;
use zerocopy::IntoBytes;

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

#[bitos(6)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelStatus {
    #[bits(0)]
    pub underrun: bool,
    #[bits(1)]
    pub overrun: bool,
    #[bits(2)]
    pub collision: bool,
    #[bits(3)]
    pub no_response: bool,
    /// Whether the output buffer has not yet been copied.
    #[bits(4)]
    pub output_not_copied: bool,
    /// Whether the input buffer has new data.
    #[bits(5)]
    pub input_ready: bool,
}

#[bitos(32)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Status {
    #[bits(0..6)]
    pub channel3: ChannelStatus,
    #[bits(8..14)]
    pub channel2: ChannelStatus,
    #[bits(16..22)]
    pub channel1: ChannelStatus,
    #[bits(24..30)]
    pub channel0: ChannelStatus,
    #[bits(31)]
    pub copy_buffers: bool,
}

impl Status {
    pub fn channel(&self, n: usize) -> ChannelStatus {
        match n {
            0 => self.channel0(),
            1 => self.channel1(),
            2 => self.channel2(),
            3 => self.channel3(),
            _ => panic!("out of range channel"),
        }
    }

    pub fn set_channel(&mut self, n: usize, value: ChannelStatus) {
        match n {
            0 => self.set_channel0(value),
            1 => self.set_channel1(value),
            2 => self.set_channel2(value),
            3 => self.set_channel3(value),
            _ => panic!("out of range channel"),
        };
    }
}

#[derive(Clone, Copy, Default)]
pub struct ChannelOutput {
    pub data: u32,
    pub dirty: bool,
}

#[derive(Clone, Copy, Default)]
pub struct ChannelInput {
    pub low: u32,
    pub high: u32,
}

pub struct Interface {
    pub channel_output: [ChannelOutput; 4],
    pub channel_input: [ChannelInput; 4],
    pub poll: Poll,
    pub comm_control: CommControl,
    pub status: Status,
    pub buffer: [u8; 128],
}

impl Interface {
    pub fn any_interrupt(&self) -> bool {
        let read = self.comm_control.read_interrupt() && self.comm_control.read_interrupt_mask();
        let transfer =
            self.comm_control.transfer_interrupt() && self.comm_control.transfer_interrupt_mask();

        read || transfer
    }
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            channel_output: [Default::default(); 4],
            channel_input: [Default::default(); 4],
            poll: Default::default(),
            comm_control: Default::default(),
            status: Default::default(),
            buffer: [0; 128],
        }
    }
}

#[bitos(64)]
struct StandardController {
    #[bits(0..8)]
    pub analog_y: u8,
    #[bits(8..16)]
    pub analog_x: u8,
    #[bits(16)]
    pub pad_left: bool,
    #[bits(17)]
    pub pad_right: bool,
    #[bits(18)]
    pub pad_down: bool,
    #[bits(19)]
    pub pad_up: bool,
    #[bits(20)]
    pub trigger_z: bool,
    #[bits(21)]
    pub trigger_right: bool,
    #[bits(22)]
    pub trigger_left: bool,
    #[bits(24)]
    pub button_a: bool,
    #[bits(25)]
    pub button_b: bool,
    #[bits(26)]
    pub button_x: bool,
    #[bits(27)]
    pub button_y: bool,
    #[bits(28)]
    pub button_start: bool,

    #[bits(32..40)]
    pub analog_trigger_right: u8,
    #[bits(40..48)]
    pub analog_trigger_left: u8,
    #[bits(48..56)]
    pub analog_sub_y: u8,
    #[bits(56..64)]
    pub analog_sub_x: u8,
}

pub fn poll_controller(sys: &mut System, channel: usize) {
    if !sys.serial.poll.port_enable_at(channel).unwrap() {
        return;
    }

    let Some(controller) = sys.modules.input.controller(channel) else {
        return;
    };

    let data = StandardController::from_bits(0)
        .with_analog_y(controller.analog_y)
        .with_analog_x(controller.analog_x)
        .with_pad_left(controller.pad_left)
        .with_pad_right(controller.pad_right)
        .with_pad_down(controller.pad_down)
        .with_pad_up(controller.pad_up)
        .with_trigger_z(controller.trigger_z)
        .with_trigger_right(controller.trigger_right)
        .with_trigger_left(controller.trigger_left)
        .with_button_a(controller.button_a)
        .with_button_b(controller.button_b)
        .with_button_x(controller.button_x)
        .with_button_y(controller.button_y)
        .with_button_start(controller.button_start)
        .with_analog_trigger_right(controller.analog_trigger_right)
        .with_analog_trigger_left(controller.analog_trigger_left)
        .with_analog_sub_y(controller.analog_sub_y)
        .with_analog_sub_x(controller.analog_sub_x)
        .to_bits();

    sys.serial.channel_input[channel].low = data.bits(32, 64) as u32;
    sys.serial.channel_input[channel].high = data.bits(0, 32) as u32;

    let mut status = sys.serial.status.channel(channel);
    status.set_input_ready(true);
    sys.serial.status.set_channel(channel, status);
    sys.serial.comm_control.set_read_interrupt(true);
}

fn process_cmd(sys: &mut System, channel: usize) {
    let mut i = 0;
    let mut read = || {
        let value = sys.serial.buffer[i];
        i += 1;
        value
    };

    let cmd = read();
    let Some(cmd) = Command::from_repr(cmd) else {
        todo!("unknown SI command {cmd:?}")
    };

    match cmd {
        Command::Info => {
            tracing::debug!("info");
            sys.serial.buffer[..3].copy_from_slice(&[0x09, 0x00, 0x00]);
        }
        Command::Poll => {
            tracing::debug!("poll");
            self::poll_controller(sys, channel);
        }
        Command::GetOrigin => {
            tracing::debug!("get_origin");
            sys.serial.buffer[..10]
                .copy_from_slice(&[0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00]);
        }
        Command::Calibrate => {
            tracing::debug!("calibrate");
            sys.serial.buffer[..10]
                .copy_from_slice(&[0x00, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00, 0x00, 0x00]);
        }
    }
}

fn do_transfer(sys: &mut System) {
    // dbg!(sys.serial.comm_control);
    tracing::debug!("transfer");

    process_cmd(sys, sys.serial.comm_control.channel().value() as usize);

    sys.serial.comm_control.set_transfer_start(false);
    sys.serial.comm_control.set_transfer_interrupt(true);
    pi::check_interrupts(sys);
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

pub fn write_status(sys: &mut System, value: Status) {
    if value.copy_buffers() {
        for channel in 0..4 {
            if std::mem::take(&mut sys.serial.channel_output[channel].dirty) {
                sys.serial.buffer[..3].copy_from_slice(
                    &sys.serial.channel_output[channel].data.to_be().as_bytes()[1..4],
                );

                process_cmd(sys, channel);
            }
        }
    }
}
