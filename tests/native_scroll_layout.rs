//! Synthetic layout regression for a scroll view inside GPUI's block wrapper.
use taffy::prelude::*;
use taffy::{geometry::Point, style::Overflow};
fn layout(bounded: bool) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let rows: Vec<_> = (0..25)
        .map(|_| {
            tree.new_leaf(Style {
                size: Size {
                    width: Dimension::from_length(300.),
                    height: Dimension::from_length(150.),
                },
                flex_shrink: 0.,
                ..Default::default()
            })
            .unwrap()
        })
        .collect();
    let scroll = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: Dimension::from_percent(1.),
                    height: if bounded {
                        Dimension::from_percent(1.)
                    } else {
                        Dimension::AUTO
                    },
                },
                flex_grow: 1.,
                min_size: Size {
                    width: Dimension::AUTO,
                    height: Dimension::from_length(0.),
                },
                overflow: Point {
                    x: Overflow::Visible,
                    y: Overflow::Scroll,
                },
                ..Default::default()
            },
            &rows,
        )
        .unwrap();
    let wrapper = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size {
                    width: Dimension::from_length(360.),
                    height: Dimension::from_length(640.),
                },
                ..Default::default()
            },
            &[scroll],
        )
        .unwrap();
    tree.compute_layout(
        wrapper,
        Size {
            width: AvailableSpace::Definite(360.),
            height: AvailableSpace::Definite(640.),
        },
    )
    .unwrap();
    let result = tree.layout(scroll).unwrap();
    (result.size.height, result.content_size.height)
}
#[test]
fn anime_scroll_viewport_stays_bounded_with_overflowing_rows() {
    let old = layout(false);
    let fixed = layout(true);
    assert!(old.0 > 640., "Unbounded block child expands with its rows");
    assert_eq!(fixed.0, 640.);
    assert!(
        fixed.1 > fixed.0,
        "Content must remain scrollable instead of compressing into the viewport"
    );
}
