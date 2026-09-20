//! A selection: a bounding box plus an optional per-cell coverage mask.
//!
//! Phase 1 had only rectangles, so a selection *was* its bounding box. Lasso
//! selection breaks that, and rather than add a second, parallel "irregular
//! selection" concept the two are unified here: a [`Selection`] is always a
//! box, and it *may* additionally carry a mask saying which cells inside
//! that box are actually covered.
//!
//! Why a dense `Vec<bool>` rather than a run-length or region representation:
//! every hot operation is a point query (`contains`) during a per-pixel write
//! loop, which a dense mask answers with one index. A cleverer structure
//! would save memory on huge selections and cost time on every stroke. If
//! selection memory ever matters, the place to fix it is here, behind this
//! unchanged interface.
//!
//! Deterministic by construction: no floating point, no iteration-order
//! sensitivity. The polygon scan-conversion in [`Selection::from_polygon`]
//! uses integer edge arithmetic and a fixed crossing order, so the same
//! points always produce the same mask on every platform.

/// A selected region of the canvas.
///
/// An empty selection (zero width or height) is legal and selects nothing;
/// drawing through it is a no-op rather than an error, which is what a real
/// editor does when you draw outside a marquee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    /// `None` means the whole bounding box is covered. `Some(m)` is
    /// row-major coverage of the box, with `m.len() == width * height`.
    mask: Option<Vec<bool>>,
}

impl Selection {
    /// A rectangular selection.
    pub fn rect(x: i64, y: i64, width: u32, height: u32) -> Self {
        Self { x, y, width, height, mask: None }
    }

    /// A selection covering the whole of a `width x height` canvas.
    pub fn all(width: u32, height: u32) -> Self {
        Self::rect(0, 0, width, height)
    }

    /// A selection from an explicit coverage mask.
    ///
    /// The mask is normalised: if it is the wrong length it is rejected
    /// (`None` is returned), and if every cell is covered the mask is
    /// dropped so the selection compares equal to the plain rectangle. That
    /// normalisation is what lets `PartialEq` mean "selects the same cells"
    /// rather than "was built the same way".
    pub fn from_mask(x: i64, y: i64, width: u32, height: u32, mask: Vec<bool>) -> Option<Self> {
        if mask.len() != width as usize * height as usize {
            return None;
        }
        let mask = if mask.iter().all(|c| *c) { None } else { Some(mask) };
        Some(Self { x, y, width, height, mask })
    }

    /// Scan-converts a closed polygon into a masked selection.
    ///
    /// Two passes, and the second one matters:
    ///
    /// 1. **Interior**, by the even-odd (crossing-count) rule, treating each
    ///    vertex as a pixel *centre*.
    /// 2. **Outline**, by stamping every edge with Bresenham.
    ///
    /// Without pass 2 this would follow the usual top-left fill rule, which
    /// drops the bottom and right boundary — so a lasso traced around a
    /// square would fail to select the very pixels the user traced over.
    /// That is correct for rendering a polygon and wrong for selecting one.
    /// Stamping the outline makes the boundary inclusive, which is what
    /// every pixel editor's lasso does.
    ///
    /// Fewer than three points selects nothing: a point or a line segment
    /// has no interior, and silently promoting it to a rectangle would be a
    /// surprise.
    pub fn from_polygon(points: &[(i64, i64)]) -> Self {
        if points.len() < 3 {
            return Self::rect(0, 0, 0, 0);
        }

        let min_x = points.iter().map(|p| p.0).min().unwrap();
        let max_x = points.iter().map(|p| p.0).max().unwrap();
        let min_y = points.iter().map(|p| p.1).min().unwrap();
        let max_y = points.iter().map(|p| p.1).max().unwrap();

        let width = (max_x - min_x + 1).max(0) as u32;
        let height = (max_y - min_y + 1).max(0) as u32;
        if width == 0 || height == 0 {
            return Self::rect(min_x, min_y, 0, 0);
        }

        let mut mask = vec![false; width as usize * height as usize];
        let mut set = |x: i64, y: i64| {
            let (dx, dy) = (x - min_x, y - min_y);
            if dx >= 0 && dy >= 0 && dx < width as i64 && dy < height as i64 {
                mask[dy as usize * width as usize + dx as usize] = true;
            }
        };

        // Pass 1 — interior.
        for row in 0..height as i64 {
            let line = min_y + row;
            let mut crossings: Vec<i64> = Vec::new();

            for i in 0..points.len() {
                let (x0, y0) = points[i];
                let (x1, y1) = points[(i + 1) % points.len()];
                // Half-open test: an edge counts only if the scanline is
                // below exactly one of its endpoints, so a shared vertex is
                // never counted twice and horizontal edges are skipped.
                if (y0 > line) == (y1 > line) {
                    continue;
                }
                crossings.push(x0 + (line - y0) * (x1 - x0) / (y1 - y0));
            }

            crossings.sort_unstable();
            for pair in crossings.chunks_exact(2) {
                for x in pair[0]..=pair[1] {
                    set(x, line);
                }
            }
        }

        // Pass 2 — outline, so the traced boundary is always included.
        for i in 0..points.len() {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            for (x, y) in crate::engine::bresenham_points(x0, y0, x1, y1) {
                set(x, y);
            }
        }

        Self::from_mask(min_x, min_y, width, height, mask)
            .expect("mask was built at exactly width*height")
    }

    pub fn x(&self) -> i64 {
        self.x
    }

    pub fn y(&self) -> i64 {
        self.y
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// `(x, y, width, height)` of the bounding box.
    pub fn bounds(&self) -> (i64, i64, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }

    /// Whether this selection is rectangular (has no mask).
    pub fn is_rect(&self) -> bool {
        self.mask.is_none()
    }

    /// Whether the selection covers no cells at all.
    pub fn is_empty(&self) -> bool {
        self.width == 0
            || self.height == 0
            || self.mask.as_ref().is_some_and(|m| !m.iter().any(|c| *c))
    }

    /// Number of covered cells.
    pub fn count(&self) -> usize {
        match &self.mask {
            None => self.width as usize * self.height as usize,
            Some(m) => m.iter().filter(|c| **c).count(),
        }
    }

    /// Whether `(x, y)` is selected.
    pub fn contains(&self, x: i64, y: i64) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        if dx < 0 || dy < 0 || dx >= self.width as i64 || dy >= self.height as i64 {
            return false;
        }
        match &self.mask {
            None => true,
            Some(m) => m[dy as usize * self.width as usize + dx as usize],
        }
    }

    /// The same shape, translated by `(dx, dy)`.
    pub fn translated(&self, dx: i64, dy: i64) -> Self {
        Self { x: self.x + dx, y: self.y + dy, ..self.clone() }
    }

    /// The same box, with the mask mirrored horizontally.
    pub fn flipped_horizontally(&self) -> Self {
        self.remapped(|col, row, w, _| (w - 1 - col, row))
    }

    /// The same box, with the mask mirrored vertically.
    pub fn flipped_vertically(&self) -> Self {
        self.remapped(|col, row, _, h| (col, h - 1 - row))
    }

    /// Applies a coordinate remap to the mask, keeping the box unchanged.
    fn remapped(&self, f: impl Fn(usize, usize, usize, usize) -> (usize, usize)) -> Self {
        let Some(mask) = &self.mask else {
            return self.clone();
        };
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![false; mask.len()];
        for row in 0..h {
            for col in 0..w {
                let (sc, sr) = f(col, row, w, h);
                out[row * w + col] = mask[sr * w + sc];
            }
        }
        Self { x: self.x, y: self.y, width: self.width, height: self.height, mask: Some(out) }
    }

    /// Iterates every selected cell in scan order (top to bottom, then left
    /// to right). The order is fixed and callers may rely on it.
    pub fn cells(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        let (x0, y0, w, h) = (self.x, self.y, self.width as i64, self.height as i64);
        (0..h).flat_map(move |row| (0..w).map(move |col| (col, row))).filter_map(
            move |(col, row)| {
                let (px, py) = (x0 + col, y0 + row);
                self.contains(px, py).then_some((px, py))
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_selects_its_whole_box() {
        let s = Selection::rect(2, 3, 4, 5);
        assert_eq!(s.bounds(), (2, 3, 4, 5));
        assert!(s.is_rect());
        assert_eq!(s.count(), 20);
        assert!(s.contains(2, 3));
        assert!(s.contains(5, 7));
        assert!(!s.contains(1, 3));
        assert!(!s.contains(6, 3));
        assert!(!s.contains(2, 8));
    }

    #[test]
    fn select_all_covers_the_canvas() {
        let s = Selection::all(3, 2);
        assert_eq!(s.count(), 6);
        assert!(s.contains(0, 0));
        assert!(s.contains(2, 1));
        assert!(!s.contains(3, 1));
    }

    #[test]
    fn an_empty_selection_is_recognised() {
        assert!(Selection::rect(0, 0, 0, 5).is_empty());
        assert!(Selection::rect(0, 0, 5, 0).is_empty());
        assert!(!Selection::rect(0, 0, 1, 1).is_empty());

        let blank = Selection::from_mask(0, 0, 2, 2, vec![false; 4]).unwrap();
        assert!(blank.is_empty());
        assert_eq!(blank.count(), 0);
    }

    #[test]
    fn a_fully_covered_mask_normalises_to_a_rectangle() {
        let s = Selection::from_mask(1, 1, 2, 2, vec![true; 4]).unwrap();
        assert!(s.is_rect(), "an all-true mask is just a rectangle");
        assert_eq!(s, Selection::rect(1, 1, 2, 2), "and must compare equal to one");
    }

    #[test]
    fn a_wrong_sized_mask_is_rejected() {
        assert!(Selection::from_mask(0, 0, 2, 2, vec![true; 3]).is_none());
        assert!(Selection::from_mask(0, 0, 2, 2, vec![true; 5]).is_none());
    }

    #[test]
    fn a_triangle_selects_a_non_rectangular_region() {
        // A right triangle with the square corner at the top left.
        let s = Selection::from_polygon(&[(0, 0), (4, 0), (0, 4)]);
        assert_eq!(s.bounds(), (0, 0, 5, 5));
        assert!(!s.is_rect(), "a triangle is not a rectangle");

        // Inside the hypotenuse.
        assert!(s.contains(0, 0));
        assert!(s.contains(1, 1));
        // Inside the bounding box but outside the triangle — this is the
        // whole point of masks.
        assert!(!s.contains(4, 4));
        assert!(!s.contains(3, 3));
        assert!(s.count() < 25);
    }

    #[test]
    fn a_polygon_scan_converts_a_solid_square_exactly() {
        // A degenerate "polygon" that is really a square must select every
        // cell, and therefore normalise back to a plain rectangle.
        let s = Selection::from_polygon(&[(0, 0), (3, 0), (3, 3), (0, 3)]);
        assert_eq!(s.bounds(), (0, 0, 4, 4));
        assert_eq!(s.count(), 16);
        assert!(s.is_rect());
    }

    #[test]
    fn a_concave_polygon_excludes_its_notch() {
        // An "L" shape: the top-right quadrant is cut out.
        let s = Selection::from_polygon(&[(0, 0), (2, 0), (2, 2), (4, 2), (4, 4), (0, 4)]);
        assert!(s.contains(0, 0), "inside the upper arm");
        assert!(s.contains(3, 3), "inside the lower arm");
        assert!(!s.contains(4, 0), "the notch must be excluded");
        assert!(!s.contains(3, 1));
    }

    #[test]
    fn fewer_than_three_points_selects_nothing() {
        assert!(Selection::from_polygon(&[]).is_empty());
        assert!(Selection::from_polygon(&[(1, 1)]).is_empty());
        assert!(Selection::from_polygon(&[(1, 1), (5, 5)]).is_empty());
    }

    #[test]
    fn polygon_scan_conversion_is_deterministic() {
        let points = [(1, 0), (7, 2), (5, 9), (0, 6), (3, 4)];
        assert_eq!(Selection::from_polygon(&points), Selection::from_polygon(&points));
    }

    #[test]
    fn translation_moves_the_shape_without_changing_it() {
        let s = Selection::from_polygon(&[(0, 0), (4, 0), (0, 4)]);
        let moved = s.translated(10, 20);
        assert_eq!(moved.bounds(), (10, 20, 5, 5));
        assert_eq!(moved.count(), s.count());
        for (x, y) in s.cells() {
            assert!(moved.contains(x + 10, y + 20));
        }
    }

    #[test]
    fn flipping_twice_is_the_identity() {
        let s = Selection::from_polygon(&[(0, 0), (4, 0), (0, 4)]);
        assert_eq!(s.flipped_horizontally().flipped_horizontally(), s);
        assert_eq!(s.flipped_vertically().flipped_vertically(), s);
    }

    #[test]
    fn flipping_a_triangle_moves_its_right_angle() {
        let s = Selection::from_polygon(&[(0, 0), (4, 0), (0, 4)]);
        assert!(s.contains(0, 4) && !s.contains(4, 4));
        let flipped = s.flipped_horizontally();
        assert!(flipped.contains(4, 4) && !flipped.contains(0, 4));
        assert_eq!(flipped.count(), s.count(), "flipping preserves area");
    }

    #[test]
    fn cells_are_emitted_in_scan_order_and_match_contains() {
        let s = Selection::from_polygon(&[(0, 0), (3, 0), (0, 3)]);
        let cells: Vec<_> = s.cells().collect();
        assert_eq!(cells.len(), s.count());
        for &(x, y) in &cells {
            assert!(s.contains(x, y));
        }
        let mut sorted = cells.clone();
        sorted.sort_by_key(|&(x, y)| (y, x));
        assert_eq!(cells, sorted, "cells() must emit top-to-bottom, left-to-right");
    }
}
