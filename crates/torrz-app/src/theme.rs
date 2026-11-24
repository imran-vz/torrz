//! Graphite: the app's colours, type and icons, as the mock drew them.

use rune::{Color, El, LineCap, Shape, TextStyle, canvas};

/// sRGB bytes decoded to linear light (Rune's colours are linear), so the palette can be `const`.
#[rustfmt::skip]
const LINEAR: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.000910581, 0.00121411, 0.00151763, 0.00182116, 0.00212469,
    0.00242822, 0.00273174, 0.00303527, 0.00334654, 0.00367651, 0.00402472, 0.00439144, 0.00477695,
    0.00518152, 0.00560539, 0.00604883, 0.00651209, 0.00699541, 0.00749903, 0.00802319, 0.00856813,
    0.00913406, 0.00972122, 0.0103298, 0.0109601, 0.0116122, 0.0122865, 0.012983, 0.0137021,
    0.0144438, 0.0152085, 0.0159963, 0.0168074, 0.017642, 0.0185002, 0.0193824, 0.0202886,
    0.021219, 0.0221739, 0.0231534, 0.0241576, 0.0251869, 0.0262412, 0.0273209, 0.028426,
    0.0295568, 0.0307134, 0.031896, 0.0331048, 0.0343398, 0.0356013, 0.0368895, 0.0382044,
    0.0395462, 0.0409152, 0.0423114, 0.043735, 0.0451862, 0.0466651, 0.0481718, 0.0497066,
    0.0512695, 0.0528606, 0.0544803, 0.0561285, 0.0578054, 0.0595112, 0.0612461, 0.06301,
    0.0648033, 0.0666259, 0.0684782, 0.0703601, 0.0722719, 0.0742136, 0.0761854, 0.0781874,
    0.0802198, 0.0822827, 0.0843762, 0.0865005, 0.0886556, 0.0908417, 0.093059, 0.0953075,
    0.0975873, 0.0998987, 0.102242, 0.104616, 0.107023, 0.109462, 0.111932, 0.114435,
    0.116971, 0.119538, 0.122139, 0.124772, 0.127438, 0.130136, 0.132868, 0.135633,
    0.138432, 0.141263, 0.144128, 0.147027, 0.14996, 0.152926, 0.155926, 0.158961,
    0.162029, 0.165132, 0.168269, 0.171441, 0.174647, 0.177888, 0.181164, 0.184475,
    0.187821, 0.191202, 0.194618, 0.198069, 0.201556, 0.205079, 0.208637, 0.212231,
    0.215861, 0.219526, 0.223228, 0.226966, 0.23074, 0.234551, 0.238398, 0.242281,
    0.246201, 0.250158, 0.254152, 0.258183, 0.262251, 0.266356, 0.270498, 0.274677,
    0.278894, 0.283149, 0.287441, 0.291771, 0.296138, 0.300544, 0.304987, 0.309469,
    0.313989, 0.318547, 0.323143, 0.327778, 0.332452, 0.337164, 0.341914, 0.346704,
    0.351533, 0.3564, 0.361307, 0.366253, 0.371238, 0.376262, 0.381326, 0.386429,
    0.391572, 0.396755, 0.401978, 0.40724, 0.412543, 0.417885, 0.423268, 0.42869,
    0.434154, 0.439657, 0.445201, 0.450786, 0.456411, 0.462077, 0.467784, 0.473531,
    0.47932, 0.48515, 0.491021, 0.496933, 0.502886, 0.508881, 0.514918, 0.520996,
    0.527115, 0.533276, 0.539479, 0.545724, 0.552011, 0.55834, 0.564712, 0.571125,
    0.57758, 0.584078, 0.590619, 0.597202, 0.603827, 0.610496, 0.617207, 0.62396,
    0.630757, 0.637597, 0.64448, 0.651406, 0.658375, 0.665387, 0.672443, 0.679542,
    0.686685, 0.693872, 0.701102, 0.708376, 0.715694, 0.723055, 0.730461, 0.73791,
    0.745404, 0.752942, 0.760525, 0.768151, 0.775822, 0.783538, 0.791298, 0.799103,
    0.806952, 0.814847, 0.822786, 0.83077, 0.838799, 0.846873, 0.854993, 0.863157,
    0.871367, 0.879622, 0.887923, 0.896269, 0.904661, 0.913099, 0.921582, 0.930111,
    0.938686, 0.947307, 0.955973, 0.964686, 0.973445, 0.982251, 0.991102, 1.0,
];

/// A colour from its sRGB hex, as the mock wrote them.
const fn rgb(v: u32) -> Color {
    Color { r: LINEAR[((v >> 16) & 255) as usize], g: LINEAR[((v >> 8) & 255) as usize], b: LINEAR[(v & 255) as usize], a: 1.0 }
}

pub const BG: Color = rgb(0x19191b);
pub const ROW_ALT: Color = rgb(0x1e1e21);
pub const SIDE_TOP: Color = rgb(0x2a2a2e);
pub const SIDE_BOTTOM: Color = rgb(0x242427);
pub const SIDE_ON: Color = rgb(0x3b3b41);
pub const BAR: Color = rgb(0x222225);
pub const HEADER: Color = rgb(0x1d1d20);
pub const PANEL: Color = rgb(0x202023);
pub const FIELD: Color = rgb(0x16161a);
pub const SHEET: Color = rgb(0x2a2a2e);
pub const PALETTE: Color = rgb(0x242428);

pub const EDGE: Color = rgb(0x0b0b0c);
pub const RULE: Color = rgb(0x2d2d31);
pub const RULE_STRONG: Color = rgb(0x3a3a40);
pub const TRACK: Color = rgb(0x37373c);

pub const TEXT: Color = rgb(0xececef);
pub const TEXT_2: Color = rgb(0xc8c8cd);
pub const MUTED: Color = rgb(0x8b8b92);
pub const FAINT: Color = rgb(0x6f6f76);
pub const WHITE: Color = rgb(0xffffff);

pub const BLUE: Color = rgb(0x4c9bff);
pub const BLUE_TEXT: Color = rgb(0x6aaeff);
pub const SELECT: Color = rgb(0x1f5fd0);
pub const GREEN: Color = rgb(0x3ecf7a);
pub const GREEN_TEXT: Color = rgb(0x4fd48a);
pub const AMBER: Color = rgb(0xf0b23e);
pub const RED: Color = rgb(0xff6b5e);
pub const GREY: Color = rgb(0x77777e);
pub const KNOB_OFF: Color = rgb(0x55555c);
pub const PURPLE: Color = rgb(0xc77dff);

pub const HOVER: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 };
pub const PRESS: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.11 };

const TNUM: &str = "'tnum' 1";
pub const BODY: TextStyle = TextStyle::new(12.0, 400.0).features(TNUM);
pub const BODY_M: TextStyle = TextStyle::new(12.0, 500.0).features(TNUM);
pub const SMALL: TextStyle = TextStyle::new(11.0, 400.0).features(TNUM);
pub const SMALL_M: TextStyle = TextStyle::new(11.0, 500.0).features(TNUM);
pub const TINY_B: TextStyle = TextStyle::new(10.5, 600.0);
pub const SIDE: TextStyle = TextStyle::new(12.5, 400.0);
pub const TITLE: TextStyle = TextStyle::new(13.5, 650.0);
pub const HEAD: TextStyle = TextStyle::new(13.0, 600.0);
pub const MONO: TextStyle = TextStyle::new(11.0, 400.0).mono();
pub const INPUT: TextStyle = TextStyle::new(12.5, 400.0);
pub const PALETTE_INPUT: TextStyle = TextStyle::new(16.0, 400.0);

/// Category dots cycle through these.
pub const DOTS: [Color; 5] = [AMBER, PURPLE, GREEN_TEXT, BLUE_TEXT, RED];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Plus,
    Link,
    Pause,
    Play,
    Trash,
    Search,
    Down,
    Up,
    Folder,
    Check,
    All,
    Warn,
    Sliders,
    Gauge,
    Sidebar,
    Panel,
    Command,
    Power,
    Sort,
}

impl Icon {
    /// Stroked paths in a 16 point box.
    fn paths(self) -> &'static [&'static str] {
        match self {
            Icon::Plus => &["M8 3v10M3 8h10"],
            Icon::Link => &["M6.8 9.2l2.4-2.4", "M7 4.6l1.2-1.2a2.6 2.6 0 013.7 3.7L10.6 8.4", "M9 11.4l-1.2 1.2a2.6 2.6 0 01-3.7-3.7l1.3-1.3"],
            Icon::Pause => &["M5.5 3.5v9M10.5 3.5v9"],
            Icon::Play => &["M5 3.5l7.5 4.5L5 12.5z"],
            Icon::Trash => &["M3 4.5h10M6 4.5V3h4v1.5M4.5 4.5l.5 8.5h6l.5-8.5"],
            Icon::Search => &["M11 7a4 4 0 11-8 0 4 4 0 018 0z", "M10 10l3.5 3.5"],
            Icon::Down => &["M8 3v9M4.5 8.5L8 12l3.5-3.5"],
            Icon::Up => &["M8 13V4M4.5 7.5L8 4l3.5 3.5"],
            Icon::Folder => &["M2 4.5h4l1.5 1.5H14v6.5H2z"],
            Icon::Check => &["M3.5 8.5l3 3 6-7"],
            Icon::All => &["M2.5 4.5h11M2.5 8h11M2.5 11.5h11"],
            Icon::Warn => &["M8 2.5l6 10.5H2z", "M8 7v3"],
            Icon::Sliders => &["M3 5h10M3 11h10", "M7.6 5a1.6 1.6 0 11-3.2 0 1.6 1.6 0 013.2 0z", "M11.6 11a1.6 1.6 0 11-3.2 0 1.6 1.6 0 013.2 0z"],
            Icon::Gauge => &["M3 11a5.5 5.5 0 1110 0", "M8 10l2.5-3.5"],
            Icon::Sidebar => &["M2.5 3.5h11v9h-11z", "M6 3.5v9"],
            Icon::Panel => &["M2.5 3.5h11v9h-11z", "M2.5 9h11"],
            Icon::Command => &["M6 6h4v4H6z", "M6 6V4.5A1.5 1.5 0 104.5 6H6", "M10 6h1.5A1.5 1.5 0 1010 4.5V6", "M10 10v1.5a1.5 1.5 0 101.5-1.5H10", "M6 10H4.5A1.5 1.5 0 106 11.5V10"],
            Icon::Power => &["M8 2.5v5", "M4.6 4.6a5 5 0 106.8 0"],
            Icon::Sort => &["M5 3v10M2.5 10.5L5 13l2.5-2.5", "M11 13V3M8.5 5.5L11 3l2.5 2.5"],
        }
    }
}

pub fn icon(i: Icon, size: f32, color: Color) -> El {
    let mut shape = Shape::new();
    for d in i.paths() {
        shape = shape.stroke_svg(d, 1.4, LineCap::Round, color);
    }
    canvas(shape.view_box(16.0, 16.0)).size(size, size)
}
