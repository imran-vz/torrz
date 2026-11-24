//! The app's icon while it runs: the light or the dark Classic arrow, by the user's choice or,
//! on `auto`, by the system's appearance. (The bundle carries the same pair for when it isn't
//! running; macOS switches that one itself.)

use std::sync::atomic::{AtomicU8, Ordering};

use crate::state::IconStyle;

static LIGHT: &[u8] = include_bytes!("../../../assets/icon-light.png");
static DARK: &[u8] = include_bytes!("../../../assets/icon-dark.png");

/// 0: nothing set yet, 1: light, 2: dark.
static SHOWN: AtomicU8 = AtomicU8::new(0);

/// Shows the icon `style` asks for, if it isn't the one showing. Cheap enough for every view
/// pass, which is how a change of the system's appearance reaches it.
pub fn sync(style: IconStyle, system_dark: bool) {
    let dark = match style {
        IconStyle::Auto => system_dark,
        IconStyle::Light => false,
        IconStyle::Dark => true,
    };
    let want = if dark { 2 } else { 1 };
    if SHOWN.swap(want, Ordering::Relaxed) != want {
        // Headless runs set nothing; a failure here only leaves the bundle's own icon showing.
        let _ = rune::set_app_icon(if dark { DARK } else { LIGHT });
    }
}
