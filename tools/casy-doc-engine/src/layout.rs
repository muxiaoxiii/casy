use crate::{LayoutBlock, Region};
use oar_ocr::{
    domain::{structure::LayoutElementType, tasks::layout_detection::LayoutDetectionElement},
    processors::{
        layout_sorting::{sort_layout_enhanced, SortableElement},
        sort_by_xycut, BoundingBox, SortDirection,
    },
};

/// PaddleX reading order groups OCR lines into predicted blocks before sorting columns.
pub fn order_regions(
    regions: &mut Vec<Region>,
    elements: &[LayoutDetectionElement],
    width: f32,
    height: f32,
) {
    let first_y = elements
        .iter()
        .map(|e| e.bbox.y_min())
        .fold(f32::INFINITY, f32::min);
    let mut blocks: Vec<SortableElement> = elements
        .iter()
        .map(|element| SortableElement {
            bbox: element.bbox.clone(),
            element_type: {
                let kind = LayoutElementType::from_label(&element.element_type);
                // A title label in the middle of a page must not leapfrog preceding text.
                if kind == LayoutElementType::DocTitle
                    && element.bbox.y_min() > first_y + height * 0.05
                {
                    LayoutElementType::ParagraphTitle
                } else {
                    kind
                }
            },
            num_lines: None,
        })
        .collect();
    let mut assignments = Vec::new();
    for region in regions.iter() {
        let bbox = BoundingBox::from_coords(
            region.bbox[0],
            region.bbox[1],
            region.bbox[2],
            region.bbox[3],
        );
        let matched = elements
            .iter()
            .enumerate()
            .filter(|(_, block)| bbox.ioa(&block.bbox) > 0.6)
            .min_by(|(_, a), (_, b)| a.bbox.area().total_cmp(&b.bbox.area()))
            .map(|(i, _)| i);
        let index = matched.unwrap_or_else(|| {
            let index = blocks.len();
            blocks.push(SortableElement {
                bbox,
                element_type: LayoutElementType::Text,
                num_lines: Some(1),
            });
            index
        });
        assignments.push(index);
    }
    for (i, block) in blocks.iter_mut().enumerate() {
        block.num_lines = Some(
            assignments
                .iter()
                .filter(|&&index| index == i)
                .count()
                .max(1) as u32,
        );
    }
    let mut order = sort_layout_enhanced(&blocks, width, height);
    let main: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&index| {
            !matches!(
                blocks[index].element_type,
                LayoutElementType::Header
                    | LayoutElementType::Footer
                    | LayoutElementType::DocTitle
                    | LayoutElementType::Footnote
            )
        })
        .collect();
    let mut spans: Vec<(f32, f32)> = main
        .iter()
        .map(|&index| (blocks[index].bbox.x_min(), blocks[index].bbox.x_max()))
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut right = f32::NEG_INFINITY;
    let mut has_gutter = false;
    for (start, end) in spans {
        if right.is_finite() && start - right > width * 0.02 {
            has_gutter = true;
        }
        right = right.max(end);
    }
    // The enhanced helper chooses Y-first for multiple columns, interleaving aligned
    // paragraphs. A clear full-height gutter requires its existing X-first XY-cut.
    if has_gutter {
        let boxes: Vec<_> = main
            .iter()
            .map(|&index| blocks[index].bbox.clone())
            .collect();
        let sorted = sort_by_xycut(&boxes, SortDirection::Horizontal, 1);
        let mut sorted = sorted.into_iter().map(|index| main[index]);
        for index in &mut order {
            if main.contains(index) {
                *index = sorted.next().unwrap();
            }
        }
    }
    let original = std::mem::take(regions);
    for index in order {
        // The OCR predictor already orders lines inside a block, including vertical scripts.
        for (region, &block) in original.iter().zip(&assignments) {
            if block == index {
                regions.push(region.clone());
            }
        }
    }
    if regions.len() != original.len() {
        *regions = original;
    }
}

/// Preserve the detector's semantic blocks after OCR lines have been put in reading order.
pub fn describe_blocks(
    regions: &[Region],
    elements: &[LayoutDetectionElement],
) -> Vec<LayoutBlock> {
    let assignments = regions.iter().map(|region| {
        let bbox = BoundingBox::from_coords(
            region.bbox[0], region.bbox[1], region.bbox[2], region.bbox[3],
        );
        elements.iter().enumerate()
            .filter(|(_, element)| bbox.ioa(&element.bbox) > 0.6)
            .min_by(|(_, a), (_, b)| a.bbox.area().total_cmp(&b.bbox.area()))
            .map(|(index, _)| index)
    }).collect::<Vec<_>>();
    let mut blocks: Vec<_> = elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let region_indices = assignments
                .iter()
                .enumerate()
                .filter_map(|(region_index, assigned)| (*assigned == Some(index)).then_some(region_index))
                .collect::<Vec<_>>();
            LayoutBlock {
                id: format!("block-{index}"),
                kind: element.element_type.clone(),
                bbox: [
                    element.bbox.x_min(), element.bbox.y_min(),
                    element.bbox.x_max(), element.bbox.y_max(),
                ],
                confidence: element.score,
                reading_order: region_indices.first().copied().unwrap_or(usize::MAX) as u32,
                region_indices,
            }
        })
        .collect();
    blocks.sort_by_key(|block| block.reading_order);
    for (order, block) in blocks.iter_mut().enumerate() {
        block.reading_order = order as u32;
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn column_order_keeps_all_lines_and_their_coordinates() {
        let block = |x1, y1, x2, y2| LayoutDetectionElement {
            bbox: BoundingBox::from_coords(x1, y1, x2, y2),
            element_type: "text".into(),
            score: 0.99,
        };
        let blocks = vec![block(10., 40., 190., 300.), block(220., 40., 390., 300.)];
        let line = |text: &str, x, y| Region {
            text: text.into(),
            bbox: [x, y, x + 100., y + 20.],
            confidence: Some(0.99),
        };
        let mut regions = vec![
            line("left one", 10., 50.),
            line("right one", 220., 50.),
            line("left two", 10., 90.),
            line("right two", 220., 90.),
        ];
        order_regions(&mut regions, &blocks, 400., 600.);
        assert_eq!(
            regions.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            vec!["left one", "left two", "right one", "right two"]
        );
        assert_eq!(regions[1].bbox, [10., 90., 110., 110.]);
        let described = describe_blocks(&regions, &blocks);
        assert_eq!(described.len(), 2);
        assert_eq!(described[0].reading_order, 0);
        assert_eq!(described[0].region_indices, vec![0, 1]);
        assert_eq!(described[1].region_indices, vec![2, 3]);
    }

    #[test]
    fn an_inset_vertical_title_does_not_precede_the_page_continuation() {
        let blocks = vec![
            LayoutDetectionElement {
                bbox: BoundingBox::from_coords(10., 40., 150., 60.),
                element_type: "text".into(),
                score: 0.9,
            },
            LayoutDetectionElement {
                bbox: BoundingBox::from_coords(280., 180., 300., 350.),
                element_type: "doc_title".into(),
                score: 0.6,
            },
        ];
        let mut regions = vec![
            Region {
                text: "continuation".into(),
                bbox: [10., 40., 150., 60.],
                confidence: None,
            },
            Region {
                text: "縦書き".into(),
                bbox: [280., 180., 300., 350.],
                confidence: None,
            },
        ];
        order_regions(&mut regions, &blocks, 400., 600.);
        assert_eq!(regions[0].text, "continuation");
    }

    #[test]
    fn aligned_paragraphs_read_down_each_column() {
        let mut blocks = Vec::new();
        let mut regions = Vec::new();
        for row in 0..4 {
            for (column, x) in [("left", 10.), ("right", 220.)] {
                let y = 40. + row as f32 * 80.;
                blocks.push(LayoutDetectionElement {
                    bbox: BoundingBox::from_coords(x, y, x + 170., y + 60.),
                    element_type: "text".into(),
                    score: 0.99,
                });
                for line in 0..3 {
                    regions.push(Region {
                        text: format!("{column} {row} {line}"),
                        bbox: [
                            x,
                            y + line as f32 * 20.,
                            x + 160.,
                            y + line as f32 * 20. + 16.,
                        ],
                        confidence: None,
                    });
                }
            }
        }
        order_regions(&mut regions, &blocks, 400., 600.);
        assert!(regions[..12].iter().all(|r| r.text.starts_with("left")));
        assert!(regions[12..].iter().all(|r| r.text.starts_with("right")));
    }
}
