//! Move grab — the state during which a window is being dragged by the user.
//!
//! Activated via Super + Left Mouse Button on a window surface.

use crate::state::NothingCompositorState;
use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
        GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab,
        PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<NothingCompositorState>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<NothingCompositorState> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        data.space
            .map_element(self.window.clone(), new_location.to_i32_round(), true);
    }

    fn relative_motion(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        const BTN_LEFT: u32 = 0x110;

        if !handle.current_pressed().contains(&BTN_LEFT) {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
    ) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut NothingCompositorState,
        handle: &mut PointerInnerHandle<'_, NothingCompositorState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &PointerGrabStartData<NothingCompositorState> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut NothingCompositorState) {}
}
