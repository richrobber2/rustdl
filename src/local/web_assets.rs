//! Embedded UI assets, decoration, and immutable asset responses.

use super::super::local;
use std::error::Error;
use std::sync::OnceLock;
use tiny_http::{Request, Response, StatusCode};

pub(in super::super) fn respond_playback_script(request: Request) -> Result<(), Box<dyn Error>> {
    local::web_assets::respond_immutable_asset(
        request,
        PLAYBACK_SCRIPT,
        "application/javascript; charset=utf-8",
    )
}

pub(in super::super) fn respond_view_transition_script(
    request: Request,
) -> Result<(), Box<dyn Error>> {
    local::web_assets::respond_immutable_asset(
        request,
        VIEW_TRANSITION_SCRIPT,
        "application/javascript; charset=utf-8",
    )
}

pub(in super::super) fn respond_immutable_asset(
    request: Request,
    content: &'static str,
    content_type: &'static str,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(content)
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", content_type))
        .with_header(local::html::header(
            "Cache-Control",
            "private, max-age=31536000, immutable",
        ))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) fn respond_immutable_binary_asset(
    request: Request,
    content: &'static [u8],
    content_type: &'static str,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_data(content.to_vec())
        .with_status_code(StatusCode(200))
        .with_header(local::html::header("Content-Type", content_type))
        .with_header(local::html::header(
            "Cache-Control",
            "private, max-age=31536000, immutable",
        ))
        .with_header(local::html::header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

pub(in super::super) const INDEX_CSS: &str = include_str!("../../assets/css/index.css");

pub(in super::super) const INDEX_HTML: &str = include_str!("../../assets/html/index.html");

pub(in super::super) const PLAYER_CSS: &str = include_str!("../../assets/css/player.css");

pub(in super::super) const DARK_SPACE_BACKGROUND: &[u8] =
    include_bytes!("../../assets/dark-space-v1.png");

pub(in super::super) const DARK_SPACE_BACKGROUND_PLACEHOLDER: &str =
    "__RUSTDL_DARK_SPACE_BACKGROUND__";

pub(in super::super) const APPEARANCE_CSS: &str = include_str!("../../assets/css/appearance.css");

pub(in super::super) const APPEARANCE_BOOT_SCRIPT: &str =
    include_str!("../../assets/js/appearance-boot.js");

pub(in super::super) const APPEARANCE_SCRIPT: &str = include_str!("../../assets/js/appearance.js");

pub(in super::super) fn hashed_asset_path(name: &str, extension: &str, content: &str) -> String {
    let digest = blake3::hash(content.as_bytes()).to_hex();
    format!("/__app/{name}.{}.{}", &digest[..16], extension)
}

pub(in super::super) fn hashed_binary_asset_path(
    name: &str,
    extension: &str,
    content: &[u8],
) -> String {
    let digest = blake3::hash(content).to_hex();
    format!("/__app/{name}.{}.{}", &digest[..16], extension)
}

pub(in super::super) fn dark_space_background_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_binary_asset_path("dark-space", "png", DARK_SPACE_BACKGROUND)
    })
}

pub(in super::super) const RAINY_CITY_BACKGROUND: &[u8] =
    include_bytes!("../../assets/images/aniwaves-rainy-city.webp");

pub(in super::super) fn rainy_city_background_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_binary_asset_path("rainy-city", "webp", RAINY_CITY_BACKGROUND)
    })
}

pub(in super::super) const RAINY_CITY_LIGHT_BACKGROUND: &[u8] =
    include_bytes!("../../assets/images/aniwaves-rainy-city-light.webp");

pub(in super::super) fn rainy_city_light_background_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_binary_asset_path(
            "rainy-city-light",
            "webp",
            RAINY_CITY_LIGHT_BACKGROUND,
        )
    })
}

pub(in super::super) const SUNRISE_FRAMES: [&[u8]; 12] = [
    include_bytes!("../../assets/images/sunrise-v2/frame-01.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-02.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-03.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-04.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-05.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-06.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-07.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-08.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-09.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-10.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-11.webp"),
    include_bytes!("../../assets/images/sunrise-v2/frame-12.webp"),
];

pub(in super::super) fn sunrise_paths() -> &'static [String; 12] {
    static PATHS: OnceLock<[String; 12]> = OnceLock::new();
    PATHS.get_or_init(|| {
        std::array::from_fn(|i| {
            local::web_assets::hashed_binary_asset_path(
                &format!("sunrise-{}", i + 1),
                "webp",
                SUNRISE_FRAMES[i],
            )
        })
    })
}

pub(in super::super) fn appearance_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let mut css = APPEARANCE_CSS
            .replace(
                DARK_SPACE_BACKGROUND_PLACEHOLDER,
                local::web_assets::dark_space_background_path(),
            )
            .replace(
                "__RUSTDL_RAINY_CITY_BACKGROUND__",
                local::web_assets::rainy_city_background_path(),
            )
            .replace(
                "__RUSTDL_RAINY_CITY_LIGHT_BACKGROUND__",
                local::web_assets::rainy_city_light_background_path(),
            );
        for (i, path) in sunrise_paths().iter().enumerate() {
            css = css.replace(&format!("__RUSTDL_SUNRISE_{}__", i + 1), path);
        }
        css
    })
}

pub(in super::super) fn playback_script_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| local::web_assets::hashed_asset_path("playback", "js", PLAYBACK_SCRIPT))
}

pub(in super::super) fn view_transition_script_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_asset_path("view-transitions", "js", VIEW_TRANSITION_SCRIPT)
    })
}

pub(in super::super) fn appearance_css_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_asset_path(
            "appearance",
            "css",
            local::web_assets::appearance_css(),
        )
    })
}

pub(in super::super) fn appearance_script_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| local::web_assets::hashed_asset_path("appearance", "js", APPEARANCE_SCRIPT))
}

pub(crate) fn decorate_app_html(mut html: String) -> String {
    if !html.contains("<!doctype html") || html.contains("data-rustdl-appearance") {
        return html;
    }
    let head = format!(
        r#"<link data-rustdl-appearance rel="stylesheet" href="{}"><script>{APPEARANCE_BOOT_SCRIPT}</script></head>"#,
        local::web_assets::appearance_css_path()
    );
    html = html.replacen("</head>", &head, 1);
    let tail = format!(
        r#"<script data-rustdl-appearance src="{}" defer></script></body>"#,
        local::web_assets::appearance_script_path()
    );
    html.replacen("</body>", &tail, 1)
}

pub(in super::super) fn index_css() -> &'static str {
    INDEX_CSS
}

pub(in super::super) fn index_css_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        local::web_assets::hashed_asset_path("index", "css", local::web_assets::index_css())
    })
}

pub(in super::super) fn player_css_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| local::web_assets::hashed_asset_path("player", "css", PLAYER_CSS))
}

pub(in super::super) fn index_html_template() -> &'static str {
    static HTML: OnceLock<String> = OnceLock::new();
    HTML.get_or_init(|| {
        INDEX_HTML.replacen(
            "<!--INDEX_STYLESHEET-->",
            &format!(
                r#"<link rel="stylesheet" href="{}">"#,
                local::web_assets::index_css_path()
            ),
            1,
        )
    })
}

pub(in super::super) fn playback_script_tag() -> String {
    format!(
        r#"<script src="{}" defer></script>"#,
        local::web_assets::playback_script_path()
    )
}

pub(in super::super) fn view_transition_script_tag() -> String {
    format!(
        r#"<script src="{}" defer></script>"#,
        local::web_assets::view_transition_script_path()
    )
}

pub(in super::super) const VIEW_TRANSITION_SCRIPT: &str =
    include_str!("../../assets/js/view-transitions.js");

pub(in super::super) const PLAYBACK_SCRIPT: &str = include_str!("../../assets/js/playback.js");

pub(in super::super) fn view_transition_name(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
        .unwrap_or(filename);
    let safe = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("video-{safe}")
}
