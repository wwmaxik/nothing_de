use std::time::Duration;
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{
                texture::TextureRenderBuffer,
                texture::TextureRenderElement,
                Kind,
            },
            gles::GlesRenderer,
        },
    },
    output::Output,
    reexports::wayland_server::Resource,
    utils::{Point, Rectangle, Transform},
};

use crate::state::NothingCompositorState;

pub fn render_frame(
    state: &mut NothingCompositorState,
    backend: &mut smithay::backend::winit::WinitGraphicsBackend<GlesRenderer>,
    output: &Output,
    damage_tracker: &mut OutputDamageTracker,
    start_time: std::time::Instant,
) {
    let size = backend.window_size();
    let damage = Rectangle::from_size(size);

    // Background color
    let clear_color = if state.ui_state.dark_mode {
        [8.0 / 255.0, 8.0 / 255.0, 8.0 / 255.0, 1.0]
    } else {
        [245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0, 1.0]
    };

    // 1. Update stats
    state.ui_state.update();

    // 2. Get logical screen size
    let output_geo = state.space.output_geometry(output)
        .unwrap_or_else(|| Rectangle::new((0, 0).into(), (1280, 800).into()));
    let screen_w = output_geo.size.w as u32;
    let screen_h = output_geo.size.h as u32;

    // Determine canvas dimensions based on mode
    // In Desktop mode: full screen canvas (top bar + desktop widgets + bottom dock)
    // In overlay modes: full screen canvas
    let canvas_w = screen_w as usize;
    let canvas_h = screen_h as usize;

    let _scale = output.current_scale().fractional_scale();

    {
        let (renderer, mut framebuffer) = backend.bind().unwrap();

        // Create or resize texture buffer if needed
        let mut trb_needs_init = true;
        if let Some((_, (w, h))) = state.ui_render_buffer {
            if w == canvas_w as i32 && h == canvas_h as i32 {
                trb_needs_init = false;
            }
        }

        if trb_needs_init {
            let initial_data = vec![0u8; canvas_w * canvas_h * 4];
            let trb = TextureRenderBuffer::from_memory(
                renderer,
                &initial_data,
                Fourcc::Argb8888,
                (canvas_w as i32, canvas_h as i32),
                false,
                1,
                Transform::Normal,
                None,
            ).expect("Failed to create UI texture render buffer");
            state.ui_render_buffer = Some((trb, (canvas_w as i32, canvas_h as i32)));
        }

        // Render the appropriate canvas based on UI mode
        let canvas = match state.ui_state.ui_mode {
            crate::ui::UiMode::Desktop => {
                crate::ui::render_desktop_canvas(&state.ui_state, state.layout_mode, screen_w, screen_h)
            }
            crate::ui::UiMode::Dashboard => {
                crate::ui::render_dashboard_canvas(&state.ui_state, screen_w, screen_h)
            }
            crate::ui::UiMode::AppLauncher => {
                crate::ui::render_app_launcher_canvas(&state.ui_state, screen_w, screen_h)
            }
            crate::ui::UiMode::QuickSettings => {
                crate::ui::render_quick_settings_canvas(&state.ui_state, state.layout_mode, screen_w, screen_h)
            }
        };

        // Update texture buffer
        if let Some((ref mut trb, _)) = state.ui_render_buffer {
            let region = smithay::utils::Rectangle::from_size(smithay::utils::Size::from((canvas_w as i32, canvas_h as i32)));
            trb.update_from_memory(
                renderer,
                &canvas.pixels,
                region,
                None,
            ).expect("Failed to update UI texture");
        }

        // Build custom elements
        let mut custom_elements = Vec::new();
        if let Some((ref trb, _)) = state.ui_render_buffer {
            let location_physical = Point::from((0.0, 0.0));
            let element = TextureRenderElement::from_texture_render_buffer(
                location_physical,
                trb,
                None,
                None,
                None,
                Kind::Unspecified,
            );
            custom_elements.push(element);
        }

        // Render everything
        smithay::desktop::space::render_output(
            output,
            renderer,
            &mut framebuffer,
            1.0,
            0,
            [&state.space],
            &custom_elements,
            damage_tracker,
            clear_color,
        )
        .unwrap();
    }

    backend.submit(Some(&[damage])).unwrap();

    // Send frame callbacks
    state.space.elements().for_each(|window| {
        window.send_frame(
            output,
            start_time.elapsed(),
            Some(Duration::ZERO),
            |_, _| Some(output.clone()),
        )
    });

    state.space.refresh();

    // Detect removed windows and re-layout
    let before = state.windows.len();
    state.windows.retain(|w| {
        w.toplevel()
            .map(|t| t.wl_surface().is_alive())
            .unwrap_or(false)
    });
    if state.windows.len() != before {
        crate::layout::apply_layout(state);
    }

    let _ = state.display_handle.flush_clients();
}
