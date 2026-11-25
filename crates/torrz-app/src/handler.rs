//! Which app opens magnet links. Declaring the scheme in the bundle (scripts/make-app.sh) makes
//! Torrz one of the apps that can; this makes it the one that does, when another torrent app
//! claimed them first. macOS only: elsewhere the desktop's own settings decide.

/// Whether magnet links open in this app: `None` when that can't be known or changed here (not
/// macOS, or not running from an app bundle, as under `cargo run` and in tests).
pub fn magnet_is_default() -> Option<bool> {
    imp::magnet_is_default()
}

/// Asks the system to open magnet links with this app. macOS asks the user to confirm, so the
/// answer comes later, through `done`, with the reason when it didn't happen. `done` may run on
/// any thread.
pub fn make_magnet_default(done: impl Fn(Result<(), String>) + Send + 'static) -> anyhow::Result<()> {
    imp::make_magnet_default(done)
}

#[cfg(target_os = "macos")]
mod imp {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSError, NSString, NSURL};

    /// This app's bundle, if it runs from one.
    fn bundle() -> Option<(Retained<NSURL>, Retained<NSString>)> {
        let b = NSBundle::mainBundle();
        Some((b.bundleURL(), b.bundleIdentifier()?))
    }

    pub fn magnet_is_default() -> Option<bool> {
        let (_, id) = bundle()?;
        let probe = NSURL::URLWithString(&NSString::from_str("magnet:?xt=urn:btih:0"))?;
        let Some(app) = NSWorkspace::sharedWorkspace().URLForApplicationToOpenURL(&probe) else {
            return Some(false);
        };
        // By identifier: another copy of Torrz (dist/ and /Applications) is still Torrz.
        Some(
            NSBundle::bundleWithURL(&app)
                .and_then(|b| b.bundleIdentifier())
                .is_some_and(|other| other.to_string() == id.to_string()),
        )
    }

    pub fn make_magnet_default(done: impl Fn(Result<(), String>) + Send + 'static) -> anyhow::Result<()> {
        let Some((url, _)) = bundle() else {
            anyhow::bail!(
                "Torrz isn't running from its app bundle, so macOS can't send it links (build it with scripts/make-app.sh)"
            )
        };
        let block = RcBlock::new(move |err: *mut NSError| {
            // SAFETY: AppKit passes either null or a valid NSError that lives for the call.
            let err = unsafe { err.as_ref() };
            done(match err {
                None => Ok(()),
                Some(e) => Err(e.localizedDescription().to_string()),
            });
        });
        NSWorkspace::sharedWorkspace()
            .setDefaultApplicationAtURL_toOpenURLsWithScheme_completionHandler(
                &url,
                &NSString::from_str("magnet"),
                Some(&block),
            );
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn magnet_is_default() -> Option<bool> {
        None
    }

    pub fn make_magnet_default(
        _: impl Fn(Result<(), String>) + Send + 'static,
    ) -> anyhow::Result<()> {
        anyhow::bail!("choose the app for magnet links in your desktop's settings")
    }
}
