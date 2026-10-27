//! Home's Inter typography; editor document fonts remain in their own engines.
use egui::{FontData, FontDefinitions, FontFamily, FontId};
#[cfg(target_os = "macos")]
use std::sync::{Arc, OnceLock};

pub fn ready(ctx: &egui::Context) -> bool {
    ctx.fonts(|fonts| {
        ["home-medium", "home-semibold"].into_iter().all(|name| {
            fonts
                .definitions()
                .families
                .contains_key(&FontFamily::Name(name.into()))
        })
    })
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        (
            "Inter",
            include_bytes!("../../reader/assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "Inter Medium",
            include_bytes!("../../reader/assets/fonts/Inter-Medium.ttf").as_slice(),
        ),
        (
            "Inter Semibold",
            include_bytes!("../../reader/assets/fonts/Inter-SemiBold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(data).into());
    }
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, name) in [
        (FontFamily::Proportional, "Inter"),
        (FontFamily::Name("home-medium".into()), "Inter Medium"),
        (FontFamily::Name("home-semibold".into()), "Inter Semibold"),
    ] {
        let mut names = vec![name.to_string()];
        names.extend(fallback.clone());
        fonts.families.insert(family, names);
    }
    ctx.set_fonts(fonts);
}

/// Add installed macOS script faces after each workspace installs its own UI fonts.
/// They are read locally and never copied into the app bundle.
pub fn add_indic_system_fallbacks(ctx: &egui::Context) {
    #[cfg(target_os = "macos")]
    {
        const SENTINEL: &str = "polymath-devanagari";
        if ctx.fonts(|f| f.definitions().font_data.contains_key(SENTINEL)) {
            return;
        }
        static FONTS: OnceLock<Vec<(&'static str, Arc<FontData>)>> = OnceLock::new();
        let system_fonts = FONTS.get_or_init(|| {
            [
                (
                    SENTINEL,
                    "/System/Library/Fonts/Supplemental/DevanagariMT.ttc",
                ),
                (
                    "polymath-bangla",
                    "/System/Library/Fonts/KohinoorBangla.ttc",
                ),
                (
                    "polymath-gujarati",
                    "/System/Library/Fonts/KohinoorGujarati.ttc",
                ),
                (
                    "polymath-gurmukhi",
                    "/System/Library/Fonts/Supplemental/Gurmukhi.ttf",
                ),
                (
                    "polymath-tamil",
                    "/System/Library/Fonts/Supplemental/Tamil MN.ttc",
                ),
                (
                    "polymath-telugu",
                    "/System/Library/Fonts/KohinoorTelugu.ttc",
                ),
                (
                    "polymath-kannada",
                    "/System/Library/Fonts/NotoSansKannada.ttc",
                ),
                (
                    "polymath-malayalam",
                    "/System/Library/Fonts/Supplemental/Malayalam MN.ttc",
                ),
                ("polymath-urdu", "/System/Library/Fonts/SFArabic.ttf"),
            ]
            .into_iter()
            .filter_map(|(name, path)| {
                let size = std::fs::metadata(path).ok()?.len();
                if size > 16 << 20 {
                    return None;
                }
                let bytes = std::fs::read(path).ok()?;
                Some((name, Arc::new(FontData::from_owned(bytes))))
            })
            .collect()
        });
        if system_fonts.is_empty() {
            return;
        }
        // set_fonts from an editor becomes active on the next pass. add_font is queued
        // after it, so this never replaces an editor's named families with stale ones.
        let families: Vec<_> = ctx.fonts(|f| f.definitions().families.keys().cloned().collect());
        for (name, data) in system_fonts {
            ctx.add_font(egui::epaint::text::FontInsert::new(
                name,
                (**data).clone(),
                families
                    .iter()
                    .cloned()
                    .map(|family| egui::epaint::text::InsertFontFamily {
                        family,
                        priority: egui::epaint::text::FontPriority::Lowest,
                    })
                    .collect(),
            ));
        }
    }
}

pub fn heading(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("home-semibold".into()))
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("home-medium".into()))
}
