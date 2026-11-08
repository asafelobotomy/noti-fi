//! The unread badge on the tray icon: a disc split into the colours of the groups that have unread notifications (each slice as
//! large as its share), with the count on top. Drawn here as ARGB pixels (no Qt), sent as the icon's overlay.

/// One group's unread notifications: its accent colour (#rrggbb) and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub color: [u8; 3],
    pub count: u32,
}

/// "#60a5fa" or "#6af" -> rgb; anything else -> the default blue.
pub fn parse_color(s: &str) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    let hex = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
    let rgb = match h.len() {
        6 | 8 => (|| Some([hex(h.len() - 6, 2)?, hex(h.len() - 4, 2)?, hex(h.len() - 2, 2)?]))(),
        3 => (|| Some([hex(0, 1)? * 17, hex(1, 1)? * 17, hex(2, 1)? * 17]))(),
        _ => None,
    };
    rgb.unwrap_or([0x60, 0xa5, 0xfa])
}

/// The digits 0-9, 3 pixels wide and 5 high, one row per entry (bit 2 = left column).
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b011, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

/// What the badge says: the count, at most 99.
pub fn label(total: u32) -> String {
    total.min(99).to_string()
}

/// The badge at `size` x `size` pixels as ARGB32 in network byte order (what StatusNotifierItem wants); None when nothing is unread.
pub fn render(slices: &[Slice], size: u32) -> Option<Vec<u8>> {
    let total: u32 = slices.iter().map(|s| s.count).sum();
    if total == 0 || size < 8 {
        return None;
    }
    let n = size as usize;
    let mut px = vec![0u8; n * n * 4];
    let r = size as f32 / 2.0;
    // where each slice ends, clockwise from the top, as a fraction of the turn
    let mut ends = Vec::new();
    let mut acc = 0.0;
    for s in slices.iter().filter(|s| s.count > 0) {
        acc += s.count as f32 / total as f32;
        ends.push((acc, s.color));
    }
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let d = (dx * dx + dy * dy).sqrt();
            let cover = (r - d + 0.5).clamp(0.0, 1.0); // smooth edge
            if cover <= 0.0 {
                continue;
            }
            let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
            let mut c = ends.iter().find(|(e, _)| turn < *e).or(ends.last()).map(|(_, c)| *c).unwrap_or([0x60, 0xa5, 0xfa]);
            if d > r - (size as f32 / 16.0).max(1.0) {
                c = c.map(|v| (v as f32 * 0.55) as u8); // a darker rim so the badge stands out on any panel
            }
            put(&mut px, n, x, y, c, cover);
        }
    }
    // the count, white with a dark outline, as large as fits
    let text = label(total);
    let glyphs = text.len() as u32;
    let fit_w = (size as f32 * if glyphs == 1 { 0.55 } else { 0.72 }) / (glyphs * 4 - 1) as f32;
    let fit_h = size as f32 * 0.62 / 5.0;
    let k = fit_w.min(fit_h).floor().max(1.0) as usize;
    let (w, h) = ((glyphs as usize * 4 - 1) * k, 5 * k);
    let (ox, oy) = ((n.saturating_sub(w)) / 2, (n.saturating_sub(h)) / 2);
    let mut ink = vec![false; n * n];
    for (gi, ch) in text.bytes().enumerate() {
        let rows = DIGITS[(ch - b'0') as usize];
        for (ry, bits) in rows.iter().enumerate() {
            for cx in 0..3 {
                if bits & (0b100 >> cx) == 0 {
                    continue;
                }
                for sy in 0..k {
                    for sx in 0..k {
                        let (x, y) = (ox + (gi * 4 + cx) * k + sx, oy + ry * k + sy);
                        if x < n && y < n {
                            ink[y * n + x] = true;
                        }
                    }
                }
            }
        }
    }
    for y in 0..n {
        for x in 0..n {
            if ink[y * n + x] {
                continue;
            }
            let near = (y.saturating_sub(1)..=(y + 1).min(n - 1)).any(|yy| (x.saturating_sub(1)..=(x + 1).min(n - 1)).any(|xx| ink[yy * n + xx]));
            if near {
                put(&mut px, n, x, y, [20, 20, 28], 0.85);
            }
        }
    }
    for (i, on) in ink.iter().enumerate() {
        if *on {
            put(&mut px, n, i % n, i / n, [255, 255, 255], 1.0);
        }
    }
    Some(px)
}

/// Paint one pixel over what is there (ARGB, straight alpha).
fn put(px: &mut [u8], n: usize, x: usize, y: usize, c: [u8; 3], a: f32) {
    let i = (y * n + x) * 4;
    let below = px[i] as f32 / 255.0;
    let out = a + below * (1.0 - a);
    if out <= 0.0 {
        return;
    }
    for k in 0..3 {
        let mixed = (c[k] as f32 * a + px[i + 1 + k] as f32 * below * (1.0 - a)) / out;
        px[i + 1 + k] = mixed.round() as u8;
    }
    px[i] = (out * 255.0).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u8], n: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * n + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn nothing_unread_draws_nothing() {
        assert!(render(&[], 32).is_none());
        assert!(render(&[Slice { color: [255, 0, 0], count: 0 }], 32).is_none());
    }

    #[test]
    fn colours_parse_with_a_fallback() {
        assert_eq!(parse_color("#60a5fa"), [0x60, 0xa5, 0xfa]);
        assert_eq!(parse_color("#fff"), [255, 255, 255]);
        assert_eq!(parse_color("#80ff0000"), [255, 0, 0]);
        assert_eq!(parse_color("blue"), [0x60, 0xa5, 0xfa]);
    }

    #[test]
    fn each_group_gets_a_slice_the_size_of_its_share() {
        let red = [220, 30, 30];
        let green = [30, 200, 60];
        // 3 of 4 unread are red: red runs clockwise from the top for three quarters, green has the last quarter (top-left)
        let px = render(&[Slice { color: red, count: 3 }, Slice { color: green, count: 1 }], 64).unwrap();
        assert_eq!(px.len(), 64 * 64 * 4);
        let right = at(&px, 64, 52, 32); // a quarter turn: red
        let left_top = at(&px, 64, 14, 20); // the last quarter: green
        assert_eq!(&right[1..], &red[..]);
        assert_eq!(&left_top[1..], &green[..]);
        assert_eq!(at(&px, 64, 0, 0)[0], 0, "outside the disc is transparent");
    }

    #[test]
    fn the_count_is_drawn_in_white_and_capped() {
        let px = render(&[Slice { color: [30, 30, 200], count: 7 }], 32).unwrap();
        let white = (0..32 * 32).filter(|i| px[i * 4..i * 4 + 4] == [255, 255, 255, 255]).count();
        assert!(white > 20, "digits drawn: {white} white pixels");
        assert_eq!(label(7), "7");
        assert_eq!(label(250), "99");
        assert!(render(&[Slice { color: [1, 2, 3], count: 250 }], 16).is_some());
    }
}
