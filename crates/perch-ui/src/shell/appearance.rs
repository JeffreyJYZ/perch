//! What the frame draws with: the theme and the type size, read where the kit is built.
//!
//! Three answers, in order. The settings panel's draft comes first, so a theme picked there is
//! drawn before Save; then the window's own pick from `/theme`, which is the anchors a name
//! resolved to when it was chosen; then the app's appearance, which it reads from `settings.toml`
//! on every call, so a save reaches a window that is already open. A window with none of those
//! draws the built-in kit its saved word names.

use crate::theme::{Appearance, Library};

use super::live::Live;
use super::state::Shell;

impl<L: Live> Shell<L> {
    /// The appearance the frame uses.
    pub fn appearance(&self) -> Appearance {
        if let Some(panel) = self.settings.as_ref() {
            let draft = panel.draft();
            return Appearance::resolve(&Library::discover(), &draft.theme, draft.font_points());
        }
        if let Some(picked) = self.picked.as_ref() {
            return picked.clone();
        }
        if let Some(appearance) = self.live.as_ref().and_then(Live::appearance) {
            return appearance;
        }
        if self.dark() {
            Appearance::dark()
        } else {
            Appearance::light()
        }
    }

    /// Draws `appearance` from the next frame: what `/theme` resolved a name to, so a file theme
    /// reaches the window with no app behind it too.
    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.dark = appearance.anchors.dark();
        self.picked = Some(appearance);
    }

    /// Re-reads the app's appearance: after a save, and when the panel opens, so the frame draws
    /// what the file now says rather than what it said when the window was built.
    pub fn refresh_appearance(&mut self) {
        if let Some(appearance) = self.live.as_ref().and_then(Live::appearance) {
            self.dark = appearance.anchors.dark();
            self.picked = Some(appearance);
        }
    }
}
