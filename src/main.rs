#![allow(irrefutable_let_patterns)]

mod state;
mod shell;
mod input;
mod render;
mod winit;
mod layout;
mod grabs;
mod ui;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
pub use state::NothingCompositorState;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let mut event_loop: EventLoop<NothingCompositorState> = EventLoop::try_new()?;
    let display: Display<NothingCompositorState> = Display::new()?;
    let mut state = NothingCompositorState::new(&mut event_loop, display);

    // Open a nested Wayland/X11 window
    crate::winit::init_winit(&mut event_loop, &mut state)?;

    // Expose nested WAYLAND_DISPLAY socket path for child clients
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);
    }
    
    tracing::info!("NothingDE is listening on socket: {:?}", state.socket_name);

    // Spawn nested test client if requested
    spawn_client();

    event_loop.run(None, &mut state, move |_| {
        // Run compositor event loop iterations
    })?;

    Ok(())
}

fn init_logging() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }
}

fn spawn_client() {
    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            std::process::Command::new(command).spawn().ok();
        }
        _ => {
            // Default client for nested environment verification
            std::process::Command::new("weston-terminal").spawn().ok();
        }
    }
}
