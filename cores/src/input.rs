use gilrs::{Axis, Button, Event, GamepadId, Gilrs};
use hemisphere::cores::{ControllerState, InputCore};

pub struct GilrsCore {
    gilrs: Gilrs,
    active_gamepad: Option<GamepadId>,
}

impl GilrsCore {
    pub fn new() -> Self {
        Self {
            gilrs: Gilrs::new().unwrap(),
            active_gamepad: None,
        }
    }

    fn process_events(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            dbg!(event);
            if self.active_gamepad.is_none() {
                self.active_gamepad = Some(event.id);
            }
        }
    }
}

impl InputCore for GilrsCore {
    fn controller(&mut self, index: usize) -> Option<ControllerState> {
        self.process_events();

        if index != 0 {
            return None;
        }

        let Some(gamepad_id) = self.active_gamepad else {
            return None;
        };

        let Some(gamepad) = self.gilrs.connected_gamepad(gamepad_id) else {
            self.active_gamepad = None;
            return None;
        };

        let axis = |axis| (255.0 * ((gamepad.value(axis) + 1.0) / 2.0)) as u8;
        let button = |button| {
            (255.0
                * gamepad
                    .button_data(button)
                    .map(|v| v.value())
                    .unwrap_or(0.0)) as u8
        };

        Some(ControllerState {
            analog_x: axis(Axis::LeftStickX),
            analog_y: axis(Axis::LeftStickY),
            analog_sub_x: axis(Axis::RightStickX),
            analog_sub_y: axis(Axis::RightStickY),
            analog_trigger_left: button(Button::LeftTrigger2),
            analog_trigger_right: button(Button::RightTrigger2),
            trigger_z: gamepad.is_pressed(Button::Z),
            trigger_right: gamepad.is_pressed(Button::RightTrigger),
            trigger_left: gamepad.is_pressed(Button::LeftTrigger),
            pad_left: gamepad.is_pressed(Button::DPadLeft),
            pad_right: gamepad.is_pressed(Button::DPadRight),
            pad_down: gamepad.is_pressed(Button::DPadDown),
            pad_up: gamepad.is_pressed(Button::DPadUp),
            button_a: gamepad.is_pressed(Button::South),
            button_b: gamepad.is_pressed(Button::East),
            button_x: gamepad.is_pressed(Button::West),
            button_y: gamepad.is_pressed(Button::North),
            button_start: gamepad.is_pressed(Button::Start),
        })
    }
}
