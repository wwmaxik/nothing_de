use std::time::Instant;
use smithay::utils::{Logical, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Desktop,
    Dashboard,
    AppLauncher,
    QuickSettings,
}

pub struct NothingUiState {
    pub ui_mode: UiMode,
    pub cpu_usage: u8,
    pub mem_usage: u8,
    pub battery_percentage: u8,
    pub battery_charging: bool,
    pub uptime_secs: u64,
    pub wifi_enabled: bool,
    pub dark_mode: bool,
    pub last_stats_update: Instant,
    pub prev_cpu_times: Option<(u64, u64)>,
    // Music player state
    pub music_playing: bool,
    pub music_track_index: usize,
    // Quick settings
    pub bluetooth_enabled: bool,
    pub volume: u8,
    pub brightness: u8,
    // Active workspace
    pub active_workspace: u8,
    // Frame counter for animations
    pub frame_count: u64,
}

impl NothingUiState {
    pub fn new() -> Self {
        Self {
            ui_mode: UiMode::Desktop,
            cpu_usage: 0,
            mem_usage: 0,
            battery_percentage: 100,
            battery_charging: true,
            uptime_secs: 0,
            wifi_enabled: true,
            dark_mode: true,
            last_stats_update: Instant::now(),
            prev_cpu_times: None,
            music_playing: false,
            music_track_index: 0,
            bluetooth_enabled: false,
            volume: 80,
            brightness: 70,
            active_workspace: 1,
            frame_count: 0,
        }
    }

    pub fn update(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
        if self.last_stats_update.elapsed() < std::time::Duration::from_millis(500) && self.prev_cpu_times.is_some() {
            return;
        }
        self.last_stats_update = Instant::now();

        // 1. Uptime
        self.uptime_secs = std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|s| s.split_whitespace().next().and_then(|val| val.parse::<f64>().ok()))
            .map(|val| val as u64)
            .unwrap_or(0);

        // 2. Memory
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0;
            let mut available = 0;
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    total = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                } else if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                }
            }
            if total > 0 {
                self.mem_usage = (((total - available) * 100) / total) as u8;
            }
        }

        // 3. Battery
        let mut found_battery = false;
        if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("BAT") {
                    let cap_path = entry.path().join("capacity");
                    let status_path = entry.path().join("status");
                    let capacity = std::fs::read_to_string(cap_path)
                        .ok()
                        .and_then(|s| s.trim().parse::<u8>().ok())
                        .unwrap_or(0);
                    let status = std::fs::read_to_string(status_path)
                        .ok()
                        .map(|s| s.trim().to_lowercase())
                        .unwrap_or_default();
                    let charging = status == "charging" || status == "full";
                    self.battery_percentage = capacity;
                    self.battery_charging = charging;
                    found_battery = true;
                    break;
                }
            }
        }
        if !found_battery {
            self.battery_percentage = 100;
            self.battery_charging = true;
        }

        // 4. CPU
        if let Some((total, idle)) = get_cpu_times() {
            if let Some((prev_total, prev_idle)) = self.prev_cpu_times {
                let diff_total = total.saturating_sub(prev_total);
                let diff_idle = idle.saturating_sub(prev_idle);
                if diff_total > 0 {
                    self.cpu_usage = (((diff_total - diff_idle) * 100) / diff_total) as u8;
                }
            }
            self.prev_cpu_times = Some((total, idle));
        }
    }

    /// Handles pointer clicks in Dashboard mode. Returns true if a widget was clicked.
    pub fn handle_click(&mut self, pos: Point<f64, Logical>, width: u32, height: u32) -> bool {
        if self.ui_mode != UiMode::Dashboard {
            return false;
        }

        let dx_offset = (width as i32 - 780) / 2;
        let dy_offset = (height as i32 - 380) / 2;

        let col3_x = dx_offset + 600;
        let row0_y = dy_offset + 0;
        let row1_y = dy_offset + 200;

        let click_x = pos.x as i32;
        let click_y = pos.y as i32;

        // Check WIFI Toggle (Column 3, Row 0)
        if click_x >= col3_x && click_x < col3_x + 180 && click_y >= row0_y && click_y < row0_y + 180 {
            self.wifi_enabled = !self.wifi_enabled;
            tracing::info!("Dashboard: Wi-Fi toggled to {}", self.wifi_enabled);
            return true;
        }

        // Check DARK Toggle (Column 3, Row 1)
        if click_x >= col3_x && click_x < col3_x + 180 && click_y >= row1_y && click_y < row1_y + 180 {
            self.dark_mode = !self.dark_mode;
            tracing::info!("Dashboard: Dark Mode toggled to {}", self.dark_mode);
            return true;
        }

        false
    }
}

fn get_cpu_times() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/stat").ok()?;
    let first_line = content.lines().next()?;
    let mut parts = first_line.split_whitespace();
    if parts.next()? != "cpu" {
        return None;
    }
    let numbers: Vec<u64> = parts.filter_map(|p| p.parse::<u64>().ok()).collect();
    if numbers.len() >= 4 {
        let user = numbers[0];
        let nice = numbers[1];
        let system = numbers[2];
        let idle = numbers[3];
        let iowait = numbers.get(4).copied().unwrap_or(0);
        let irq = numbers.get(5).copied().unwrap_or(0);
        let softirq = numbers.get(6).copied().unwrap_or(0);
        let steal = numbers.get(7).copied().unwrap_or(0);

        let total_idle = idle + iowait;
        let total_non_idle = user + nice + system + irq + softirq + steal;
        let total = total_idle + total_non_idle;
        return Some((total, total_idle));
    }
    None
}

// 5x7 Font Mapping for Nothing OS NDot aesthetics
const GLYPHS: &[(char, [u8; 7])] = &[
    ('0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
    ('1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('2', [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111]),
    ('3', [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
    ('4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    ('5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    ('6', [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    ('8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    ('9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110]),
    (':', [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000]),
    ('%', [0b11001, 0b11010, 0b00100, 0b01000, 0b01011, 0b10011, 0b00000]),
    ('/', [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000, 0b00000]),
    ('-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
    ('.', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100]),
    ('A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('B', [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
    ('C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    ('D', [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100]),
    ('E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
    ('F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
    ('H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
    ('K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    ('L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    ('M', [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001]),
    ('N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
    ('O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    ('R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    ('S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    ('T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('V', [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100]),
    ('W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
    ('X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    ('Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
];

pub struct NothingCanvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl NothingCanvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) * 4;
            // Write BGRA8888 (Little-endian ARGB8888)
            self.pixels[idx] = b;
            self.pixels[idx + 1] = g;
            self.pixels[idx + 2] = r;
            self.pixels[idx + 3] = a;
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.set_pixel(x, y, r, g, b, a);
            }
        }
    }

    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, r, g, b, a);
            }
        }
    }

    pub fn draw_rounded_rect(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..h {
            for dx in 0..w {
                let mut draw = true;
                if dx < radius && dy < radius {
                    let dist_sq = (radius - dx) * (radius - dx) + (radius - dy) * (radius - dy);
                    if dist_sq > radius * radius { draw = false; }
                } else if dx >= w - radius && dy < radius {
                    let r_dx = dx - (w - radius);
                    let dist_sq = r_dx * r_dx + (radius - dy) * (radius - dy);
                    if dist_sq > radius * radius { draw = false; }
                } else if dx < radius && dy >= h - radius {
                    let r_dy = dy - (h - radius);
                    let dist_sq = (radius - dx) * (radius - dx) + r_dy * r_dy;
                    if dist_sq > radius * radius { draw = false; }
                } else if dx >= w - radius && dy >= h - radius {
                    let r_dx = dx - (w - radius);
                    let r_dy = dy - (h - radius);
                    let dist_sq = r_dx * r_dx + r_dy * r_dy;
                    if dist_sq > radius * radius { draw = false; }
                }
                if draw {
                    self.set_pixel(x + dx, y + dy, r, g, b, a);
                }
            }
        }
    }

    pub fn draw_rounded_rect_border(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, thickness: usize, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..h {
            for dx in 0..w {
                let mut is_border = false;
                
                // Outer check
                let mut inside_outer = true;
                if dx < radius && dy < radius {
                    let dist_sq = (radius - dx) * (radius - dx) + (radius - dy) * (radius - dy);
                    if dist_sq > radius * radius { inside_outer = false; }
                } else if dx >= w - radius && dy < radius {
                    let r_dx = dx - (w - radius);
                    let dist_sq = r_dx * r_dx + (radius - dy) * (radius - dy);
                    if dist_sq > radius * radius { inside_outer = false; }
                } else if dx < radius && dy >= h - radius {
                    let r_dy = dy - (h - radius);
                    let dist_sq = (radius - dx) * (radius - dx) + r_dy * r_dy;
                    if dist_sq > radius * radius { inside_outer = false; }
                } else if dx >= w - radius && dy >= h - radius {
                    let r_dx = dx - (w - radius);
                    let r_dy = dy - (h - radius);
                    let dist_sq = r_dx * r_dx + r_dy * r_dy;
                    if dist_sq > radius * radius { inside_outer = false; }
                }

                if inside_outer {
                    // Check if it is NOT in the inner scaled area
                    let mut inside_inner = false;
                    let border = thickness;
                    if dx >= border && dx < w - border && dy >= border && dy < h - border {
                        inside_inner = true;
                        let inner_r = if radius > border { radius - border } else { 0 };
                        
                        let idx = dx - border;
                        let idy = dy - border;
                        let iw = w - border * 2;
                        let ih = h - border * 2;

                        if idx < inner_r && idy < inner_r {
                            let dist_sq = (inner_r - idx) * (inner_r - idx) + (inner_r - idy) * (inner_r - idy);
                            if dist_sq > inner_r * inner_r { inside_inner = false; }
                        } else if idx >= iw - inner_r && idy < inner_r {
                            let r_dx = idx - (iw - inner_r);
                            let dist_sq = r_dx * r_dx + (inner_r - idy) * (inner_r - idy);
                            if dist_sq > inner_r * inner_r { inside_inner = false; }
                        } else if idx < inner_r && idy >= ih - inner_r {
                            let r_dy = idy - (ih - inner_r);
                            let dist_sq = (inner_r - idx) * (inner_r - idx) + r_dy * r_dy;
                            if dist_sq > inner_r * inner_r { inside_inner = false; }
                        } else if idx >= iw - inner_r && idy >= ih - inner_r {
                            let r_dx = idx - (iw - inner_r);
                            let r_dy = idy - (ih - inner_r);
                            let dist_sq = r_dx * r_dx + r_dy * r_dy;
                            if dist_sq > inner_r * inner_r { inside_inner = false; }
                        }
                    }

                    if !inside_inner {
                        is_border = true;
                    }
                }

                if is_border {
                    self.set_pixel(x + dx, y + dy, r, g, b, a);
                }
            }
        }
    }

    pub fn draw_circle(&mut self, cx: usize, cy: usize, radius: usize, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..=radius * 2 {
            for dx in 0..=radius * 2 {
                let x_dist = (dx as isize - radius as isize).abs();
                let y_dist = (dy as isize - radius as isize).abs();
                if x_dist * x_dist + y_dist * y_dist <= (radius * radius) as isize {
                    let px = (cx as isize - radius as isize + dx as isize) as usize;
                    let py = (cy as isize - radius as isize + dy as isize) as usize;
                    self.set_pixel(px, py, r, g, b, a);
                }
            }
        }
    }

    pub fn draw_circle_border(&mut self, cx: usize, cy: usize, radius: usize, thickness: usize, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..=radius * 2 {
            for dx in 0..=radius * 2 {
                let x_dist = (dx as isize - radius as isize).abs();
                let y_dist = (dy as isize - radius as isize).abs();
                let dist_sq = x_dist * x_dist + y_dist * y_dist;
                let inner_r = radius as isize - thickness as isize;
                if dist_sq <= (radius * radius) as isize && dist_sq >= (inner_r * inner_r) as isize {
                    let px = (cx as isize - radius as isize + dx as isize) as usize;
                    let py = (cy as isize - radius as isize + dy as isize) as usize;
                    self.set_pixel(px, py, r, g, b, a);
                }
            }
        }
    }

    pub fn draw_char_dots(&mut self, ch: char, x: usize, y: usize, dot_size: usize, spacing: usize, r: u8, g: u8, b: u8, a: u8) {
        let glyph = GLYPHS.iter().find(|(c, _)| *c == ch.to_ascii_uppercase()).map(|(_, data)| data);
        if let Some(data) = glyph {
            for row in 0..7 {
                let row_val = data[row];
                for col in 0..5 {
                    if (row_val >> (4 - col)) & 1 == 1 {
                        let cx = x + col * (dot_size + spacing);
                        let cy = y + row * (dot_size + spacing);
                        for dy in 0..dot_size {
                            for dx in 0..dot_size {
                                self.set_pixel(cx + dx, cy + dy, r, g, b, a);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn draw_text_dots(&mut self, text: &str, x: usize, y: usize, dot_size: usize, spacing: usize, char_spacing: usize, r: u8, g: u8, b: u8, a: u8) {
        let mut cur_x = x;
        for ch in text.chars() {
            if ch == ' ' {
                cur_x += 4 * (dot_size + spacing);
            } else {
                self.draw_char_dots(ch, cur_x, y, dot_size, spacing, r, g, b, a);
                cur_x += 5 * (dot_size + spacing) + char_spacing;
            }
        }
    }
}

/// Helper to draw the top status bar onto a canvas at y_offset
fn draw_top_bar(canvas: &mut NothingCanvas, state: &NothingUiState, layout_mode: crate::layout::LayoutMode, width: usize, y: usize) {
    let bar_h = 48;
    // Glass panel background
    canvas.draw_rounded_rect(16, y + 4, width - 32, bar_h - 8, 20, 18, 18, 18, 190);
    canvas.draw_rounded_rect_border(16, y + 4, width - 32, bar_h - 8, 20, 1, 255, 255, 255, 20);

    let cy = y + bar_h / 2;
    // Left: Red pulse dot
    let pulse = if state.frame_count % 60 < 30 { 255u8 } else { 180u8 };
    canvas.draw_circle(40, cy, 4, 255, 0, 60, pulse);
    // Left: RUST-WM // SMITHAY
    canvas.draw_text_dots("RUST-WM", 54, cy - 3, 1, 1, 2, 160, 160, 160, 255);

    // Workspace dots
    let ws_x = 160;
    canvas.draw_rounded_rect(ws_x, cy - 8, 70, 16, 8, 0, 0, 0, 80);
    for i in 0..4u8 {
        let dx = ws_x + 10 + i as usize * 16;
        if state.active_workspace == i + 1 {
            canvas.draw_circle(dx, cy, 4, 255, 255, 255, 255);
        } else {
            canvas.draw_circle(dx, cy, 4, 255, 255, 255, 50);
        }
    }

    // Center: Clock + Date
    let local_time = chrono::Local::now();
    let clock_str = local_time.format("%H:%M").to_string();
    let center_x = width / 2;
    let clock_tw = clock_str.len() * 10;
    canvas.draw_text_dots(&clock_str, center_x - clock_tw / 2 - 20, cy - 4, 1, 1, 3, 255, 255, 255, 255);
    let day = local_time.format("%d").to_string();
    let month_idx = local_time.format("%m").to_string().parse::<usize>().unwrap_or(1);
    let months = ["", "JAN","FEB","MAR","APR","MAY","JUN","JUL","AUG","SEP","OCT","NOV","DEC"];
    let m = if month_idx < 13 { months[month_idx] } else { "???" };
    let date_str = format!("{} {}", day, m);
    canvas.draw_text_dots(&date_str, center_x + 30, cy - 3, 1, 1, 2, 160, 160, 160, 255);

    // Right side: Layout toggle
    let layout_str = match layout_mode {
        crate::layout::LayoutMode::Floating => "FLOAT",
        crate::layout::LayoutMode::MasterStack => "TILE",
    };
    let rx = width - 280;
    canvas.draw_rounded_rect(rx, cy - 8, 60, 16, 8, 255, 255, 255, 13);
    canvas.draw_rounded_rect_border(rx, cy - 8, 60, 16, 8, 1, 255, 255, 255, 25);
    canvas.draw_text_dots(layout_str, rx + 8, cy - 3, 1, 1, 2, 200, 200, 200, 255);

    // WiFi icon (simple dot representation)
    let wifi_x = width - 200;
    let wc = if state.wifi_enabled { 255u8 } else { 80u8 };
    canvas.draw_circle(wifi_x, cy, 3, wc, wc, wc, 255);
    canvas.draw_circle_border(wifi_x, cy, 6, 1, wc, wc, wc, 200);

    // Battery circle arc
    let bat_x = width - 170;
    canvas.draw_circle_border(bat_x, cy, 8, 2, 255, 255, 255, 25);
    // Draw filled arc based on percentage
    let pct = state.battery_percentage as f64 / 100.0;
    let steps = (pct * 50.0) as usize;
    for s in 0..steps {
        let angle = -std::f64::consts::FRAC_PI_2 + (s as f64 / 50.0) * std::f64::consts::TAU;
        let px = bat_x as f64 + angle.cos() * 8.0;
        let py = cy as f64 + angle.sin() * 8.0;
        canvas.set_pixel(px as usize, py as usize, 255, 255, 255, 255);
        canvas.set_pixel(px as usize + 1, py as usize, 255, 255, 255, 255);
    }
    let bat_str = format!("{}", state.battery_percentage);
    let btw = bat_str.len() * 5;
    canvas.draw_text_dots(&bat_str, bat_x - btw / 2, cy - 3, 1, 0, 1, 255, 255, 255, 255);

    // Settings button (circle)
    let set_x = width - 130;
    canvas.draw_circle(set_x, cy, 12, 255, 255, 255, 25);
    canvas.draw_text_dots("S", set_x - 3, cy - 4, 1, 1, 0, 255, 255, 255, 200);
}

/// Helper to draw the bottom dock
fn draw_bottom_dock(canvas: &mut NothingCanvas, width: usize, height: usize) {
    let dock_w = 340;
    let dock_h = 50;
    let dock_x = (width - dock_w) / 2;
    let dock_y = height - dock_h - 16;

    // Glass panel
    canvas.draw_rounded_rect(dock_x, dock_y, dock_w, dock_h, 25, 18, 18, 18, 190);
    canvas.draw_rounded_rect_border(dock_x, dock_y, dock_w, dock_h, 25, 1, 255, 255, 255, 20);

    let cy = dock_y + dock_h / 2;

    // Grid launcher button (white circle with dots)
    let grid_x = dock_x + 30;
    canvas.draw_circle(grid_x, cy, 16, 255, 255, 255, 255);
    // 3x3 dot grid inside
    for r in 0..3 {
        for c in 0..3 {
            canvas.draw_circle(grid_x - 5 + c * 5, cy - 5 + r * 5, 1, 0, 0, 0, 255);
        }
    }

    // Separator
    for dy in 0..20 {
        canvas.set_pixel(dock_x + 64, cy - 10 + dy, 255, 255, 255, 25);
    }

    // App buttons: TERM, LOGS, FILE, SET
    let labels = ["T", "A", "F", "S"];
    for (i, label) in labels.iter().enumerate() {
        let bx = dock_x + 90 + i * 60;
        canvas.draw_circle(bx, cy, 16, 255, 255, 255, 13);
        canvas.draw_circle_border(bx, cy, 16, 1, 255, 255, 255, 25);
        canvas.draw_text_dots(label, bx - 3, cy - 4, 1, 1, 0, 255, 255, 255, 200);
    }
}

/// Helper to draw desktop widgets on left side
fn draw_desktop_widgets(canvas: &mut NothingCanvas, state: &NothingUiState) {
    let wx = 32;
    let wy = 70;
    let wsize = 170;
    let gap = 16;

    // Widget 1: Weather
    canvas.draw_rounded_rect(wx, wy, wsize, wsize, 28, 18, 18, 18, 190);
    canvas.draw_rounded_rect_border(wx, wy, wsize, wsize, 28, 1, 255, 255, 255, 20);
    canvas.draw_text_dots("MSK", wx + 16, wy + 16, 1, 1, 2, 160, 160, 160, 255);
    canvas.draw_text_dots("24", wx + 30, wy + 55, 4, 2, 8, 255, 255, 255, 255);
    // degree symbol as dot
    canvas.draw_circle(wx + 120, wy + 55, 3, 255, 255, 255, 255);
    canvas.draw_circle(wx + 120, wy + 55, 1, 18, 18, 18, 255);
    canvas.draw_text_dots("KRASNOYARSK", wx + 12, wy + wsize - 24, 1, 0, 1, 100, 100, 100, 255);

    // Widget 2: Music Player
    let m_y = wy + wsize + gap;
    canvas.draw_rounded_rect(wx, m_y, wsize, wsize, 28, 18, 18, 18, 190);
    canvas.draw_rounded_rect_border(wx, m_y, wsize, wsize, 28, 1, 255, 255, 255, 20);
    canvas.draw_circle(wx + 16, m_y + 16, 3, 255, 0, 60, 255); // red music dot
    canvas.draw_text_dots("EAR 2", wx + 60, m_y + 14, 1, 0, 1, 160, 160, 160, 255);

    let tracks = [("OVERTHINKER", "INZO"), ("NOTHING ELSE", "JOE HERTZ"), ("GHOST", "KAWAI")];
    let tidx = state.music_track_index % tracks.len();
    let (title, artist) = tracks[tidx];
    let title_tw = title.len() * 8;
    canvas.draw_text_dots(title, wx + (wsize - title_tw) / 2, m_y + 60, 1, 1, 2, 255, 255, 255, 255);
    let artist_tw = artist.len() * 7;
    canvas.draw_text_dots(artist, wx + (wsize - artist_tw) / 2, m_y + 80, 1, 0, 1, 160, 160, 160, 255);

    // Playback controls
    let ctrl_y = m_y + 120;
    let ctrl_cx = wx + wsize / 2;
    // Prev
    canvas.draw_circle(ctrl_cx - 40, ctrl_y, 12, 255, 255, 255, 13);
    canvas.draw_text_dots("-", ctrl_cx - 43, ctrl_y - 3, 1, 1, 0, 200, 200, 200, 255);
    // Play/Pause
    let play_char = if state.music_playing { "P" } else { "." };
    canvas.draw_circle(ctrl_cx, ctrl_y, 16, 255, 255, 255, 255);
    canvas.draw_text_dots(play_char, ctrl_cx - 3, ctrl_y - 4, 1, 1, 0, 0, 0, 0, 255);
    // Next
    canvas.draw_circle(ctrl_cx + 40, ctrl_y, 12, 255, 255, 255, 13);
    canvas.draw_text_dots("-", ctrl_cx + 37, ctrl_y - 3, 1, 1, 0, 200, 200, 200, 255);

    // Widget 3: System Monitor (spans 2 columns width)
    let s_y = m_y + wsize + gap;
    let s_w = wsize * 2 + gap;
    let s_h = 100;
    canvas.draw_rounded_rect(wx, s_y, s_w, s_h, 28, 18, 18, 18, 190);
    canvas.draw_rounded_rect_border(wx, s_y, s_w, s_h, 28, 1, 255, 255, 255, 20);
    canvas.draw_text_dots("RUST SMITHAY STATS", wx + 16, s_y + 14, 1, 0, 1, 160, 160, 160, 255);
    canvas.draw_text_dots("LIVE", wx + s_w - 50, s_y + 14, 1, 0, 1, 255, 0, 60, 255);

    // CPU bar
    let bar_x = wx + 16;
    let bar_w = s_w - 32;
    let cpu_str = format!("CPU {}%", state.cpu_usage);
    canvas.draw_text_dots(&cpu_str, bar_x, s_y + 36, 1, 0, 1, 160, 160, 160, 255);
    canvas.draw_rounded_rect(bar_x, s_y + 50, bar_w, 6, 3, 30, 30, 30, 255);
    let cpu_fill = (bar_w as u32 * state.cpu_usage as u32 / 100) as usize;
    if cpu_fill > 0 {
        canvas.draw_rounded_rect(bar_x, s_y + 50, cpu_fill.max(6), 6, 3, 255, 255, 255, 255);
    }

    // RAM bar
    let ram_str = format!("RAM {}%", state.mem_usage);
    canvas.draw_text_dots(&ram_str, bar_x, s_y + 64, 1, 0, 1, 160, 160, 160, 255);
    canvas.draw_rounded_rect(bar_x, s_y + 78, bar_w, 6, 3, 30, 30, 30, 255);
    let ram_fill = (bar_w as u32 * state.mem_usage as u32 / 100) as usize;
    if ram_fill > 0 {
        canvas.draw_rounded_rect(bar_x, s_y + 78, ram_fill.max(6), 6, 3, 255, 255, 255, 255);
    }
}

/// Desktop mode: transparent full-screen canvas with top bar, desktop widgets, bottom dock
pub fn render_desktop_canvas(state: &NothingUiState, layout_mode: crate::layout::LayoutMode, width: u32, height: u32) -> NothingCanvas {
    let w = width as usize;
    let h = height as usize;
    let mut canvas = NothingCanvas::new(w, h);
    // Transparent background (windows show through)
    canvas.clear(0, 0, 0, 0);

    draw_top_bar(&mut canvas, state, layout_mode, w, 0);
    draw_desktop_widgets(&mut canvas, state);
    draw_bottom_dock(&mut canvas, w, h);

    canvas
}

pub fn render_dashboard_canvas(state: &NothingUiState, width: u32, height: u32) -> NothingCanvas {
    let mut canvas = NothingCanvas::new(width as usize, height as usize);
    
    // Choose colors based on dark mode toggle
    let (bg_overlay_color, widget_bg_color, widget_border_color, primary_color, secondary_color, hole_color) = if state.dark_mode {
        (
            (10, 10, 10, 217),      // bg_overlay
            (0, 0, 0, 240),         // widget_bg
            (80, 80, 80, 255),      // widget_border
            (255, 255, 255, 255),   // primary_text
            (140, 140, 140, 255),   // secondary_text
            (0, 0, 0)               // hole color (matching widget_bg)
        )
    } else {
        (
            (240, 240, 240, 217),   // bg_overlay
            (255, 255, 255, 240),   // widget_bg
            (180, 180, 180, 255),   // widget_border
            (0, 0, 0, 255),         // primary_text
            (100, 100, 100, 255),   // secondary_text
            (255, 255, 255)         // hole color (matching widget_bg)
        )
    };

    canvas.clear(bg_overlay_color.0, bg_overlay_color.1, bg_overlay_color.2, bg_overlay_color.3);
    
    // Center the grid of widgets (780x380)
    let dx_offset = (width as i32 - 780) / 2;
    let dy_offset = (height as i32 - 380) / 2;
    
    let col0_x = (dx_offset + 0) as usize;
    let col2_x = (dx_offset + 400) as usize;
    let col3_x = (dx_offset + 600) as usize;
    
    let row0_y = (dy_offset + 0) as usize;
    let row1_y = (dy_offset + 200) as usize;

    // 1. Clock Widget (2x2 grid size: 380x380)
    canvas.draw_rounded_rect(col0_x, row0_y, 380, 380, 24, widget_bg_color.0, widget_bg_color.1, widget_bg_color.2, widget_bg_color.3);
    canvas.draw_rounded_rect_border(col0_x, row0_y, 380, 380, 24, 1, widget_border_color.0, widget_border_color.1, widget_border_color.2, widget_border_color.3);
    
    let local_time = chrono::Local::now();
    let hh_str = local_time.format("%H").to_string();
    let mm_str = local_time.format("%M").to_string();
    
    // Big stacked clock
    let clock_dx = col0_x + 106;
    let hh_dy = row0_y + 80;
    let mm_dy = row0_y + 190;
    
    canvas.draw_text_dots(&hh_str, clock_dx, hh_dy, 6, 2, 16, primary_color.0, primary_color.1, primary_color.2, primary_color.3);
    canvas.draw_text_dots(&mm_str, clock_dx, mm_dy, 6, 2, 16, primary_color.0, primary_color.1, primary_color.2, primary_color.3);
    
    canvas.draw_text_dots("TIME", col0_x + 30, row0_y + 30, 1, 1, 3, secondary_color.0, secondary_color.1, secondary_color.2, secondary_color.3);
    canvas.draw_circle(col0_x + 350, row0_y + 350, 6, 255, 0, 0, 255);

    // 2. SYS Widget (CPU + RAM) (1x1: 180x180)
    canvas.draw_rounded_rect(col2_x, row0_y, 180, 180, 24, widget_bg_color.0, widget_bg_color.1, widget_bg_color.2, widget_bg_color.3);
    canvas.draw_rounded_rect_border(col2_x, row0_y, 180, 180, 24, 1, widget_border_color.0, widget_border_color.1, widget_border_color.2, widget_border_color.3);
    canvas.draw_text_dots("SYS", col2_x + 20, row0_y + 20, 1, 1, 3, secondary_color.0, secondary_color.1, secondary_color.2, secondary_color.3);
    
    let cpu_str = format!("CPU {}%", state.cpu_usage);
    let ram_str = format!("RAM {}%", state.mem_usage);
    canvas.draw_text_dots(&cpu_str, col2_x + 20, row0_y + 60, 2, 1, 4, primary_color.0, primary_color.1, primary_color.2, primary_color.3);
    canvas.draw_text_dots(&ram_str, col2_x + 20, row0_y + 110, 2, 1, 4, primary_color.0, primary_color.1, primary_color.2, primary_color.3);

    // 3. PWR Widget (Battery + Uptime) (1x1: 180x180)
    canvas.draw_rounded_rect(col2_x, row1_y, 180, 180, 24, widget_bg_color.0, widget_bg_color.1, widget_bg_color.2, widget_bg_color.3);
    canvas.draw_rounded_rect_border(col2_x, row1_y, 180, 180, 24, 1, widget_border_color.0, widget_border_color.1, widget_border_color.2, widget_border_color.3);
    canvas.draw_text_dots("PWR", col2_x + 20, row1_y + 20, 1, 1, 3, secondary_color.0, secondary_color.1, secondary_color.2, secondary_color.3);
    
    let bat_char = if state.battery_charging { "+" } else { "" };
    let bat_str = format!("BAT {}{}%", bat_char, state.battery_percentage);
    
    let upt_hours = state.uptime_secs as f64 / 3600.0;
    let upt_str = if upt_hours >= 1.0 {
        format!("UPT {:.1}H", upt_hours)
    } else {
        format!("UPT {}M", state.uptime_secs / 60)
    };
    canvas.draw_text_dots(&bat_str, col2_x + 20, row1_y + 60, 2, 1, 4, primary_color.0, primary_color.1, primary_color.2, primary_color.3);
    canvas.draw_text_dots(&upt_str, col2_x + 20, row1_y + 110, 2, 1, 4, primary_color.0, primary_color.1, primary_color.2, primary_color.3);

    // 4. WIFI Toggle Widget (1x1: 180x180)
    canvas.draw_rounded_rect(col3_x, row0_y, 180, 180, 24, widget_bg_color.0, widget_bg_color.1, widget_bg_color.2, widget_bg_color.3);
    canvas.draw_rounded_rect_border(col3_x, row0_y, 180, 180, 24, 1, widget_border_color.0, widget_border_color.1, widget_border_color.2, widget_border_color.3);
    canvas.draw_text_dots("WIFI", col3_x + 20, row0_y + 20, 1, 1, 3, secondary_color.0, secondary_color.1, secondary_color.2, secondary_color.3);
    
    let wifi_color = if state.wifi_enabled {
        if state.dark_mode { (255, 255, 255) } else { (0, 0, 0) }
    } else {
        if state.dark_mode { (80, 80, 80) } else { (180, 180, 180) }
    };
    canvas.draw_circle(col3_x + 90, row0_y + 90, 28, wifi_color.0, wifi_color.1, wifi_color.2, 255);
    canvas.draw_circle(col3_x + 90, row0_y + 90, 24, hole_color.0, hole_color.1, hole_color.2, 255);
    canvas.draw_circle(col3_x + 90, row0_y + 90, 4, wifi_color.0, wifi_color.1, wifi_color.2, 255);
    
    let wifi_status = if state.wifi_enabled { "ON" } else { "OFF" };
    let wifi_text_x = col3_x + 90 - (wifi_status.len() * 10) / 2;
    canvas.draw_text_dots(wifi_status, wifi_text_x, row0_y + 145, 1, 1, 3, wifi_color.0, wifi_color.1, wifi_color.2, 255);

    // 5. DARK Toggle Widget (1x1: 180x180)
    canvas.draw_rounded_rect(col3_x, row1_y, 180, 180, 24, widget_bg_color.0, widget_bg_color.1, widget_bg_color.2, widget_bg_color.3);
    canvas.draw_rounded_rect_border(col3_x, row1_y, 180, 180, 24, 1, widget_border_color.0, widget_border_color.1, widget_border_color.2, widget_border_color.3);
    canvas.draw_text_dots("DARK", col3_x + 20, row1_y + 20, 1, 1, 3, secondary_color.0, secondary_color.1, secondary_color.2, secondary_color.3);
    
    let dark_color = if state.dark_mode {
        if state.dark_mode { (255, 255, 255) } else { (0, 0, 0) }
    } else {
        if state.dark_mode { (80, 80, 80) } else { (180, 180, 180) }
    };
    if state.dark_mode {
        canvas.draw_circle(col3_x + 90, row1_y + 90, 24, 255, 255, 255, 255);
        canvas.draw_circle(col3_x + 100, row1_y + 85, 20, hole_color.0, hole_color.1, hole_color.2, 255);
    } else {
        canvas.draw_circle(col3_x + 90, row1_y + 90, 14, 0, 0, 0, 255);
        canvas.draw_circle(col3_x + 90, row1_y + 60, 3, 0, 0, 0, 255);
        canvas.draw_circle(col3_x + 90, row1_y + 120, 3, 0, 0, 0, 255);
        canvas.draw_circle(col3_x + 60, row1_y + 90, 3, 0, 0, 0, 255);
        canvas.draw_circle(col3_x + 120, row1_y + 90, 3, 0, 0, 0, 255);
    }
    
    let dark_status = if state.dark_mode { "ON" } else { "OFF" };
    let dark_text_x = col3_x + 90 - (dark_status.len() * 10) / 2;
    canvas.draw_text_dots(dark_status, dark_text_x, row1_y + 145, 1, 1, 3, dark_color.0, dark_color.1, dark_color.2, 255);

    canvas
}

/// App Launcher fullscreen overlay
pub fn render_app_launcher_canvas(_state: &NothingUiState, width: u32, height: u32) -> NothingCanvas {
    let w = width as usize;
    let h = height as usize;
    let mut canvas = NothingCanvas::new(w, h);
    canvas.clear(0, 0, 0, 204); // 80% black overlay

    let cx = w / 2;

    // Title: NOTHING APPS
    canvas.draw_text_dots("NOTHING APPS", cx - 100, 80, 2, 1, 4, 255, 255, 255, 255);

    // Close button
    canvas.draw_circle(w - 60, 90, 16, 255, 255, 255, 25);
    canvas.draw_text_dots("X", w - 63, 86, 1, 1, 0, 255, 255, 255, 200);

    // Search bar
    let sb_w = 500;
    let sb_x = (w - sb_w) / 2;
    canvas.draw_rounded_rect(sb_x, 140, sb_w, 40, 16, 255, 255, 255, 13);
    canvas.draw_rounded_rect_border(sb_x, 140, sb_w, 40, 16, 1, 255, 255, 255, 25);
    canvas.draw_text_dots("SEARCH APPS...", sb_x + 20, 153, 1, 1, 2, 120, 120, 120, 255);

    // App grid (4 apps in a row)
    let apps = [("TERMINAL", "T"), ("MONITOR", "A"), ("FILES", "F"), ("SETTINGS", "S")];
    let grid_w = 400;
    let grid_x = (w - grid_w) / 2;
    let grid_y = 220;

    for (i, (name, icon)) in apps.iter().enumerate() {
        let ax = grid_x + i * 100 + 10;
        let ay = grid_y;

        // App card
        canvas.draw_rounded_rect(ax, ay, 80, 100, 20, 255, 255, 255, 13);
        canvas.draw_rounded_rect_border(ax, ay, 80, 100, 20, 1, 255, 255, 255, 13);

        // Icon circle
        canvas.draw_circle(ax + 40, ay + 35, 18, 255, 255, 255, 25);
        canvas.draw_text_dots(icon, ax + 37, ay + 31, 1, 1, 0, 255, 255, 255, 255);

        // App name
        let nw = name.len() * 7;
        canvas.draw_text_dots(name, ax + 40 - nw / 2, ay + 70, 1, 0, 1, 255, 255, 255, 200);
    }

    // Build version
    canvas.draw_text_dots("V0.1.0-SMITHAY-RUST", cx - 80, h - 40, 1, 0, 1, 80, 80, 80, 255);

    canvas
}

/// Quick Settings panel (right side overlay, desktop visible behind)
pub fn render_quick_settings_canvas(state: &NothingUiState, layout_mode: crate::layout::LayoutMode, width: u32, height: u32) -> NothingCanvas {
    let w = width as usize;
    let h = height as usize;
    let mut canvas = NothingCanvas::new(w, h);
    canvas.clear(0, 0, 0, 0);

    // Draw the desktop elements underneath
    draw_top_bar(&mut canvas, state, layout_mode, w, 0);
    draw_desktop_widgets(&mut canvas, state);
    draw_bottom_dock(&mut canvas, w, h);

    // Semi-transparent overlay on right side
    let panel_w = 280;
    let panel_x = w - panel_w - 16;
    let panel_y = 64;
    let panel_h = 420;

    canvas.draw_rounded_rect(panel_x, panel_y, panel_w, panel_h, 32, 18, 18, 18, 220);
    canvas.draw_rounded_rect_border(panel_x, panel_y, panel_w, panel_h, 32, 1, 255, 255, 255, 20);

    // Header
    canvas.draw_text_dots("NOTHING SYSTEM", panel_x + 24, panel_y + 24, 1, 1, 2, 160, 160, 160, 255);
    // Close btn
    canvas.draw_circle(panel_x + panel_w - 30, panel_y + 28, 10, 255, 255, 255, 25);
    canvas.draw_text_dots("X", panel_x + panel_w - 33, panel_y + 24, 1, 1, 0, 255, 255, 255, 200);

    // Toggle tiles (2x2 grid)
    let tile_size = 100;
    let tile_gap = 12;
    let tiles_x = panel_x + 24;
    let tiles_y = panel_y + 60;

    let toggles = [
        ("WIFI", state.wifi_enabled),
        ("BT", state.bluetooth_enabled),
        ("SLEEP", false),
        ("ACCENT", true),
    ];

    for (i, (name, active)) in toggles.iter().enumerate() {
        let col = i % 2;
        let row = i / 2;
        let tx = tiles_x + col * (tile_size + tile_gap);
        let ty = tiles_y + row * (tile_size + tile_gap);

        if *active {
            canvas.draw_rounded_rect(tx, ty, tile_size, tile_size, 20, 255, 255, 255, 255);
            canvas.draw_text_dots(name, tx + 12, ty + tile_size - 24, 1, 1, 2, 0, 0, 0, 255);
        } else {
            canvas.draw_rounded_rect(tx, ty, tile_size, tile_size, 20, 255, 255, 255, 13);
            canvas.draw_rounded_rect_border(tx, ty, tile_size, tile_size, 20, 1, 255, 255, 255, 25);
            canvas.draw_text_dots(name, tx + 12, ty + tile_size - 24, 1, 1, 2, 160, 160, 160, 255);
        }
    }

    // Sliders
    let sl_y = tiles_y + 2 * (tile_size + tile_gap) + 16;
    let sl_w = panel_w - 48;

    // Volume
    canvas.draw_text_dots("VOLUME", panel_x + 24, sl_y, 1, 0, 1, 160, 160, 160, 255);
    canvas.draw_rounded_rect(panel_x + 24, sl_y + 16, sl_w, 6, 3, 30, 30, 30, 255);
    let vol_fill = (sl_w as u32 * state.volume as u32 / 100) as usize;
    if vol_fill > 0 {
        canvas.draw_rounded_rect(panel_x + 24, sl_y + 16, vol_fill.max(6), 6, 3, 255, 255, 255, 255);
    }

    // Brightness
    canvas.draw_text_dots("BRIGHTNESS", panel_x + 24, sl_y + 36, 1, 0, 1, 160, 160, 160, 255);
    canvas.draw_rounded_rect(panel_x + 24, sl_y + 52, sl_w, 6, 3, 30, 30, 30, 255);
    let br_fill = (sl_w as u32 * state.brightness as u32 / 100) as usize;
    if br_fill > 0 {
        canvas.draw_rounded_rect(panel_x + 24, sl_y + 52, br_fill.max(6), 6, 3, 255, 255, 255, 255);
    }

    // Build version
    canvas.draw_text_dots("BUILD V0.1.0", panel_x + 70, panel_y + panel_h - 24, 1, 0, 1, 80, 80, 80, 255);

    canvas
}
