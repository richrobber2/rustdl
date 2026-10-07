//! Public bundled home artwork; never resolves downloaded-media paths.
use crate::settings::Settings;
use gpui_kit::*;
use std::sync::{Arc, OnceLock};

#[derive(Default)]
pub struct State {
    previous: Option<(bool, bool)>,
    transition: Option<(bool, std::time::Instant, u64)>,
    generation: u64,
}

pub fn background(
    state: &mut State,
    settings: Option<&Settings>,
    dark: bool,
    viewport: Size<Pixels>,
    scroll_offset: Point<Pixels>,
    scroll_max: Point<Pixels>,
) -> impl IntoElement {
    static SPACE: OnceLock<Arc<Image>> = OnceLock::new();
    static CITY: OnceLock<Arc<Image>> = OnceLock::new();
    static LIGHT_CITY: OnceLock<Arc<Image>> = OnceLock::new();
    let city = settings.is_some_and(|settings| settings.background_theme == "rainy-city");
    let image = if city && !dark {
        LIGHT_CITY.get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Webp,
                include_bytes!("../../assets/images/aniwaves-rainy-city-light.webp").to_vec(),
            ))
        })
    } else if city {
        CITY.get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Webp,
                include_bytes!("../../assets/images/aniwaves-rainy-city.webp").to_vec(),
            ))
        })
    } else {
        SPACE.get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../assets/dark-space-v1.png").to_vec(),
            ))
        })
    };
    let space_effect = !city && settings.is_some_and(|settings| settings.space_effect_enabled);
    let reduce_motion = settings.is_none_or(|settings| settings.reduce_motion);
    if let Some((old_city, old_dark)) = state.previous {
        if old_city && city && old_dark != dark && !reduce_motion {
            state.generation = state.generation.wrapping_add(1);
            state.transition = Some((dark, std::time::Instant::now(), state.generation));
        }
    }
    if !city || reduce_motion {
        state.transition = None;
    }
    state.previous = Some((city, dark));
    if state
        .transition
        .is_some_and(|(_, started, _)| started.elapsed().as_secs_f32() >= 4.3)
    {
        state.transition = None;
    }
    let width = f32::from(viewport.width);
    let height = f32::from(viewport.height);
    let mut stars = div().absolute().size_full();
    // Deterministic public decoration; no media or random per-frame allocations.
    for index in 0..48 {
        let x = ((index * 37 + 11) % 101) as f32 / 100.0;
        let y = ((index * 61 + 7) % 103) as f32 / 102.0;
        stars = stars.child(
            div()
                .absolute()
                .left(px(x * width))
                .top(px(y * height))
                .w(px(if index % 3 == 0 { 2.0 } else { 1.0 }))
                .h(px(1.5))
                .rounded_full()
                .bg(if dark {
                    rgba(0xdceaff78)
                } else {
                    rgba(0x2950a318)
                }),
        );
    }
    let stars = if space_effect && !reduce_motion {
        stars
            .with_animation(
                "home-space-drift",
                Animation::new(std::time::Duration::from_secs(95))
                    .repeat()
                    .with_max_fps(20.0),
                move |view, phase| {
                    view.left(px(phase * width * 0.07))
                        .top(px(phase * height * 0.06))
                },
            )
            .into_any_element()
    } else {
        stars.into_any_element()
    };
    let max = f32::from(scroll_max.y);
    let progress = if max.is_finite() && max > 0.0 {
        (-f32::from(scroll_offset.y) / max).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let margin = if city && !reduce_motion {
        height * 0.04
    } else {
        0.0
    };
    let offset = if city && !reduce_motion {
        (0.5 - progress) * height * 0.08
    } else {
        0.0
    };
    let mut backdrop = div().absolute().size_full().overflow_hidden().child(
        img(image.clone())
            .absolute()
            .left(px(0.))
            .top(px(-margin + offset))
            .w(viewport.width)
            .h(px(height + margin * 2.))
            .object_fit(ObjectFit::Cover),
    );
    if let Some((target_dark, _, generation)) = state.transition {
        static FRAMES: OnceLock<Vec<Arc<Image>>> = OnceLock::new();
        let bytes: [&[u8]; 12] = [
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
        let frames = FRAMES
            .get_or_init(|| {
                bytes
                    .iter()
                    .map(|bytes| Arc::new(Image::from_bytes(ImageFormat::Webp, bytes.to_vec())))
                    .collect()
            })
            .clone();
        let old = if target_dark {
            LIGHT_CITY.get_or_init(|| {
                Arc::new(Image::from_bytes(
                    ImageFormat::Webp,
                    include_bytes!("../../assets/images/aniwaves-rainy-city-light.webp").to_vec(),
                ))
            })
        } else {
            CITY.get_or_init(|| {
                Arc::new(Image::from_bytes(
                    ImageFormat::Webp,
                    include_bytes!("../../assets/images/aniwaves-rainy-city.webp").to_vec(),
                ))
            })
        }
        .clone();
        backdrop = backdrop.child(div().absolute().size_full().with_animation(
            ("city-sunrise", generation),
            Animation::new(std::time::Duration::from_millis(4300)).with_max_fps(20.0),
            move |view, phase| {
                let seconds = phase * 4.3;
                let step = ((seconds / 0.28).floor() as usize).min(11);
                let index = if target_dark { 11 - step } else { step };
                let previous = if step == 0 {
                    old.clone()
                } else {
                    frames[if target_dark { 12 - step } else { step - 1 }].clone()
                };
                let alpha = ((seconds - step as f32 * 0.28) / 0.4).clamp(0.0, 1.0);
                let fade = if seconds > 3.6 {
                    ((4.3 - seconds) / 0.7).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                view.opacity(fade)
                    .child(img(previous).size_full().object_fit(ObjectFit::Cover))
                    .child(
                        div().absolute().size_full().opacity(alpha).child(
                            img(frames[index].clone())
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                    )
            },
        ));
    }
    backdrop
        .child(div().absolute().size_full().bg(if dark {
            rgba(0x101018cc)
        } else {
            rgba(0xffffffdd)
        }))
        .children(space_effect.then_some(stars))
}
