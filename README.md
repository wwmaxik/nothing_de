# Nothing OS Wayland Desktop Environment (Rust Prototype)

A custom Wayland Desktop Environment (DE) and compositor prototype inspired by the minimalist dot-matrix aesthetic of **Nothing OS**. Built in Rust utilizing the **Smithay** compositor library, this project features a dual-mode user interface, robust window management, and interactive widgets.

---

## 🎨 Design Philosophy

Inspired by Nothing's signature brand design, this DE implements:
- **NDot Style Typography**: A custom dot-matrix glyph rendering engine built into the compositor.
- **High-contrast Minimalist UI**: Sleek black, white, and gray color palette.
- **Glassmorphic & Rounded Elements**: Smooth borders, curved docks, and soft shadows designed to feel premium.
- **Dual Mode Interface**:
  - **Desktop Mode**: A clean workspace with a top-docked status bar showing active layout, status icons, and time.
  - **Dashboard Mode**: An interactive, full-screen widget panel inspired by the Nothing OS widgets.

---

## ✨ Features

- **Dual-Mode UI**:
  - **Desktop Mode**: Renders a persistent status bar panel at the top of the screen.
  - **Dashboard Mode** (Toggled via `Super + D`): Renders a beautiful full-screen overlay with dot-matrix digital clock, CPU usage circular gauge, RAM usage circular gauge, system uptime, battery status widget, and interactive buttons.
- **Interactive Widgets**:
  - **Wi-Fi Toggle Widget**: Interactive widget that toggles the Wi-Fi state on and off, changing color and updating icons.
  - **Theme Toggle Widget**: Interactive widget that toggles between Dark Mode and Light Mode, dynamically updating the compositor background color and text themes.
- **Window Management**:
  - **Tiling Layout (MasterStack)**: Active windows automatically tile, splitting the screen into a primary master area and a stack area for other windows.
  - **Floating Layout**: Free-form window positioning.
  - **Layout Toggle (`Super + T`)**: Switch dynamically between Tiling and Floating layout modes.
  - **Keyboard Window Cycling**: Navigate through open windows using `Super + J` (next) and `Super + K` (previous).
  - **Close Window**: Instantly close the active window with `Super + Q`.
- **Intuitive Mouse Grabs**:
  - **Titlebar Click-and-Drag**: Grab and move any window by simply clicking and dragging its top 32 pixels—no modifier keys required!
  - **Classic Super-Drag**: Drag windows from anywhere on their surface by holding the `Super` key and left-clicking.

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Super + D` | Toggle Dashboard Mode (Widget overlay) |
| `Super + T` | Toggle Layout Mode (Floating ↔️ Tiling MasterStack) |
| `Super + J` | Cycle focus to the **next** window |
| `Super + K` | Cycle focus to the **previous** window |
| `Super + Q` | Close the currently focused window |
| `Super + Enter` | Spawn a terminal (`weston-terminal`) |

---

## 📁 Project Structure

```
nothing_de/
├── src/
│   ├── main.rs         # Compositor entry point & event loop
│   ├── state.rs        # Main state tracking (screens, windows, UI state)
│   ├── input.rs        # Keyboard & pointer event dispatching
│   ├── grabs.rs        # Pointer grab implementation for window movement
│   ├── layout.rs       # MasterStack and Floating layout logic
│   └── ui.rs           # Canvas rendering, widgets, metrics, & font glyphs
├── Cargo.toml          # Cargo package dependencies
└── README.md           # Project documentation
```

---

## 🚀 Getting Started

### Prerequisites

You need the Rust toolchain installed, alongside development libraries for Wayland, EGL, GBM, and xkbcommon.

**On Ubuntu/Debian:**
```bash
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev libgbm-dev libegl-dev libxml2-dev libinput-dev libudev-dev
```

### Running

To run nested inside an existing X11 or Wayland session:
```bash
cargo run
```

*Note: Ensure you have `weston-terminal` installed if you want to spawn the default terminal using `Super + Enter`.*
