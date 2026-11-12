//! The providers section of the settings panel: the list of profiles, the press that selects one,
//! and the two presses that add one and make one active.
//!
//! Nothing here writes anything: a row's press calls the shell, which asks the app.

use rune::{El, col, row, text};
use rune_kit::{Button, ButtonVariant, Kit};

use crate::files::{note, section_label};

use super::live::Live;
use super::settings::Panel;
use super::settings_snapshot::ProviderView;
use super::state::Shell;

/// The panel's providers section: the list, and the presses that change it.
pub(super) fn providers<L: Live>(panel: &Panel, kit: &Kit) -> El {
    let mut body = col().key("settings-providers").w_full().gap(6.0).child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(section_label(kit, "Providers", "settings-providers-label"))
            .child(
                Button::new("Add provider")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(|shell: &mut Shell<L>| {
                        shell.add_profile();
                    })
                    .build(kit)
                    .key("settings-provider-add"),
            )
            .child(
                Button::new("Use this provider")
                    .variant(ButtonVariant::Outline)
                    .on_press(|shell: &mut Shell<L>| {
                        shell.activate_profile();
                    })
                    .build(kit)
                    .key("settings-provider-activate"),
            ),
    );
    if panel.profiles().is_empty() {
        body = body.child(note(
            kit,
            "No provider yet: add one, and the app has something to ask.",
        ));
    }
    for (index, profile) in panel.profiles().iter().enumerate() {
        body = body.child(row_for::<L>(profile, index, kit));
    }
    body
}

/// One profile: its name as the press that selects it, and what it points at.
fn row_for<L: Live>(profile: &ProviderView, index: usize, kit: &Kit) -> El {
    let label = if profile.active {
        format!("{} (active)", profile.name)
    } else {
        profile.name.clone()
    };
    row()
        .key(format!("settings-profile-row-{index}"))
        .w_full()
        .gap(10.0)
        .center_y()
        .child(
            Button::new(label)
                .variant(if profile.active {
                    ButtonVariant::Submit
                } else {
                    ButtonVariant::Outline
                })
                .on_press(move |shell: &mut Shell<L>| {
                    shell.select_profile(index);
                })
                .build(kit)
                .key(format!("settings-profile-{index}")),
        )
        .child(
            text(profile.model.clone())
                .key(format!("settings-profile-model-{index}"))
                .font(kit.ty.small_medium)
                .color(kit.c.text_muted)
                .grow(1.0)
                .truncate(),
        )
        .child(
            text(profile.key.words())
                .key(format!("settings-profile-key-{index}"))
                .font(kit.ty.small)
                .color(kit.c.text_muted),
        )
}
