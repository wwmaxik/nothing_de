use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, KeyState,
    },
    input::{
        keyboard::FilterResult,
        pointer::{
            AxisFrame, ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent,
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use crate::grabs::MoveSurfaceGrab;

use crate::state::NothingCompositorState;

impl NothingCompositorState {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);

                self.seat.get_keyboard().unwrap().input::<(), _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |state, _modifiers, handle| {
                        if event.state() == KeyState::Pressed && _modifiers.logo {
                            let keysym = handle.modified_sym();

                            // Super + Q: Close focused window
                            if keysym == smithay::input::keyboard::keysyms::KEY_q.into() {
                                if let Some(focused_surface) = state.seat.get_keyboard().unwrap().current_focus() {
                                    let mut target_window = None;
                                    for window in state.space.elements() {
                                        if let Some(surface) = window.toplevel() {
                                            if surface.wl_surface() == &focused_surface {
                                                target_window = Some(window.clone());
                                                break;
                                            }
                                        }
                                    }
                                    if let Some(window) = target_window {
                                        if let Some(toplevel) = window.toplevel() {
                                            toplevel.send_close();
                                        }
                                    }
                                }
                                return FilterResult::Intercept(());
                            }

                            // Super + Enter: Launch terminal
                            if keysym == smithay::input::keyboard::keysyms::KEY_Return.into() {
                                std::process::Command::new("weston-terminal").spawn().ok();
                                return FilterResult::Intercept(());
                            }

                            // Super + T: Toggle layout mode (Floating <-> MasterStack)
                            if keysym == smithay::input::keyboard::keysyms::KEY_t.into() {
                                state.layout_mode = match state.layout_mode {
                                    crate::layout::LayoutMode::Floating => {
                                        tracing::info!("Layout mode: MasterStack (Tiling)");
                                        crate::layout::LayoutMode::MasterStack
                                    }
                                    crate::layout::LayoutMode::MasterStack => {
                                        tracing::info!("Layout mode: Floating");
                                        crate::layout::LayoutMode::Floating
                                    }
                                };
                                crate::layout::apply_layout(state);
                                return FilterResult::Intercept(());
                            }

                            // Super + J: Focus next window
                            if keysym == smithay::input::keyboard::keysyms::KEY_j.into() {
                                focus_next_window(state, 1);
                                return FilterResult::Intercept(());
                            }

                            // Super + K: Focus previous window
                            if keysym == smithay::input::keyboard::keysyms::KEY_k.into() {
                                focus_next_window(state, -1);
                                return FilterResult::Intercept(());
                            }

                            // Super + D: Toggle Dashboard
                            if keysym == smithay::input::keyboard::keysyms::KEY_d.into() {
                                state.ui_state.ui_mode = match state.ui_state.ui_mode {
                                    crate::ui::UiMode::Desktop => crate::ui::UiMode::Dashboard,
                                    _ => crate::ui::UiMode::Desktop,
                                };
                                tracing::info!("UI Mode changed to {:?}", state.ui_state.ui_mode);
                                return FilterResult::Intercept(());
                            }

                            // Super + A: Toggle App Launcher
                            if keysym == smithay::input::keyboard::keysyms::KEY_a.into() {
                                state.ui_state.ui_mode = match state.ui_state.ui_mode {
                                    crate::ui::UiMode::AppLauncher => crate::ui::UiMode::Desktop,
                                    _ => crate::ui::UiMode::AppLauncher,
                                };
                                tracing::info!("UI Mode changed to {:?}", state.ui_state.ui_mode);
                                return FilterResult::Intercept(());
                            }

                            // Super + S: Toggle Quick Settings
                            if keysym == smithay::input::keyboard::keysyms::KEY_s.into() {
                                state.ui_state.ui_mode = match state.ui_state.ui_mode {
                                    crate::ui::UiMode::QuickSettings => crate::ui::UiMode::Desktop,
                                    _ => crate::ui::UiMode::QuickSettings,
                                };
                                tracing::info!("UI Mode changed to {:?}", state.ui_state.ui_mode);
                                return FilterResult::Intercept(());
                            }
                        }
                        FilterResult::Forward
                    },
                );
            }
            InputEvent::PointerMotion { .. } => {
                // Winit backend primarily uses PointerMotionAbsolute.
                // Raw TTY backends use PointerMotion (relative) which we will hook up in DRM/libinput integration.
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = match self.space.outputs().next() {
                    Some(o) => o,
                    None => return,
                };

                let output_geo = match self.space.output_geometry(output) {
                    Some(g) => g,
                    None => return,
                };

                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let serial = SERIAL_COUNTER.next_serial();
                let pointer = self.seat.get_pointer().unwrap();

                // Find surface under the pointer
                let under = if self.ui_state.ui_mode != crate::ui::UiMode::Desktop {
                    None
                } else {
                    self.space.element_under(pos).and_then(|(window, location)| {
                        window
                            .surface_under(pos - location.to_f64(), smithay::desktop::WindowSurfaceType::ALL)
                            .map(|(s, p)| (s, (p + location).to_f64()))
                    })
                };

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let button_state = event.state();

                const BTN_LEFT: u32 = 0x110;

                if self.ui_state.ui_mode != crate::ui::UiMode::Desktop {
                    if ButtonState::Pressed == button_state && button == BTN_LEFT {
                        let size = if let Some(output) = self.space.outputs().next() {
                            output.current_mode().map(|m| m.size).unwrap_or_else(|| (1280, 800).into())
                        } else {
                            (1280, 800).into()
                        };
                        self.ui_state.handle_click(pointer.current_location(), size.w as u32, size.h as u32);
                    }
                    return;
                }

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    // Check if Super is held for window dragging
                    let modifiers = keyboard.modifier_state();

                    if let Some((window, window_loc)) = self
                        .space
                        .element_under(pointer.current_location())
                        .map(|(w, l)| (w.clone(), l))
                    {
                        // Super + Left Click OR Left Click in the top 32px (titlebar area) to start drag
                        let relative_y = pointer.current_location().y - window_loc.y as f64;
                        if (modifiers.logo || relative_y < 32.0) && button == BTN_LEFT {
                            let start_data = PointerGrabStartData {
                                focus: None,
                                button: BTN_LEFT,
                                location: pointer.current_location(),
                            };
                            let grab = MoveSurfaceGrab {
                                start_data,
                                window: window.clone(),
                                initial_window_location: window_loc,
                            };
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                            return;
                        }

                        // Normal click: raise and focus the window
                        self.space.raise_element(&window, true);
                        keyboard.set_focus(
                            self,
                            Some(window.toplevel().unwrap().wl_surface().clone()),
                            serial,
                        );
                        self.space.elements().for_each(|window| {
                            window.toplevel().unwrap().send_pending_configure();
                        });
                    } else {
                        // Clicked background, clear keyboard focus
                        self.space.elements().for_each(|window| {
                            window.set_activated(false);
                            window.toplevel().unwrap().send_pending_configure();
                        });
                        keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                    }
                };

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
                let vertical_amount = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time_msec()).source(source);

                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }
}

/// Cycle keyboard focus through the tracked window list.
/// `direction`: +1 for next, -1 for previous.
fn focus_next_window(state: &mut NothingCompositorState, direction: i32) {
    let count = state.windows.len();
    if count == 0 {
        return;
    }

    let serial = SERIAL_COUNTER.next_serial();
    let keyboard = state.seat.get_keyboard().unwrap();
    let current_focus = keyboard.current_focus();

    // Find current focus index in our tracking list
    let current_idx = current_focus.as_ref().and_then(|focused| {
        state.windows.iter().position(|w| {
            w.toplevel()
                .map(|t| t.wl_surface() == focused)
                .unwrap_or(false)
        })
    });

    let next_idx = match current_idx {
        Some(idx) => ((idx as i32 + direction).rem_euclid(count as i32)) as usize,
        None => 0,
    };

    let target = state.windows[next_idx].clone();

    // Deactivate all, activate target
    for window in state.space.elements() {
        window.set_activated(false);
    }
    target.set_activated(true);

    // Raise and focus
    state.space.raise_element(&target, true);
    if let Some(toplevel) = target.toplevel() {
        keyboard.set_focus(state, Some(toplevel.wl_surface().clone()), serial);
    }

    // Send pending configures for activation state changes
    for window in state.space.elements() {
        window.toplevel().unwrap().send_pending_configure();
    }
}
