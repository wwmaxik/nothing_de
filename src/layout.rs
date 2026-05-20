use smithay::{
    desktop::Window,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::Resource,
    },
    utils::{Logical, Size},
};

use crate::state::NothingCompositorState;

/// Master window occupies 60% of screen width in tiling mode
const MASTER_RATIO: f64 = 0.6;

/// Stagger offset for new floating windows (pixels)
const FLOAT_STAGGER: i32 = 40;

/// Layout modes matching the Nothing OS prototype:
/// - Floating: free-form window placement (drag & resize)
/// - MasterStack: automatic tiling (master left 60%, stack right 40%)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Floating,
    MasterStack,
}

/// Recalculate and apply layout for all tracked windows.
pub fn apply_layout(state: &mut NothingCompositorState) {
    let output = match state.space.outputs().next() {
        Some(o) => o.clone(),
        None => return,
    };
    let output_geo = match state.space.output_geometry(&output) {
        Some(g) => g,
        None => return,
    };

    // Purge dead windows from our tracking list
    state.windows.retain(|w| {
        w.toplevel()
            .map(|t| t.wl_surface().is_alive())
            .unwrap_or(false)
    });

    let count = state.windows.len();
    if count == 0 {
        return;
    }

    let windows = state.windows.clone();

    match state.layout_mode {
        LayoutMode::Floating => {
            // In floating mode, stagger windows from top-left corner.
            // Only reposition windows that haven't been placed yet (initial spawn).
            // We use a simple cascade: each window offset by FLOAT_STAGGER.
            for (i, window) in windows.iter().enumerate() {
                let x = output_geo.loc.x + 80 + (i as i32 * FLOAT_STAGGER);
                let y = output_geo.loc.y + 40 + (i as i32 * FLOAT_STAGGER);

                // Don't force a size in floating mode — let clients choose their preferred size.
                // Just set the position and remove tiled states.
                if let Some(toplevel) = window.toplevel() {
                    toplevel.with_pending_state(|s| {
                        s.states.unset(xdg_toplevel::State::TiledLeft);
                        s.states.unset(xdg_toplevel::State::TiledRight);
                        s.states.unset(xdg_toplevel::State::TiledTop);
                        s.states.unset(xdg_toplevel::State::TiledBottom);
                        // Clear any compositor-forced size so client can pick its own
                        s.size = None;
                    });
                    toplevel.send_pending_configure();
                }
                state.space.map_element(window.clone(), (x, y), false);
            }
        }

        LayoutMode::MasterStack => {
            if count == 1 {
                // Single window fills the entire output
                let window = &windows[0];
                configure_tiled(window, (output_geo.size.w, output_geo.size.h).into());
                state.space.map_element(window.clone(), output_geo.loc, false);
            } else {
                let master_width = (output_geo.size.w as f64 * MASTER_RATIO) as i32;
                let stack_width = output_geo.size.w - master_width;
                let stack_count = (count - 1) as i32;
                let stack_height = output_geo.size.h / stack_count;

                // Master window (first in tracking list)
                let master = &windows[0];
                configure_tiled(master, (master_width, output_geo.size.h).into());
                state.space.map_element(master.clone(), output_geo.loc, false);

                // Stack windows (remaining)
                for (i, window) in windows[1..].iter().enumerate() {
                    let x = output_geo.loc.x + master_width;
                    let y = output_geo.loc.y + (i as i32 * stack_height);
                    // Last stack window absorbs rounding remainder
                    let h = if i as i32 == stack_count - 1 {
                        output_geo.size.h - (i as i32 * stack_height)
                    } else {
                        stack_height
                    };
                    configure_tiled(window, (stack_width, h).into());
                    state.space.map_element(window.clone(), (x, y), false);
                }
            }
        }
    }
}

/// Send a configure with the given size and all four tiled-edge states set.
fn configure_tiled(window: &Window, size: Size<i32, Logical>) {
    if let Some(toplevel) = window.toplevel() {
        toplevel.with_pending_state(|s| {
            s.size = Some(size);
            s.states.set(xdg_toplevel::State::TiledLeft);
            s.states.set(xdg_toplevel::State::TiledRight);
            s.states.set(xdg_toplevel::State::TiledTop);
            s.states.set(xdg_toplevel::State::TiledBottom);
        });
        toplevel.send_pending_configure();
    }
}
