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
    utils::{Logical, Point, Rectangle, Transform},
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

    // Background color dynamically changes based on dark mode toggle
    let clear_color = if state.ui_state.dark_mode {
        [8.0 / 255.0, 8.0 / 255.0, 8.0 / 255.0, 1.0] // #080808
    } else {
        [245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0, 1.0] // #f5f5f5
    };

    // 1. Update stats
    state.ui_state.update();

    // 2. Get logical screen size
    let output_geo = state.space.output_geometry(output)
        .unwrap_or_else(|| Rectangle::new((0, 0).into(), (1280, 800).into()));
    let screen_w = output_geo.size.w;
    let screen_h = output_geo.size.h;

    let (canvas_w, canvas_h) = if state.ui_state.ui_mode == crate::ui::UiMode::Desktop {
        (screen_w as usize, 40)
    } else {
        (screen_w as usize, screen_h as usize)
    };

    let scale = output.current_scale().fractional_scale();

    // Bind renderer once inside a block to release mutable borrow of backend before submit
    {
        let (renderer, mut framebuffer) = backend.bind().unwrap();

        // 3. Create or resize texture buffer if needed
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

        // 4. Render canvas pixels
        let canvas = if state.ui_state.ui_mode == crate::ui::UiMode::Desktop {
            crate::ui::render_dock_canvas(&state.ui_state, state.layout_mode, screen_w as u32, screen_h as u32)
        } else {
            crate::ui::render_dashboard_canvas(&state.ui_state, screen_w as u32, screen_h as u32)
        };

        // 5. Update texture buffer
        if let Some((ref mut trb, _)) = state.ui_render_buffer {
            let region = smithay::utils::Rectangle::from_size(smithay::utils::Size::from((canvas_w as i32, canvas_h as i32)));
            trb.update_from_memory(
                renderer,
                &canvas.pixels,
                region,
                None,
            ).expect("Failed to update UI texture");
        }

        // 6. Build the custom elements array for space render
        let mut custom_elements = Vec::new();
        if let Some((ref trb, _)) = state.ui_render_buffer {
            let location_logical: Point<f64, Logical> = Point::from((0.0, 0.0));
            // Convert to physical location
            let location_physical = Point::from((location_logical.x * scale, location_logical.y * scale));

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

        // 7. Render everything
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

    // Send frame callbacks to clients to request the next frames
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
