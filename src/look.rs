//! How the window looks, as data and without the window: the colours, the mark of the program and the small drawings of the four tabs (all drawn here from a few points, not taken
//! from anywhere), and the layout of the window by its size. The window draws these; the tests check them: that the text can be read on every background, that every drawing stays
//! in its square, and that the parts of the layout fill the window without lying on one another.

use crate::app_state::Tab;

/// A colour, red green blue from 0 to 255.
pub type Rgb = [u8; 3];

/// The colours of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub panel: Rgb,
    pub text: Rgb,
    pub dim: Rgb,
    pub accent: Rgb,
    pub good: Rgb,
    pub danger: Rgb,
    pub line: Rgb,
    pub plate: Rgb,
    /// The surface of the bed in the 3D view and the lines of its grid.
    pub bed: Rgb,
    pub grid: Rgb,
    /// The colour of a part on the bed, and of the part that is selected.
    pub model: Rgb,
    pub selected: Rgb,
    /// The ground of the side panels and of the bars, a step away from the ground of the work.
    pub bar: Rgb,
}

/// The two looks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub const ALL: [Theme; 2] = [Theme::Dark, Theme::Light];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn from_name(text: &str) -> Option<Theme> {
        let t = text.trim().to_ascii_lowercase();
        Theme::ALL.into_iter().find(|x| x.name() == t)
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette::phoenix(),
            Theme::Light => Palette::daylight(),
        }
    }

    /// The other look.
    pub fn toggled(self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

impl Palette {
    /// The look of the program: a dark warm ground with a flame-coloured accent.
    pub const fn phoenix() -> Palette {
        Palette {
            background: [0x16, 0x14, 0x12],
            panel: [0x20, 0x1d, 0x1a],
            text: [0xee, 0xe6, 0xdc],
            dim: [0xa8, 0x9a, 0x88],
            accent: [0xff, 0x6a, 0x3d],
            good: [0x9b, 0xe3, 0x4a],
            danger: [0xff, 0x54, 0x70],
            line: [0x4d, 0xd0, 0xe1],
            plate: [0x6a, 0x76, 0x88],
            bed: [0x2a, 0x26, 0x22],
            grid: [0x4a, 0x44, 0x3e],
            model: [0xd9, 0x9a, 0x6c],
            selected: [0xff, 0xb3, 0x47],
            bar: [0x1c, 0x19, 0x17],
        }
    }

    /// The light look: a warm paper ground with a darker flame accent, so that the text and the colours keep their contrast.
    pub const fn daylight() -> Palette {
        Palette {
            background: [0xf4, 0xf0, 0xea],
            panel: [0xfb, 0xf8, 0xf4],
            text: [0x22, 0x1e, 0x1a],
            dim: [0x5c, 0x52, 0x48],
            accent: [0xb3, 0x3a, 0x12],
            good: [0x2d, 0x6a, 0x12],
            danger: [0xb0, 0x1c, 0x3c],
            line: [0x0b, 0x6a, 0x7a],
            plate: [0x6a, 0x76, 0x88],
            bed: [0xdd, 0xd6, 0xcc],
            grid: [0xb8, 0xae, 0xa0],
            model: [0xc0, 0x6a, 0x3a],
            selected: [0xe0, 0x8a, 0x00],
            bar: [0xe9, 0xe3, 0xda],
        }
    }
}

/// A colour made lighter or darker by a factor: 1 leaves it as it is, less is darker, and every channel stays between 0 and 255. A factor that is not a number leaves the colour.
pub fn scaled(c: Rgb, k: f32) -> Rgb {
    let k = if k.is_finite() {
        k.clamp(0.0, 2.0)
    } else {
        1.0
    };
    c.map(|v| (f32::from(v) * k).round().clamp(0.0, 255.0) as u8)
}

/// The height, in points, of a box that scrolls inside a page of `available` points: under half of what there is, never under 120 (it would show nothing) and never over 360 (a
/// long text would push everything else out of sight). A height that is not a number gives the least.
pub fn scroll_box_height(available: f32) -> f32 {
    if !available.is_finite() {
        return 120.0;
    }
    (available * 0.45).clamp(120.0, 360.0)
}

/// A text cut into lines of at most `max_chars` letters: at a space when there is one in reach, in the middle of a word when there is none (a language with no spaces). Lines
/// are never empty, and no letter is lost or repeated.
pub fn wrap_lines(text: &str, max_chars: usize) -> Vec<String> {
    let max = max_chars.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut len = 0usize;
    for word in text.split_whitespace() {
        let wlen = word.chars().count();
        if len > 0 && len + 1 + wlen <= max {
            line.push(' ');
            line.push_str(word);
            len += 1 + wlen;
            continue;
        }
        if len > 0 {
            lines.push(std::mem::take(&mut line));
        }
        let mut rest = word;
        while rest.chars().count() > max {
            let cut = rest.char_indices().nth(max).map_or(rest.len(), |(i, _)| i);
            lines.push(rest[..cut].to_string());
            rest = &rest[cut..];
        }
        line.push_str(rest);
        len = rest.chars().count();
    }
    if len > 0 {
        lines.push(line);
    }
    lines
}

/// Of two colours for a text, the one that reads better on a ground.
pub fn readable_on(ground: Rgb, a: Rgb, b: Rgb) -> Rgb {
    if contrast(a, ground) >= contrast(b, ground) {
        a
    } else {
        b
    }
}

fn channel(c: u8) -> f64 {
    let s = f64::from(c) / 255.0;
    if s <= 0.03928 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// The relative luminance of a colour (WCAG 2).
pub fn luminance(c: Rgb) -> f64 {
    0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
}

/// The contrast between two colours, from 1 (the same) to 21 (black on white).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// A drawing: lines of points in a square of side 1, from the top left.
pub type Drawing = Vec<Vec<[f32; 2]>>;

fn bezier(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            [
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]
        })
        .collect()
}

/// The mark of the program: three flames that rise from one base, the middle one the tallest. It is the same figure at any size.
pub fn logo() -> Drawing {
    let flame = |x0: f32, x1: f32, top: f32, lean: f32| {
        let mid = (x0 + x1) / 2.0;
        let mut line = bezier([x0, 0.92], [x0 - 0.08, 0.55], [mid + lean, top], 12);
        line.extend(
            bezier([mid + lean, top], [x1 + 0.08, 0.55], [x1, 0.92], 12)
                .into_iter()
                .skip(1),
        );
        line
    };
    vec![
        flame(0.10, 0.50, 0.40, -0.04),
        flame(0.50, 0.90, 0.36, 0.04),
        flame(0.28, 0.72, 0.06, 0.0),
    ]
}

/// The small drawing of a tab, in the same square.
pub fn glyph(tab: Tab) -> Drawing {
    match tab {
        // a cube seen from a corner
        Tab::Prepare => vec![
            vec![
                [0.5, 0.08],
                [0.9, 0.3],
                [0.9, 0.72],
                [0.5, 0.94],
                [0.1, 0.72],
                [0.1, 0.3],
                [0.5, 0.08],
            ],
            vec![[0.1, 0.3], [0.5, 0.52], [0.9, 0.3]],
            vec![[0.5, 0.52], [0.5, 0.94]],
        ],
        // layers: stacked lines
        Tab::Preview => (0..4)
            .map(|i| {
                let y = 0.2 + 0.2 * i as f32;
                vec![[0.1, y], [0.9, y]]
            })
            .collect(),
        // a printer: a frame, a gantry and a bed
        Tab::Device => vec![
            vec![
                [0.12, 0.1],
                [0.88, 0.1],
                [0.88, 0.9],
                [0.12, 0.9],
                [0.12, 0.1],
            ],
            vec![[0.12, 0.34], [0.88, 0.34]],
            vec![[0.5, 0.34], [0.5, 0.52]],
            vec![[0.25, 0.8], [0.75, 0.8]],
        ],
        // a folder
        Tab::Project => vec![vec![
            [0.08, 0.2],
            [0.38, 0.2],
            [0.46, 0.32],
            [0.92, 0.32],
            [0.92, 0.84],
            [0.08, 0.84],
            [0.08, 0.2],
        ]],
    }
}

/// A rectangle of the window, in points from the top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn area(&self) -> f32 {
        self.w * self.h
    }

    /// Whether two rectangles share some area (touching edges do not).
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w - 1e-3
            && o.x < self.x + self.w - 1e-3
            && self.y < o.y + o.h - 1e-3
            && o.y < self.y + self.h - 1e-3
    }
}

/// The height of the bar at the top (the tabs and the language) and of the bar at the bottom (the status).
pub const TOP_BAR: f32 = 44.0;
pub const STATUS_BAR: f32 = 28.0;
/// A window narrower than this folds the list at the side into the tab.
pub const WIDE: f32 = 1000.0;
/// A window wider than this also shows the server and the queue at the right.
pub const VERY_WIDE: f32 = 1500.0;

/// The parts of the window for its size: the same arrangement in every program of the family (the bars on top and at the bottom, a list at the left, the work in the middle and, on a
/// wide screen, the server at the right).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub top: Rect,
    pub status: Rect,
    pub side: Option<Rect>,
    pub centre: Rect,
    pub right: Option<Rect>,
}

impl Layout {
    pub fn for_size(width: f32, height: f32) -> Layout {
        let (w, h) = (width.max(1.0), height.max(1.0));
        let top_h = TOP_BAR.min(h / 4.0);
        let status_h = STATUS_BAR.min(h / 4.0);
        let body_h = (h - top_h - status_h).max(0.0);
        let side_w = (w * 0.24).clamp(260.0, 380.0);
        let right_w = (w * 0.20).clamp(300.0, 420.0);
        let has_side = w >= WIDE;
        let has_right = w >= VERY_WIDE;
        let side_used = if has_side { side_w } else { 0.0 };
        let right_used = if has_right { right_w } else { 0.0 };
        let rect = |x, y, w, h| Rect { x, y, w, h };
        Layout {
            top: rect(0.0, 0.0, w, top_h),
            status: rect(0.0, h - status_h, w, status_h),
            side: has_side.then(|| rect(0.0, top_h, side_w, body_h)),
            centre: rect(side_used, top_h, w - side_used - right_used, body_h),
            right: has_right.then(|| rect(w - right_w, top_h, right_w, body_h)),
        }
    }

    /// All the parts that are there, for the window to place and for the tests to add up.
    pub fn parts(&self) -> Vec<Rect> {
        let mut v = vec![self.top, self.status, self.centre];
        v.extend(self.side);
        v.extend(self.right);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_can_be_read_on_every_background_it_is_put_on() {
        let p = Palette::phoenix();
        for bg in [p.background, p.panel] {
            assert!(contrast(p.text, bg) >= 7.0, "text on {bg:?}");
            assert!(contrast(p.dim, bg) >= 4.5, "dim text on {bg:?}");
            for (name, c) in [
                ("accent", p.accent),
                ("good", p.good),
                ("danger", p.danger),
                ("line", p.line),
            ] {
                assert!(
                    contrast(c, bg) >= 4.5,
                    "{name} on {bg:?}: {:.2}",
                    contrast(c, bg)
                );
            }
        }
        // the plate and the lines of the model can be told apart from the ground
        assert!(contrast(p.plate, p.background) >= 3.0);
        assert!(contrast(p.line, p.plate) >= 1.5);
    }

    #[test]
    fn the_light_look_reads_as_well_as_the_dark_one_on_every_ground() {
        for theme in Theme::ALL {
            let p = theme.palette();
            for bg in [p.background, p.panel, p.bar] {
                assert!(
                    contrast(p.text, bg) >= 7.0,
                    "{} text on {bg:?}",
                    theme.name()
                );
                assert!(contrast(p.dim, bg) >= 4.5, "{} dim on {bg:?}", theme.name());
                for (name, c) in [
                    ("accent", p.accent),
                    ("good", p.good),
                    ("danger", p.danger),
                    ("line", p.line),
                ] {
                    assert!(
                        contrast(c, bg) >= 4.5,
                        "{} {name} on {bg:?}: {:.2}",
                        theme.name(),
                        contrast(c, bg)
                    );
                }
            }
            // a part, the one selected and the grid can be told from the bed they lie on
            assert!(contrast(p.model, p.bed) >= 1.5, "{} model", theme.name());
            assert!(
                contrast(p.selected, p.bed) >= 1.5,
                "{} selected",
                theme.name()
            );
            assert!(contrast(p.grid, p.bed) >= 1.15, "{} grid", theme.name());
            assert_ne!(p.model, p.selected);
        }
        assert_ne!(Palette::phoenix(), Palette::daylight());
    }

    #[test]
    fn the_track_and_the_fill_of_a_bar_can_be_told_from_the_ground_in_both_looks() {
        for theme in Theme::ALL {
            let p = theme.palette();
            assert!(contrast(p.grid, p.panel) >= 1.5, "{} track", theme.name());
            assert!(
                contrast(p.grid, p.background) >= 1.5,
                "{} track",
                theme.name()
            );
            assert!(contrast(p.accent, p.grid) >= 2.0, "{} fill", theme.name());
            // the text over the ground of a page is readable, and the text that names a bar sits on that ground
            assert!(contrast(p.text, p.panel) >= 7.0, "{} text", theme.name());
            assert!(contrast(p.dim, p.panel) >= 4.5, "{} dim", theme.name());
        }
    }

    #[test]
    fn a_box_that_scrolls_is_between_a_third_of_a_page_and_a_screenful() {
        assert_eq!(scroll_box_height(f32::NAN), 120.0);
        assert_eq!(scroll_box_height(f32::INFINITY), 120.0);
        assert_eq!(scroll_box_height(100.0), 120.0);
        assert_eq!(scroll_box_height(600.0), 270.0);
        assert_eq!(scroll_box_height(5000.0), 360.0);
        let mut last = 0.0;
        for a in (0..3000).step_by(50) {
            let h = scroll_box_height(a as f32);
            assert!(h >= last && (120.0..=360.0).contains(&h));
            last = h;
        }
    }

    #[test]
    fn a_text_is_cut_into_lines_that_fit_and_lose_nothing() {
        assert_eq!(wrap_lines("", 10), Vec::<String>::new());
        assert_eq!(wrap_lines("   ", 10), Vec::<String>::new());
        assert_eq!(wrap_lines("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap_lines("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(
            wrap_lines("a b", 0),
            vec!["a", "b"],
            "a width of nothing is one letter"
        );
        // a language with no spaces is cut by letters
        let ja = "モデルのパスを入力して開くを押すか、デモを読み込みます";
        let lines = wrap_lines(ja, 10);
        assert!(lines
            .iter()
            .all(|l| !l.is_empty() && l.chars().count() <= 10));
        assert_eq!(lines.concat(), ja);
        // the words of every text of the empty bed keep all their letters, in every language
        for l in crate::i18n::Lang::ALL {
            let t = crate::i18n::Msg::EmptyBedHint.tr(l);
            let lines = wrap_lines(t, 40);
            assert!(
                lines.iter().all(|x| x.chars().count() <= 40),
                "{}",
                l.code()
            );
            let squeezed: String = t.split_whitespace().collect();
            assert_eq!(
                lines.concat().split_whitespace().collect::<String>(),
                squeezed
            );
        }
    }

    #[test]
    fn the_text_of_a_button_is_the_one_that_reads_better_on_its_colour() {
        for theme in Theme::ALL {
            let p = theme.palette();
            let t = readable_on(p.accent, p.text, p.background);
            assert!(contrast(t, p.accent) >= 3.0, "{}: {t:?}", theme.name());
            assert!(contrast(t, p.accent) >= contrast(p.dim, p.accent));
        }
        assert_eq!(
            readable_on([0, 0, 0], [255, 255, 255], [10, 10, 10]),
            [255, 255, 255]
        );
        assert_eq!(
            readable_on([255, 255, 255], [250, 250, 250], [5, 5, 5]),
            [5, 5, 5]
        );
    }

    #[test]
    fn a_colour_is_lit_and_darkened_inside_its_range() {
        assert_eq!(scaled([100, 200, 250], 1.0), [100, 200, 250]);
        assert_eq!(scaled([100, 200, 250], 0.5), [50, 100, 125]);
        assert_eq!(scaled([100, 200, 250], 2.0), [200, 255, 255]);
        assert_eq!(scaled([100, 200, 250], 9.0), [200, 255, 255]);
        assert_eq!(scaled([100, 200, 250], -1.0), [0, 0, 0]);
        assert_eq!(scaled([100, 200, 250], f32::NAN), [100, 200, 250]);
    }

    #[test]
    fn the_theme_is_read_by_name_and_toggles() {
        assert_eq!(Theme::default(), Theme::Dark);
        assert_eq!(Theme::from_name(" LIGHT "), Some(Theme::Light));
        assert_eq!(Theme::from_name("dark"), Some(Theme::Dark));
        assert_eq!(Theme::from_name("sepia"), None);
        assert_eq!(Theme::Dark.toggled().toggled(), Theme::Dark);
        assert_eq!(Theme::Light.palette(), Palette::daylight());
    }

    #[test]
    fn contrast_is_one_for_equal_colours_and_twenty_one_for_black_on_white() {
        assert!((contrast([9, 9, 9], [9, 9, 9]) - 1.0).abs() < 1e-12);
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 1e-9);
        assert_eq!(
            contrast([10, 20, 30], [200, 210, 220]),
            contrast([200, 210, 220], [10, 20, 30])
        );
    }

    #[test]
    fn every_drawing_stays_in_its_square_and_is_made_of_real_lines() {
        let mut all = vec![("logo".to_string(), logo())];
        for t in Tab::ALL {
            all.push((t.key().to_string(), glyph(t)));
        }
        for (name, d) in &all {
            assert!(!d.is_empty(), "{name}");
            for line in d {
                assert!(line.len() >= 2, "{name}");
                for p in line {
                    assert!(
                        p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                        "{name}: {p:?}"
                    );
                }
                assert!(
                    line.windows(2).any(|w| w[0] != w[1]),
                    "{name}: a line that goes nowhere"
                );
            }
        }
        // the four glyphs are four different drawings, and the logo is the three flames
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(glyph(Tab::ALL[i]), glyph(Tab::ALL[j]));
            }
        }
        assert_eq!(logo().len(), 3);
        assert_eq!(logo(), logo(), "the mark is the same every time");
    }

    #[test]
    fn the_flames_close_on_their_base_and_the_middle_one_is_the_tallest() {
        let flames = logo();
        for f in &flames {
            let (a, b) = (f.first().unwrap(), f.last().unwrap());
            assert!((a[1] - 0.92).abs() < 1e-6 && (b[1] - 0.92).abs() < 1e-6);
        }
        let top = |f: &Vec<[f32; 2]>| f.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!(top(&flames[2]) < top(&flames[0]) && top(&flames[2]) < top(&flames[1]));
    }

    #[test]
    fn the_parts_of_the_layout_fill_the_window_and_never_lie_on_one_another() {
        for (w, h) in [
            (320.0, 240.0),
            (760.0, 480.0),
            (999.0, 700.0),
            (1000.0, 700.0),
            (1280.0, 800.0),
            (1499.0, 900.0),
            (1500.0, 900.0),
            (1920.0, 1080.0),
            (3840.0, 2160.0),
            (5000.0, 400.0),
        ] {
            let l = Layout::for_size(w, h);
            let parts = l.parts();
            let total: f32 = parts.iter().map(Rect::area).sum();
            assert!((total - w * h).abs() < 1.0, "{w}x{h}: {total}");
            for (i, a) in parts.iter().enumerate() {
                assert!(
                    a.x >= -1e-3 && a.y >= -1e-3 && a.x + a.w <= w + 1e-3 && a.y + a.h <= h + 1e-3,
                    "{w}x{h}: {a:?}"
                );
                assert!(a.w > 0.0 && a.h >= 0.0);
                for b in &parts[i + 1..] {
                    assert!(!a.overlaps(b), "{w}x{h}: {a:?} and {b:?}");
                }
            }
        }
    }

    #[test]
    fn the_list_at_the_side_and_the_server_at_the_right_come_with_the_width() {
        let narrow = Layout::for_size(WIDE - 1.0, 800.0);
        assert!(narrow.side.is_none() && narrow.right.is_none());
        let wide = Layout::for_size(WIDE, 800.0);
        assert!(wide.side.is_some() && wide.right.is_none());
        let very = Layout::for_size(VERY_WIDE, 800.0);
        assert!(very.side.is_some() && very.right.is_some());
        for l in [wide, very] {
            let s = l.side.unwrap();
            assert!((260.0..=380.0).contains(&s.w));
            assert!(l.centre.w >= 600.0, "{}", l.centre.w);
        }
        let r = very.right.unwrap();
        assert!((300.0..=420.0).contains(&r.w));
        // the bars keep their height in a normal window and give way in a very low one
        assert_eq!((wide.top.h, wide.status.h), (TOP_BAR, STATUS_BAR));
        let low = Layout::for_size(1200.0, 100.0);
        assert!(low.top.h + low.status.h <= 50.0 + 1e-3 && low.centre.h >= 50.0 - 1e-3);
        assert_eq!(wide.status.y + wide.status.h, 800.0);
    }
}
