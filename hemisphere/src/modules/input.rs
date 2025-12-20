#[derive(Debug, Clone, Copy)]
pub struct ControllerState {
    // Analog
    pub analog_x: u8,
    pub analog_y: u8,
    pub analog_sub_x: u8,
    pub analog_sub_y: u8,

    // Analog Triggers
    pub analog_trigger_left: u8,
    pub analog_trigger_right: u8,

    // Digital Triggers
    pub trigger_z: bool,
    pub trigger_left: bool,
    pub trigger_right: bool,

    // Pad
    pub pad_left: bool,
    pub pad_right: bool,
    pub pad_down: bool,
    pub pad_up: bool,

    // Buttons
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,
    pub button_start: bool,
}

/// Trait for controller modules.
pub trait InputModule: Send {
    fn controller(&mut self, index: usize) -> Option<ControllerState>;
}

/// An implementation of [`InputModule`] which does nothing: every controller is always
/// disconnected.
#[derive(Debug, Clone, Copy)]
pub struct NopInputModule;

impl InputModule for NopInputModule {
    fn controller(&mut self, _: usize) -> Option<ControllerState> {
        None
    }
}
