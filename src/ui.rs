use std::time::Instant;
use smithay::utils::{Logical, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Desktop,
    Dashboard,
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
        }
    }

    pub fn update(&mut self) {
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

pub fn render_dock_canvas(state: &NothingUiState, width: u32, _height: u32) -> NothingCanvas {
    let dock_w = 900.min(width as usize - 40);
    let dock_h = 56;
    let mut canvas = NothingCanvas::new(dock_w, dock_h);
    
    // Draw semi-transparent black background
    canvas.draw_rounded_rect(0, 0, dock_w, dock_h, 28, 0, 0, 0, 230);
    
    // Draw thin gray border
    canvas.draw_rounded_rect_border(0, 0, dock_w, dock_h, 28, 1, 60, 60, 60, 255);
    
    // Left: 3x3 dot matrix logo
    let logo_x = 24;
    let logo_y = 18;
    for r in 0..3 {
        for c in 0..3 {
            canvas.draw_circle(logo_x + c * 10 + 4, logo_y + r * 10 + 4, 3, 255, 255, 255, 255);
        }
    }
    
    // Draw "NOTHING" dot matrix text
    canvas.draw_text_dots("NOTHING", 64, 21, 1, 1, 4, 255, 255, 255, 255);

    // Right: Clock & Stats
    let local_time = chrono::Local::now();
    let clock_str = local_time.format("%H:%M:%S").to_string();
    
    let stats_str = format!("CPU {}%  RAM {}%  BAT {}%", state.cpu_usage, state.mem_usage, state.battery_percentage);
    
    let stats_x = dock_w.saturating_sub(420);
    let clock_x = dock_w.saturating_sub(120);
    
    canvas.draw_text_dots(&stats_str, stats_x, 21, 1, 1, 3, 160, 160, 160, 255);
    canvas.draw_text_dots(&clock_str, clock_x, 21, 1, 1, 3, 255, 255, 255, 255);
    
    // Red dot accent
    canvas.draw_circle(dock_w - 24, 28, 4, 255, 0, 0, 255);

    canvas
}

pub fn render_dashboard_canvas(state: &NothingUiState, width: u32, height: u32) -> NothingCanvas {
    let mut canvas = NothingCanvas::new(width as usize, height as usize);
    
    // 85% opacity dark overlay
    canvas.clear(10, 10, 10, 217);
    
    // Center the grid of widgets (780x380)
    let dx_offset = (width as i32 - 780) / 2;
    let dy_offset = (height as i32 - 380) / 2;
    
    let col0_x = (dx_offset + 0) as usize;
    let col2_x = (dx_offset + 400) as usize;
    let col3_x = (dx_offset + 600) as usize;
    
    let row0_y = (dy_offset + 0) as usize;
    let row1_y = (dy_offset + 200) as usize;

    // 1. Clock Widget (2x2 grid size: 380x380)
    canvas.draw_rounded_rect(col0_x, row0_y, 380, 380, 24, 0, 0, 0, 240);
    canvas.draw_rounded_rect_border(col0_x, row0_y, 380, 380, 24, 1, 80, 80, 80, 255);
    
    let local_time = chrono::Local::now();
    let hh_str = local_time.format("%H").to_string();
    let mm_str = local_time.format("%M").to_string();
    
    // Big stacked clock
    let clock_dx = col0_x + 106; // Centered offset: (380 - (30 * 2 + 12 = 72)) / 2 is actually 154, let's adjust for wider spacing or letters
    let hh_dy = row0_y + 80;
    let mm_dy = row0_y + 190;
    
    canvas.draw_text_dots(&hh_str, clock_dx, hh_dy, 6, 2, 16, 255, 255, 255, 255);
    canvas.draw_text_dots(&mm_str, clock_dx, mm_dy, 6, 2, 16, 255, 255, 255, 255);
    
    canvas.draw_text_dots("TIME", col0_x + 30, row0_y + 30, 1, 1, 3, 140, 140, 140, 255);
    canvas.draw_circle(col0_x + 350, row0_y + 350, 6, 255, 0, 0, 255);

    // 2. SYS Widget (CPU + RAM) (1x1: 180x180)
    canvas.draw_rounded_rect(col2_x, row0_y, 180, 180, 24, 0, 0, 0, 240);
    canvas.draw_rounded_rect_border(col2_x, row0_y, 180, 180, 24, 1, 80, 80, 80, 255);
    canvas.draw_text_dots("SYS", col2_x + 20, row0_y + 20, 1, 1, 3, 140, 140, 140, 255);
    
    let cpu_str = format!("CPU {}%", state.cpu_usage);
    let ram_str = format!("RAM {}%", state.mem_usage);
    canvas.draw_text_dots(&cpu_str, col2_x + 20, row0_y + 60, 2, 1, 4, 255, 255, 255, 255);
    canvas.draw_text_dots(&ram_str, col2_x + 20, row0_y + 110, 2, 1, 4, 255, 255, 255, 255);

    // 3. PWR Widget (Battery + Uptime) (1x1: 180x180)
    canvas.draw_rounded_rect(col2_x, row1_y, 180, 180, 24, 0, 0, 0, 240);
    canvas.draw_rounded_rect_border(col2_x, row1_y, 180, 180, 24, 1, 80, 80, 80, 255);
    canvas.draw_text_dots("PWR", col2_x + 20, row1_y + 20, 1, 1, 3, 140, 140, 140, 255);
    
    let bat_char = if state.battery_charging { "+" } else { "" };
    let bat_str = format!("BAT {}{}%", bat_char, state.battery_percentage);
    
    let upt_hours = state.uptime_secs as f64 / 3600.0;
    let upt_str = if upt_hours >= 1.0 {
        format!("UPT {:.1}H", upt_hours)
    } else {
        format!("UPT {}M", state.uptime_secs / 60)
    };
    canvas.draw_text_dots(&bat_str, col2_x + 20, row1_y + 60, 2, 1, 4, 255, 255, 255, 255);
    canvas.draw_text_dots(&upt_str, col2_x + 20, row1_y + 110, 2, 1, 4, 255, 255, 255, 255);

    // 4. WIFI Toggle Widget (1x1: 180x180)
    canvas.draw_rounded_rect(col3_x, row0_y, 180, 180, 24, 0, 0, 0, 240);
    canvas.draw_rounded_rect_border(col3_x, row0_y, 180, 180, 24, 1, 80, 80, 80, 255);
    canvas.draw_text_dots("WIFI", col3_x + 20, row0_y + 20, 1, 1, 3, 140, 140, 140, 255);
    
    let wifi_color = if state.wifi_enabled { (255, 255, 255) } else { (80, 80, 80) };
    canvas.draw_circle(col3_x + 90, row0_y + 90, 28, wifi_color.0, wifi_color.1, wifi_color.2, 255);
    canvas.draw_circle(col3_x + 90, row0_y + 90, 24, 0, 0, 0, 255);
    canvas.draw_circle(col3_x + 90, row0_y + 90, 4, wifi_color.0, wifi_color.1, wifi_color.2, 255);
    
    let wifi_status = if state.wifi_enabled { "ON" } else { "OFF" };
    let wifi_text_x = col3_x + 90 - (wifi_status.len() * 10) / 2;
    canvas.draw_text_dots(wifi_status, wifi_text_x, row0_y + 145, 1, 1, 3, wifi_color.0, wifi_color.1, wifi_color.2, 255);

    // 5. DARK Toggle Widget (1x1: 180x180)
    canvas.draw_rounded_rect(col3_x, row1_y, 180, 180, 24, 0, 0, 0, 240);
    canvas.draw_rounded_rect_border(col3_x, row1_y, 180, 180, 24, 1, 80, 80, 80, 255);
    canvas.draw_text_dots("DARK", col3_x + 20, row1_y + 20, 1, 1, 3, 140, 140, 140, 255);
    
    let dark_color = if state.dark_mode { (255, 255, 255) } else { (80, 80, 80) };
    if state.dark_mode {
        canvas.draw_circle(col3_x + 90, row1_y + 90, 24, 255, 255, 255, 255);
        canvas.draw_circle(col3_x + 100, row1_y + 85, 20, 0, 0, 0, 255);
    } else {
        canvas.draw_circle(col3_x + 90, row1_y + 90, 14, 80, 80, 80, 255);
        canvas.draw_circle(col3_x + 90, row1_y + 60, 3, 80, 80, 80, 255);
        canvas.draw_circle(col3_x + 90, row1_y + 120, 3, 80, 80, 80, 255);
        canvas.draw_circle(col3_x + 60, row1_y + 90, 3, 80, 80, 80, 255);
        canvas.draw_circle(col3_x + 120, row1_y + 90, 3, 80, 80, 80, 255);
    }
    
    let dark_status = if state.dark_mode { "ON" } else { "OFF" };
    let dark_text_x = col3_x + 90 - (dark_status.len() * 10) / 2;
    canvas.draw_text_dots(dark_status, dark_text_x, row1_y + 145, 1, 1, 3, dark_color.0, dark_color.1, dark_color.2, 255);

    canvas
}
