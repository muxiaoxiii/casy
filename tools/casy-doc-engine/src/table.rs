//! Recover ruled tables from visible borders. No vocabulary or numeric values are inferred.
use crate::{Region, source_map};
use image::RgbImage;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub bbox: [f32; 4],
    pub regions: Vec<usize>,
    grid: [f32; 4],
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub bbox: [f32; 4],
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<Cell>,
    horizontal_slope: f32,
    vertical_slope: f32,
}
struct Raster<'a> {
    image: &'a RgbImage,
    bounds: [i32; 4],
}
impl Raster<'_> {
    fn dark(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && x < self.image.width() as i32
            && y < self.image.height() as i32
            && self
                .image
                .get_pixel(x as u32, y as u32)
                .0
                .iter()
                // Thin scanned rules can be gray even when the printed text is black.
                // Continuity and rectangular topology, rather than blackness alone,
                // distinguish rules from the much lighter page background.
                .all(|&v| v < 180)
    }
    fn support(
        &self,
        horizontal: bool,
        position: f32,
        slope: f32,
        start: f32,
        end: f32,
    ) -> (f32, usize) {
        self.support_radius(horizontal, position, slope, start, end, 4)
    }
    fn support_radius(
        &self,
        horizontal: bool,
        position: f32,
        slope: f32,
        start: f32,
        end: f32,
        radius: i32,
    ) -> (f32, usize) {
        let mut count = 0;
        let mut longest = 0;
        let mut run = 0;
        let mut gap = 0;
        let start = start.ceil() as i32;
        let end = end.floor() as i32;
        for along in start..end {
            let cross = (position + slope * along as f32).round() as i32;
            let hit = (-radius..=radius).any(|d| {
                if horizontal {
                    self.dark(along, cross + d)
                } else {
                    self.dark(cross + d, along)
                }
            });
            if hit {
                count += 1;
                run += gap + 1;
                gap = 0;
                longest = longest.max(run)
            } else {
                gap += 1;
                if gap > 4 {
                    run = 0;
                    gap = 0
                }
            }
        }
        (count as f32 / (end - start).max(1) as f32, longest)
    }
    fn axis(&self, horizontal: bool, points: &[(f32, f32)]) -> (f32, Vec<f32>) {
        let [x0, y0, x1, y1] = self.bounds;
        let (start, end) = if horizontal { (x0, x1) } else { (y0, y1) };
        let length = (end - start) as f32;
        let size = (self.image.width().max(self.image.height()) + 256) as usize;
        let mut best = (0.0f64, 0.0f32, vec![]);
        for step in -40..=40 {
            let slope = step as f32 / 1000.0;
            let mut profile = vec![0u32; size];
            for &(x, y) in points {
                let p = if horizontal {
                    y - slope * x
                } else {
                    x - slope * y
                };
                let i = (p.round() as i32 + 128) as usize;
                if i < size {
                    profile[i] += 1
                }
            }
            let score = profile
                .iter()
                .map(|&n| ((n as f64 - length as f64 * 0.5).max(0.0)).powi(2))
                .sum();
            if score > best.0 {
                best = (score, slope, profile)
            }
        }
        if best.0 == 0.0 {
            return (0.0, vec![]);
        }
        let mut groups: Vec<Vec<(f32, usize)>> = vec![];
        for i in 4..size - 4 {
            let sum: u32 = best.2[i - 4..=i + 4].iter().sum();
            if (sum as f32) < length * if horizontal { 0.4 } else { 0.09 } {
                continue;
            }
            let pos = i as f32 - 128.0;
            let (_, longest) = self.support(horizontal, pos, best.1, start as f32, end as f32);
            if (longest as f32) < length * if horizontal { 0.43 } else { 0.09 } || longest < 35 {
                continue;
            }
            if groups
                .last()
                .is_none_or(|g| pos - g.last().unwrap().0 > 5.0)
            {
                groups.push(vec![])
            }
            groups.last_mut().unwrap().push((pos, longest));
        }
        (
            best.1,
            groups
                .iter()
                .map(|g| {
                    g.iter().map(|(p, n)| p * (*n as f32)).sum::<f32>()
                        / g.iter().map(|(_, n)| *n as f32).sum::<f32>()
                })
                .collect(),
        )
    }
}
fn root(parent: &mut [usize], mut a: usize) -> usize {
    while parent[a] != a {
        parent[a] = parent[parent[a]];
        a = parent[a]
    }
    a
}
fn union(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    parent[a] = b
}
fn point(u: f32, v: f32, hs: f32, vs: f32) -> (f32, f32) {
    let x = (u + vs * v) / (1.0 - vs * hs);
    (x, v + hs * x)
}
fn bounds(grid: [f32; 4], hs: f32, vs: f32) -> [f32; 4] {
    let points = [
        point(grid[0], grid[1], hs, vs),
        point(grid[2], grid[1], hs, vs),
        point(grid[2], grid[3], hs, vs),
        point(grid[0], grid[3], hs, vs),
    ];
    [
        points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min),
        points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min),
        points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max),
        points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max),
    ]
}
/// Both outer borders and every inferred cell must have a rectangular grid topology.
pub fn detect(image: &RgbImage, bbox: [f32; 4]) -> Option<Table> {
    let raster = Raster {
        image,
        bounds: [
            (bbox[0] as i32 - 12).max(0),
            (bbox[1] as i32 - 12).max(0),
            (bbox[2] as i32 + 12).min(image.width() as i32),
            (bbox[3] as i32 + 12).min(image.height() as i32),
        ],
    };
    let [x0, y0, x1, y1] = raster.bounds;
    if x1 - x0 < 80 || y1 - y0 < 60 {
        return None;
    }
    let mut points = vec![];
    for y in y0..y1 {
        for x in x0..x1 {
            if raster.dark(x, y) {
                points.push((x as f32, y as f32))
            }
        }
    }
    let (hs, mut ys) = raster.axis(true, &points);
    let (vs, mut xs) = raster.axis(false, &points);
    if xs.len() < 3 || ys.len() < 3 || xs.len() > 41 || ys.len() > 201 {
        return None;
    }
    // Layout boxes may also enclose a heading underline above the actual table.
    while ys.len() > 2 {
        let a = point(xs[0], ys[0], hs, vs);
        let b = point(xs[xs.len() - 1], ys[1], hs, vs);
        if raster.support(false, xs[0], vs, a.1 + 6.0, b.1 - 6.0).0 > 0.7
            && raster
                .support(false, xs[xs.len() - 1], vs, a.1 + 6.0, b.1 - 6.0)
                .0
                > 0.7
        {
            break;
        }
        ys.remove(0);
    }
    while ys.len() > 2 {
        let n = ys.len();
        let a = point(xs[0], ys[n - 2], hs, vs);
        let b = point(xs[xs.len() - 1], ys[n - 1], hs, vs);
        if raster.support(false, xs[0], vs, a.1 + 6.0, b.1 - 6.0).0 > 0.7
            && raster
                .support(false, xs[xs.len() - 1], vs, a.1 + 6.0, b.1 - 6.0)
                .0
                > 0.7
        {
            break;
        }
        ys.pop();
    }
    xs.dedup_by(|a, b| (*a - *b).abs() < 12.0);
    ys.dedup_by(|a, b| (*a - *b).abs() < 12.0);
    let rows = ys.len() - 1;
    let columns = xs.len() - 1;
    if rows < 2 || columns < 2 || rows * columns > 2000 {
        return None;
    }
    // Missing external borders mean the structure cannot be confidently reconstructed.
    for y in [ys[0], ys[rows]] {
        let a = point(xs[0], y, hs, vs);
        let b = point(xs[columns], y, hs, vs);
        if raster.support(true, y, hs, a.0 + 6.0, b.0 - 6.0).0 < 0.7 {
            return None;
        }
    }
    let mut parent: Vec<_> = (0..rows * columns).collect();
    for row in 0..rows {
        for col in 0..columns {
            let index = row * columns + col;
            if col + 1 < columns {
                let a = point(xs[col + 1], ys[row], hs, vs);
                let b = point(xs[col + 1], ys[row + 1], hs, vs);
                let (coverage, run) =
                    raster.support_radius(false, xs[col + 1], vs, a.1 + 7.0, b.1 - 7.0, 2);
                let length = (b.1 - a.1 - 14.0).max(1.0);
                if coverage >= 0.75 && run as f32 >= length * 0.6 {
                }
                // continuous visible border
                else if (run as f32) < length * 0.5 {
                    union(&mut parent, index, index + 1)
                } else {
                    return None;
                }
            }
            if row + 1 < rows {
                let a = point(xs[col], ys[row + 1], hs, vs);
                let b = point(xs[col + 1], ys[row + 1], hs, vs);
                let (coverage, run) =
                    raster.support_radius(true, ys[row + 1], hs, a.0 + 7.0, b.0 - 7.0, 2);
                let length = (b.0 - a.0 - 14.0).max(1.0);
                if coverage >= 0.75 && run as f32 >= length * 0.6 {
                } else if (run as f32) < length * 0.5 {
                    union(&mut parent, index, index + columns)
                } else {
                    return None;
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for row in 0..rows {
        for col in 0..columns {
            groups
                .entry(root(&mut parent, row * columns + col))
                .or_default()
                .push((row, col));
        }
    }
    let mut cells = vec![];
    for entries in groups.values() {
        let r = entries.iter().map(|p| p.0).min()?;
        let c = entries.iter().map(|p| p.1).min()?;
        let re = entries.iter().map(|p| p.0).max()? + 1;
        let ce = entries.iter().map(|p| p.1).max()? + 1;
        if entries.len() != (re - r) * (ce - c) {
            return None;
        }
        let grid = [xs[c], ys[r], xs[ce], ys[re]];
        cells.push(Cell {
            row: r,
            column: c,
            row_span: re - r,
            col_span: ce - c,
            bbox: bounds(grid, hs, vs),
            regions: vec![],
            grid,
        });
    }
    cells.sort_by_key(|c| (c.row, c.column));
    Some(Table {
        bbox: bounds([xs[0], ys[0], xs[columns], ys[rows]], hs, vs),
        rows,
        columns,
        cells,
        horizontal_slope: hs,
        vertical_slope: vs,
    })
}
impl Table {
    fn grid_bbox(&self, b: [f32; 4]) -> [f32; 4] {
        // OCR stores an axis-aligned envelope around a slightly tilted text line.
        // Transforming all envelope corners would count the scan skew twice and
        // wrongly push long lines into the previous row. Match its center and
        // conservative inner extent instead; original source coordinates stay intact.
        let x = (b[0] + b[2]) / 2.0;
        let y = (b[1] + b[3]) / 2.0;
        let w = b[2] - b[0];
        let h = b[3] - b[1];
        let width = (w - self.vertical_slope.abs() * h).max(w * 0.5);
        let height = (h - self.horizontal_slope.abs() * w).max(h * 0.5);
        let u = x - self.vertical_slope * y;
        let v = y - self.horizontal_slope * x;
        [
            u - width / 2.0,
            v - height / 2.0,
            u + width / 2.0,
            v + height / 2.0,
        ]
    }
    pub fn crossing_cells(&self, region: &Region) -> Vec<usize> {
        let b = self.grid_bbox(region.bbox);
        let area = (b[2] - b[0]) * (b[3] - b[1]);
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, c)| overlap(b, c.grid) > area * 0.12)
            .map(|(i, _)| i)
            .collect()
    }
    pub fn assign(&mut self, regions: &[Region]) -> HashSet<usize> {
        for c in &mut self.cells {
            c.regions.clear()
        }
        let mut heights: Vec<_> = regions
            .iter()
            .filter(|r| overlap(r.bbox, self.bbox) > 0.0)
            .map(|r| r.bbox[3] - r.bbox[1])
            .collect();
        heights.sort_by(f32::total_cmp);
        let typical = heights.get(heights.len() / 2).copied().unwrap_or(20.0);
        let mut assigned = HashSet::new();
        for (i, r) in regions.iter().enumerate() {
            let b = self.grid_bbox(r.bbox);
            let area = ((b[2] - b[0]) * (b[3] - b[1])).max(1.0);
            // Oversized diagonal marks are retained outside the table, never mixed into a value.
            if b[3] - b[1] > typical * 2.2 {
                continue;
            }
            if let Some(cell) = self.cells.iter_mut().find(|c| {
                let center = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
                center.0 >= c.grid[0]
                    && center.0 <= c.grid[2]
                    && center.1 >= c.grid[1]
                    && center.1 <= c.grid[3]
                    && overlap(
                        b,
                        [
                            c.grid[0] - 5.0,
                            c.grid[1] - 5.0,
                            c.grid[2] + 5.0,
                            c.grid[3] + 5.0,
                        ],
                    ) / area
                        > 0.85
            }) {
                cell.regions.push(i);
                assigned.insert(i);
            }
        }
        for c in &mut self.cells {
            c.regions.sort_by(|&a, &b| {
                regions[a].bbox[1]
                    .total_cmp(&regions[b].bbox[1])
                    .then(regions[a].bbox[0].total_cmp(&regions[b].bbox[0]))
            })
        }
        assigned
    }
    pub fn html(&self, regions: &[Region]) -> String {
        let mut html = String::from("<table>\n");
        for row in 0..self.rows {
            html.push_str("<tr>");
            for cell in self.cells.iter().filter(|c| c.row == row) {
                html.push_str(&format!(
                    "<td rowspan=\"{}\" colspan=\"{}\">",
                    cell.row_span, cell.col_span
                ));
                for (n, &i) in cell.regions.iter().enumerate() {
                    if n > 0 {
                        html.push_str("<br>")
                    }
                    html.push_str(&source_map::tagged_region(i, &regions[i].text))
                }
                html.push_str("</td>");
            }
            html.push_str("</tr>\n");
        }
        html.push_str("</table>");
        html
    }
}
fn overlap(a: [f32; 4], b: [f32; 4]) -> f32 {
    ((a[2].min(b[2]) - a[0].max(b[0])).max(0.0)) * ((a[3].min(b[3]) - a[1].max(b[1])).max(0.0))
}
/// Interleave complete tables with ordinary page content. Preserve all unmatched OCR.
#[cfg(test)]
pub fn markdown(regions: &[Region], tables: &mut [Table], unresolved: usize) -> String {
    markdown_with_visuals(regions, tables, unresolved, &[])
}
pub fn markdown_with_visuals(
    regions: &[Region],
    tables: &mut [Table],
    unresolved: usize,
    visuals: &[crate::visual::Visual],
) -> String {
    markdown_with_layout(regions, tables, unresolved, visuals, &[])
}

pub fn markdown_with_layout(
    regions: &[Region],
    tables: &mut [Table],
    unresolved: usize,
    visuals: &[crate::visual::Visual],
    layout_blocks: &[crate::LayoutBlock],
) -> String {
    if tables.is_empty() && unresolved == 0 && visuals.is_empty() {
        if layout_blocks.is_empty() {
            return regions.iter().map(|region| region.text.as_str()).collect::<Vec<_>>().join("\n");
        }
        return regions.iter().enumerate().map(|(index, region)| {
            semantic_region(index, region, layout_blocks)
        }).collect::<Vec<_>>().join("\n");
    }
    let mut assigned = HashSet::new();
    let mut blocks: Vec<(f32, f32, String)> = vec![];
    let page_extent = regions.iter().map(|r| r.bbox[2]).fold(1.0f32, f32::max);
    let full_width = tables
        .iter()
        .all(|t| t.bbox[2] - t.bbox[0] > page_extent * 0.6);
    for table in tables.iter_mut() {
        let indices = table.assign(regions);
        let anchor = indices.iter().min().copied().unwrap_or(0) as f32;
        assigned.extend(indices);
        blocks.push((
            if full_width { table.bbox[1] } else { anchor },
            table.bbox[0],
            table.html(regions),
        ))
    }
    for visual in visuals {
        let indices: Vec<_> = regions
            .iter()
            .enumerate()
            .filter(|(i, r)| !assigned.contains(i) && visual.contains(r.bbox))
            .map(|(i, _)| i)
            .collect();
        let anchor = indices.first().copied().unwrap_or_else(|| {
            regions
                .iter()
                .position(|r| r.bbox[1] >= visual.bbox[1])
                .unwrap_or(regions.len())
        }) as f32;
        assigned.extend(&indices);
        blocks.push((
            if full_width { visual.bbox[1] } else { anchor },
            visual.bbox[0],
            visual.html(regions, &indices),
        ));
    }
    let mut ambiguous = vec![];
    for (i, r) in regions.iter().enumerate() {
        if assigned.contains(&i) {
            continue;
        }
        if tables.iter().any(|t| {
            overlap(r.bbox, t.bbox) > ((r.bbox[2] - r.bbox[0]) * (r.bbox[3] - r.bbox[1])) * 0.4
        }) {
            ambiguous.push(source_map::tagged_region(i, &r.text))
        } else {
            blocks.push((
                if full_width { r.bbox[1] } else { i as f32 },
                r.bbox[0],
                semantic_region(i, r, layout_blocks),
            ))
        }
    }
    blocks.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut md = blocks
        .into_iter()
        .map(|(_, _, s)| s)
        .collect::<Vec<_>>()
        .join("\n\n");
    if !ambiguous.is_empty() {
        md.push_str("\n\n> 表格附近未可靠归位的识别文字（可能含水印／印章，请对照原页）：\n\n");
        md.push_str(&ambiguous.join("\n\n"))
    }
    if unresolved > 0 {
        md.push_str("\n\n> 表格结构待核对：检测到表格，但未能从可见边框可靠恢复单元格；以上保留逐行识别文字，不代表正确的行列关系。")
    }
    md
}

fn semantic_region(index: usize, region: &Region, blocks: &[crate::LayoutBlock]) -> String {
    let tagged = source_map::tagged_region(index, &region.text);
    let kind = blocks.iter().find(|block| block.region_indices.contains(&index)).map(|block| block.kind.as_str());
    match kind {
        Some("doc_title") => format!("# {tagged}"),
        Some("paragraph_title" | "title") => format!("## {tagged}"),
        Some("list" | "list_item") => format!("- {tagged}"),
        Some("header" | "footer" | "footnote") => format!("<small>{tagged}</small>"),
        _ => tagged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(hs: f32, vs: f32) -> RgbImage {
        let mut image = RgbImage::from_pixel(460, 340, image::Rgb([255, 255, 255]));
        let xs = [40.0, 160.0, 280.0, 400.0];
        let ys = [40.0, 100.0, 160.0, 220.0, 280.0];
        let mut line = |u0: f32, v0: f32, u1: f32, v1: f32| {
            let steps = ((u1 - u0).abs() + (v1 - v0).abs()) as usize;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let (x, y) = point(u0 + (u1 - u0) * t, v0 + (v1 - v0) * t, hs, vs);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        image.put_pixel(
                            (x.round() as i32 + dx) as u32,
                            (y.round() as i32 + dy) as u32,
                            image::Rgb([30, 30, 30]),
                        );
                    }
                }
            }
        };
        for (r, &y) in ys.iter().enumerate() {
            for c in 0..3 {
                if r == 2 && c == 0 {
                    continue;
                }
                line(xs[c], y, xs[c + 1], y)
            }
        }
        for (c, &x) in xs.iter().enumerate() {
            for r in 0..4 {
                if c == 2 && r == 0 {
                    continue;
                }
                line(x, ys[r], x, ys[r + 1])
            }
        }
        image
    }
    #[test]
    fn ruled_cells_preserve_blank_cells_rowspan_colspan_and_scan_skew() {
        for (hs, vs) in [(0.0, 0.0), (0.021, -0.019), (-0.02, 0.015)] {
            let im = synthetic(hs, vs);
            let table = detect(&im, [25.0, 25.0, 420.0, 300.0]).expect("ruled grid");
            assert_eq!((table.rows, table.columns, table.cells.len()), (4, 3, 10));
            assert!(
                table
                    .cells
                    .iter()
                    .any(|c| c.row == 0 && c.column == 1 && c.col_span == 2)
            );
            assert!(
                table
                    .cells
                    .iter()
                    .any(|c| c.row == 1 && c.column == 0 && c.row_span == 2)
            );
            assert_eq!(table.html(&[]).matches("<td ").count(), 10);
        }
    }
    #[test]
    fn text_columns_do_not_become_tables_and_uncertain_layout_is_explicit() {
        let mut image = RgbImage::from_pixel(460, 340, image::Rgb([255, 255, 255]));
        for row in 0..8 {
            for col in 0..16 {
                for y in 0..14 {
                    for x in 0..7 {
                        image.put_pixel(
                            30 + col * 24 + x,
                            30 + row * 34 + y,
                            image::Rgb([0, 0, 0]),
                        );
                    }
                }
            }
        }
        assert!(detect(&image, [20.0, 20.0, 440.0, 320.0]).is_none());
        let r = Region {
            text: "raw <0.001".into(),
            bbox: [20.0, 20.0, 60.0, 40.0],
            confidence: Some(0.9), word_boxes: None,
        };
        assert_eq!(markdown(&[r.clone()], &mut [], 0), r.text);
        assert!(markdown(&[r], &mut [], 1).contains("表格结构待核对"));
    }
    #[test]
    fn content_stays_in_its_cell_and_large_overlapping_marks_are_retained_separately() {
        let mut table = detect(&synthetic(0.0, 0.0), [25.0, 25.0, 420.0, 300.0]).unwrap();
        let r = |text: &str, bbox| Region {
            text: text.into(),
            bbox,
            confidence: Some(0.99), word_boxes: None,
        };
        let regions = vec![
            r("sample B", [285.0, 105.0, 365.0, 125.0]),
            r("<0.001 | &", [170.0, 170.0, 260.0, 190.0]),
            r("watermark", [300.0, 70.0, 430.0, 270.0]),
        ];
        let md = markdown(&regions, std::slice::from_mut(&mut table), 0);
        assert_eq!(
            table
                .cells
                .iter()
                .find(|c| c.row == 2 && c.column == 1)
                .unwrap()
                .regions,
            vec![1]
        );
        assert!(md.contains("&lt;0.001 | &amp;"));
        assert!(md.contains("未可靠归位"));
        assert_eq!(md.matches("watermark").count(), 1);
        let crossing = r("label 4", [260.0, 105.0, 310.0, 125.0]);
        assert_eq!(table.crossing_cells(&crossing).len(), 2);
    }
    #[test]
    fn faint_partial_dividers_are_not_silently_merged() {
        let mut im = synthetic(0.0, 0.0);
        // A gray separator only in the lower rows; surrounding rules stay black.
        for y in 100..290 {
            for x in 276..=284 {
                if im.get_pixel(x, y).0 == [30, 30, 30] {
                    im.put_pixel(x, y, image::Rgb([170, 170, 170]));
                }
            }
        }
        let table = detect(&im, [25.0, 25.0, 420.0, 300.0]).unwrap();
        assert_eq!((table.rows, table.columns, table.cells.len()), (4, 3, 10));
    }
    #[test]
    #[ignore = "private report; set CASY_TABLE_FIXTURE_DIR explicitly"]
    fn private_gds_grid_review() {
        let root = std::path::PathBuf::from(std::env::var("CASY_TABLE_FIXTURE_DIR").unwrap());
        let im = image::open(root.join("gds-page2-full.png"))
            .unwrap()
            .to_rgb8();
        let layout: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("gds-baseline/page-2.layout.json")).unwrap(),
        )
        .unwrap();
        let b = &layout
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["kind"] == "table")
            .unwrap()["bbox"];
        let table =
            detect(&im, std::array::from_fn(|i| b[i].as_f64().unwrap() as f32)).expect("GDS grid");
        assert_eq!((table.rows, table.columns), (11, 4));
        for row in [2, 3] {
            assert_eq!(
                table
                    .cells
                    .iter()
                    .filter(|c| c.row == row && c.col_span == 1)
                    .count(),
                4
            );
        }
    }
    #[test]
    #[ignore = "private report; set CASY_TABLE_FIXTURE_DIR explicitly"]
    fn private_report_grid_review() {
        let root = std::path::PathBuf::from(
            std::env::var("CASY_TABLE_FIXTURE_DIR").expect("fixture directory"),
        );
        let pages: Vec<crate::Page> = serde_json::from_slice(
            &std::fs::read(root.join("baseline/source.document.json")).unwrap(),
        )
        .unwrap();
        for n in [2, 3] {
            let im = image::open(root.join(format!("page{n}-full.png")))
                .unwrap()
                .to_rgb8();
            let layout: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join(format!("baseline/page-{n}.layout.json"))).unwrap(),
            )
            .unwrap();
            let bbox = layout
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["kind"] == "table")
                .unwrap()["bbox"]
                .as_array()
                .unwrap();
            let bbox = std::array::from_fn(|i| bbox[i].as_f64().unwrap() as f32);
            let mut table = detect(&im, bbox).expect("real report grid");
            if n == 3 {
                assert_eq!((table.rows, table.columns), (21, 6));
                assert!(
                    table
                        .cells
                        .iter()
                        .any(|c| c.row == 1 && c.column == 5 && c.row_span == 17)
                );
            }
            if n == 2 {
                assert_eq!((table.rows, table.columns), (11, 4));
            }
            let md = markdown(&pages[n - 1].regions, std::slice::from_mut(&mut table), 0);
            if n == 2 {
                assert_eq!(
                    table
                        .cells
                        .iter()
                        .find(|c| c.row == 5 && c.column == 1)
                        .unwrap()
                        .regions
                        .len(),
                    2,
                    "long tilted text stays in its own row"
                );
            }
            std::fs::write(root.join(format!("page{n}-grid-preview.md")), md).unwrap();
            std::fs::write(
                root.join(format!("page{n}-grid.json")),
                serde_json::to_vec_pretty(&table).unwrap(),
            )
            .unwrap();
        }
    }
}
