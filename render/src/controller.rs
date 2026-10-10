//! Host gamepad events become held controls and one-tick button taps. This
//! adapter never edits the simulation: the original controller still processes
//! raw stick bytes once per 30 Hz tick, and logs record those final tick inputs.
use gilrs::{Axis, Button, EventType, Gilrs, GilrsBuilder};
use rustario64::play::{Pad, analog_stick};
use std::collections::{BTreeMap, HashSet};

const CONTROLS: &str = "Left stick: move · A/Cross: jump · B/Circle: attack\nLT/LB/RT: crouch · RB: R camera · Right stick/D-pad: C buttons · Start: pause";

#[derive(Default)]
struct Input {
    axes: [f32; 6],
    buttons: HashSet<Button>,
    camera: [bool; 4],
    taps: Pad,
    armed: bool,
}

impl Input {
    fn held(&self) -> Pad {
        let has = |b| self.buttons.contains(&b);
        Pad {
            analog_stick: Some(analog_stick(self.axes[0], self.axes[1])),
            a: has(Button::South),
            b: has(Button::East),
            z: has(Button::LeftTrigger2) || has(Button::LeftTrigger) || has(Button::RightTrigger2),
            r: has(Button::RightTrigger),
            c_left: self.camera[0] || has(Button::DPadLeft),
            c_right: self.camera[1] || has(Button::DPadRight),
            c_down: self.camera[2] || has(Button::DPadDown),
            c_up: self.camera[3] || has(Button::DPadUp),
            ..Pad::default()
        }
    }

    fn clear(&mut self) {
        self.taps = Pad::default();
        // Keep physical state: a held stick/button must return to neutral
        // before menu, focus, restart or device-selection boundaries can rearm.
        self.armed = false;
    }

    fn arm_if_neutral(&mut self) {
        let held = self.held();
        if held.stick().iter().all(|v| v.abs() < 8)
            && held.buttons() == 0
            && self.buttons.is_empty()
        {
            self.armed = true;
        }
    }

    fn axis(&mut self, axis: Axis, value: f32, running: bool) {
        let index = match axis {
            Axis::LeftStickX => 0,
            Axis::LeftStickY => 1,
            Axis::RightStickX => 2,
            Axis::RightStickY => 3,
            Axis::DPadX => 4,
            Axis::DPadY => 5,
            _ => return,
        };
        let before = self.held();
        self.axes[index] = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        for (i, value) in [
            (-self.axes[2]).max(-self.axes[4]),
            self.axes[2].max(self.axes[4]),
            (-self.axes[3]).max(-self.axes[5]),
            self.axes[3].max(self.axes[5]),
        ]
        .into_iter()
        .enumerate()
        {
            // Hysteresis keeps noisy camera axes from creating repeated taps.
            self.camera[i] = value >= if self.camera[i] { 0.4 } else { 0.55 };
        }
        self.latch(before, running);
    }

    fn button(&mut self, button: Button, down: bool, running: bool) -> bool {
        let before = self.held();
        let changed = if down {
            self.buttons.insert(button)
        } else {
            self.buttons.remove(&button)
        };
        self.latch(before, running);
        button == Button::Start && down && changed
    }

    fn latch(&mut self, before: Pad, running: bool) {
        if !running || !self.armed {
            return;
        }
        let after = self.held();
        self.taps = self.taps.with_taps(&Pad {
            a: after.a && !before.a,
            b: after.b && !before.b,
            z: after.z && !before.z,
            r: after.r && !before.r,
            c_up: after.c_up && !before.c_up,
            c_down: after.c_down && !before.c_down,
            c_left: after.c_left && !before.c_left,
            c_right: after.c_right && !before.c_right,
            ..Pad::default()
        });
    }

    fn next_pad(&mut self) -> Pad {
        let pad = if self.armed {
            self.held().with_taps(&self.taps)
        } else {
            Pad::default()
        };
        self.taps = Pad::default();
        pad
    }
}

struct Device {
    name: String,
    input: Input,
}

#[derive(Default)]
pub struct Controllers {
    backend: Option<Gilrs>,
    devices: BTreeMap<usize, Device>,
    active: Option<usize>,
    auto_select: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Actions {
    pub pause: bool,
    pub disconnected: bool,
}

impl Controllers {
    pub fn new() -> Self {
        let mut controllers = Self {
            auto_select: true,
            ..Self::default()
        };
        // Disable gilrs' stick dead-zone/rescaling and jitter filters. We handle
        // both D-pad axis and button events and leave the left stick unfiltered.
        match GilrsBuilder::new()
            .with_default_filters(false)
            .set_axis_to_btn(0.5, 0.4)
            .build()
        {
            Ok(backend) => {
                for (id, device) in backend.gamepads() {
                    controllers.connect(id.into(), device.name().to_owned());
                }
                controllers.backend = Some(backend);
            }
            Err(error) => {
                controllers.error = Some(format!(
                    "Controller input unavailable: {error}. Keyboard controls still work."
                ))
            }
        }
        controllers
    }

    fn connect(&mut self, id: usize, name: String) {
        self.devices.insert(
            id,
            Device {
                name,
                input: Input::default(),
            },
        );
        if self.active.is_none() && self.auto_select {
            self.select(Some(id));
        }
    }

    fn disconnect(&mut self, id: usize) -> bool {
        self.devices.remove(&id);
        if self.active == Some(id) {
            self.active = None;
            self.clear();
            true
        } else {
            false
        }
    }

    fn select(&mut self, id: Option<usize>) {
        self.active = id.filter(|id| self.devices.contains_key(id));
        self.clear();
    }

    pub fn clear(&mut self) {
        for device in self.devices.values_mut() {
            device.input.clear();
        }
    }

    /// Drain even in menus and while unfocused, so stale events cannot become
    /// gameplay on resume. Start is a desktop command, never a gameplay tick.
    pub fn poll(&mut self, running: bool, menu_allowed: bool) -> Actions {
        if !running {
            self.clear();
        }
        let mut actions = Actions::default();
        while let Some(event) = self.backend.as_mut().and_then(Gilrs::next_event) {
            let id = usize::from(event.id);
            match event.event {
                EventType::Connected => {
                    let name = self
                        .backend
                        .as_ref()
                        .unwrap()
                        .gamepad(event.id)
                        .name()
                        .to_owned();
                    self.connect(id, name);
                }
                EventType::Disconnected => actions.disconnected |= self.disconnect(id),
                _ => {
                    let Some(device) = self.devices.get_mut(&id) else {
                        continue;
                    };
                    let active = self.active == Some(id);
                    match event.event {
                        EventType::AxisChanged(axis, value, _) => {
                            device.input.axis(axis, value, running && active)
                        }
                        EventType::ButtonPressed(button, _) => {
                            actions.pause |= device.input.button(button, true, running && active)
                                && active
                                && menu_allowed;
                        }
                        EventType::ButtonReleased(button, _) => {
                            device.input.button(button, false, running && active);
                        }
                        // gilrs emits thresholded press/release for analog triggers;
                        // ButtonChanged duplicates them and repeats are ignored.
                        _ => {}
                    }
                }
            }
        }
        for device in self.devices.values_mut() {
            device.input.arm_if_neutral();
        }
        if let Some(backend) = self.backend.as_mut() {
            backend.inc();
        }
        actions
    }

    pub fn next_pad(&mut self) -> Pad {
        self.active
            .and_then(|id| self.devices.get_mut(&id))
            .map_or(Pad::default(), |d| d.input.next_pad())
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        let before = self.active;
        let name = self
            .active
            .and_then(|id| self.devices.get(&id))
            .map_or("Keyboard only", |d| d.name.as_str());
        egui::ComboBox::from_label("Controller")
            .selected_text(name)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut self.active, None, "Keyboard only")
                    .clicked()
                {
                    self.auto_select = false;
                }
                for (&id, device) in &self.devices {
                    ui.selectable_value(
                        &mut self.active,
                        Some(id),
                        format!("{} ({id})", device.name),
                    );
                }
            });
        if before != self.active {
            self.auto_select = self.active.is_some();
            self.clear();
        }
        if self.devices.is_empty() {
            ui.label("Connect a controller to use gamepad controls.");
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.small(CONTROLS);
        if self
            .active
            .and_then(|id| self.devices.get(&id))
            .is_some_and(|d| !d.input.armed)
        {
            ui.small("Release buttons and center the sticks before playing.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analog_axes_and_short_taps_reach_one_tick_without_host_deadzone() {
        let mut input = Input::default();
        input.arm_if_neutral();
        input.axis(Axis::LeftStickX, 0.1, true);
        input.axis(Axis::LeftStickY, 0.5, true);
        input.button(Button::South, true, true);
        input.button(Button::South, false, true);
        let pad = input.next_pad();
        assert_eq!(pad.stick(), [8, 40]);
        assert!(pad.a);
        assert!(!input.next_pad().a);
        input.axis(Axis::LeftStickX, 1.0, true);
        input.axis(Axis::LeftStickY, 1.0, true);
        assert_eq!(input.next_pad().stick(), [57, 57]);
    }

    #[test]
    fn camera_axis_hysteresis_and_dpad_events_are_independent() {
        let mut input = Input::default();
        input.arm_if_neutral();
        input.axis(Axis::RightStickX, 0.7, true);
        input.axis(Axis::RightStickX, 0.5, true);
        assert!(input.next_pad().c_right);
        input.axis(Axis::RightStickX, 0.3, true);
        assert!(!input.next_pad().c_right);
        input.axis(Axis::DPadY, 1.0, true);
        input.button(Button::DPadUp, true, true);
        input.axis(Axis::DPadY, 0.0, true);
        assert!(input.next_pad().c_up);
        input.button(Button::DPadUp, false, true);
        assert!(!input.next_pad().c_up);
    }

    #[test]
    fn boundaries_require_neutral_and_drop_menu_taps() {
        let mut input = Input::default();
        input.arm_if_neutral();
        input.axis(Axis::LeftStickX, 1.0, true);
        input.button(Button::East, true, true);
        input.clear();
        input.arm_if_neutral();
        assert_eq!(input.next_pad(), Pad::default());
        input.button(Button::East, false, false);
        input.button(Button::South, true, false);
        input.button(Button::South, false, false);
        input.axis(Axis::LeftStickX, 0.0, false);
        input.arm_if_neutral();
        assert!(!input.next_pad().a);
        input.button(Button::South, true, true);
        assert!(input.next_pad().a);
    }

    #[test]
    fn aliased_triggers_start_edges_selection_and_disconnect() {
        let mut controllers = Controllers {
            auto_select: true,
            ..Controllers::default()
        };
        controllers.connect(2, "First".into());
        controllers.connect(3, "Second".into());
        controllers.poll(true, true);
        let input = &mut controllers.devices.get_mut(&2).unwrap().input;
        input.button(Button::LeftTrigger, true, true);
        input.button(Button::LeftTrigger2, true, true);
        input.button(Button::LeftTrigger, false, true);
        assert!(input.next_pad().z);
        assert!(input.button(Button::Start, true, false));
        assert!(!input.button(Button::Start, true, false));
        input.button(Button::Start, false, false);
        assert!(input.button(Button::Start, true, false));
        assert!(!controllers.disconnect(3));
        assert!(controllers.disconnect(2));
        assert_eq!(controllers.next_pad(), Pad::default());
        controllers.connect(2, "Reconnected".into());
        assert_eq!(controllers.active, Some(2));
        assert_eq!(controllers.next_pad(), Pad::default());
        controllers.select(None);
        controllers.auto_select = false;
        controllers.connect(4, "Third".into());
        assert_eq!(controllers.active, None);
    }

    #[test]
    fn mappings_reach_the_expected_reference_buttons() {
        let mut input = Input::default();
        input.arm_if_neutral();
        for button in [
            Button::South,
            Button::East,
            Button::LeftTrigger2,
            Button::RightTrigger,
            Button::DPadLeft,
            Button::DPadRight,
            Button::DPadUp,
            Button::DPadDown,
        ] {
            input.button(button, true, true);
        }
        let p = input.next_pad();
        assert!(p.a && p.b && p.z && p.r && p.c_left && p.c_right && p.c_up && p.c_down);
    }
}
