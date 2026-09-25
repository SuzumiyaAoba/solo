use solo::design_tokens::{ColorScheme, Tone, contrast};

#[test]
fn text_and_semantic_labels_have_readable_contrast_in_both_themes() {
    for scheme in [ColorScheme::Light, ColorScheme::Dark] {
        let p = scheme.palette();
        for foreground in [p.text, p.secondary, p.muted] {
            for background in [p.canvas, p.surface, p.elevated] {
                assert!(
                    contrast(foreground, background) >= 4.5,
                    "{scheme:?} #{foreground:06x} on #{background:06x}"
                );
            }
        }
        for tone in [
            Tone::Neutral,
            Tone::Accent,
            Tone::Success,
            Tone::Warning,
            Tone::Danger,
        ] {
            let (foreground, background) = p.tone(tone);
            assert!(
                contrast(foreground, background) >= 4.5,
                "{scheme:?} {tone:?}"
            );
        }
        for background in [p.accent, p.accent_hover, p.accent_pressed, p.danger_solid] {
            assert!(
                contrast(p.on_accent, background) >= 4.5,
                "{scheme:?} button #{background:06x}"
            );
        }
    }
}

#[test]
fn focus_indicators_and_control_outlines_remain_distinguishable() {
    for scheme in [ColorScheme::Light, ColorScheme::Dark] {
        let p = scheme.palette();
        for background in [p.canvas, p.surface, p.elevated] {
            assert!(contrast(p.focus, background) >= 3.0, "{scheme:?} focus");
            assert!(
                contrast(p.control_border, background) >= 3.0,
                "{scheme:?} control"
            );
        }
    }
}

#[test]
fn contrast_reference_values() {
    assert!((contrast(0, 0xffffff) - 21.).abs() < 0.001);
    assert!((contrast(0x777777, 0x777777) - 1.).abs() < 0.001);
}
