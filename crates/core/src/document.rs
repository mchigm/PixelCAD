//! The multi-layer RGBA8 pixel document.
//!
//! `Document` is the only mutable state `pixelcad-core` owns. It is plain
//! data plus deterministic operations: no wall-clock reads, no unseeded
//! randomness, no iteration-order-dependent compositing. Every method here
//! must produce the same output for the same input, every time, on every
//! platform, so that the CLI and the GUI (and the test harness) agree
//! byte-for-byte.
//!
//! **Phase 1 change:** a document now owns an ordered stack of [`Layer`]s
//! instead of a single pixel buffer. Index `0` is the *bottom* layer;
//! `layers.len() - 1` is the *top*. Exactly one layer is active at any time,
//! and the un-suffixed pixel accessors ([`Document::get_pixel`],
//! [`Document::set_pixel`]) operate on it — which is why every Phase 0 call
//! site kept compiling. Anything that displays or exports the document uses
//! [`Document::composite`], never a single layer's raw buffer.

/// An RGBA8 color, stored as `[r, g, b, a]`.
pub type Color = [u8; 4];

/// Fully transparent black — the default fill for a freshly created canvas.
pub const TRANSPARENT: Color = [0, 0, 0, 0];

/// Fully opaque.
pub const OPAQUE: u8 = 255;

/// One layer of a [`Document`]: a full-canvas RGBA8 buffer plus its
/// presentation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    name: String,
    visible: bool,
    /// Layer-wide opacity multiplier, `0` (invisible) to `255` (unchanged).
    opacity: u8,
    /// Packed RGBA8 pixels, row-major, top-left origin. Length is always
    /// exactly `width * height * 4` of the owning document.
    pixels: Vec<u8>,
}

impl Layer {
    fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        let len = width as usize * height as usize * 4;
        Self { name: name.into(), visible: true, opacity: OPAQUE, pixels: vec![0u8; len] }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// This layer's raw pixel buffer, *uncomposited*. Display and export must
    /// use [`Document::composite`] instead.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Whether this layer contributes anything to the composite.
    fn contributes(&self) -> bool {
        self.visible && self.opacity > 0
    }
}

/// A CPU-authoritative, multi-layer RGBA8 pixel document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u32,
    height: u32,
    /// Bottom-to-top layer stack. Never empty.
    layers: Vec<Layer>,
    /// Index into `layers` of the layer that un-suffixed pixel operations
    /// target. Always a valid index.
    active: usize,
}

/// Errors returned by fallible `Document` operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("pixel coordinate ({x}, {y}) is out of bounds for a {width}x{height} document")]
    OutOfBounds { x: i64, y: i64, width: u32, height: u32 },
    #[error("canvas dimensions must be non-zero (got {width}x{height})")]
    ZeroSize { width: u32, height: u32 },
    #[error("layer index {index} is out of range (document has {count} layer(s))")]
    NoSuchLayer { index: usize, count: usize },
    #[error("cannot remove the last remaining layer")]
    LastLayer,
    #[error("layer 0 is the bottom layer; there is nothing below it to merge into")]
    NoLayerBelow,
}

impl Document {
    /// Creates a new document of the given size with a single fully
    /// transparent layer named `"Layer 1"`. Both dimensions must be non-zero.
    pub fn new(width: u32, height: u32) -> Result<Self, DocumentError> {
        if width == 0 || height == 0 {
            return Err(DocumentError::ZeroSize { width, height });
        }
        Ok(Self { width, height, layers: vec![Layer::new("Layer 1", width, height)], active: 0 })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    // ---------------------------------------------------------------- layers

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn active_layer_index(&self) -> usize {
        self.active
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active]
    }

    fn check_layer(&self, index: usize) -> Result<(), DocumentError> {
        if index >= self.layers.len() {
            return Err(DocumentError::NoSuchLayer { index, count: self.layers.len() });
        }
        Ok(())
    }

    /// Inserts a new transparent layer directly **above** the active layer
    /// and makes it active. Returns the new layer's index.
    pub fn add_layer(&mut self, name: impl Into<String>) -> usize {
        let at = self.active + 1;
        self.layers.insert(at, Layer::new(name, self.width, self.height));
        self.active = at;
        at
    }

    /// Makes `index` the active layer.
    pub fn select_layer(&mut self, index: usize) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        self.active = index;
        Ok(())
    }

    /// Removes layer `index`. The last remaining layer cannot be removed.
    pub fn remove_layer(&mut self, index: usize) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        if self.layers.len() == 1 {
            return Err(DocumentError::LastLayer);
        }
        self.layers.remove(index);
        // Keep `active` valid and pointing at the visually nearest survivor.
        if self.active >= self.layers.len() {
            self.active = self.layers.len() - 1;
        }
        Ok(())
    }

    pub fn rename_layer(&mut self, index: usize, name: impl Into<String>) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        self.layers[index].name = name.into();
        Ok(())
    }

    pub fn set_layer_visible(&mut self, index: usize, visible: bool) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        self.layers[index].visible = visible;
        Ok(())
    }

    pub fn set_layer_opacity(&mut self, index: usize, opacity: u8) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        self.layers[index].opacity = opacity;
        Ok(())
    }

    /// Moves layer `from` to position `to` in the stack, shifting the others.
    /// The active layer follows the moved layer if it *is* the moved layer.
    pub fn move_layer(&mut self, from: usize, to: usize) -> Result<(), DocumentError> {
        self.check_layer(from)?;
        self.check_layer(to)?;
        if from == to {
            return Ok(());
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        if self.active == from {
            self.active = to;
        } else {
            // Recompute where the previously active layer ended up.
            let mut a = self.active;
            if from < a {
                a -= 1;
            }
            if to <= a {
                a += 1;
            }
            self.active = a;
        }
        Ok(())
    }

    /// Inserts an independent copy of layer `index` directly above it, and
    /// makes the copy active.
    pub fn duplicate_layer(&mut self, index: usize) -> Result<usize, DocumentError> {
        self.check_layer(index)?;
        let mut copy = self.layers[index].clone();
        copy.name = format!("{} copy", copy.name);
        let at = index + 1;
        self.layers.insert(at, copy);
        self.active = at;
        Ok(at)
    }

    /// Composites layer `index` down onto `index - 1` and removes it.
    ///
    /// The merged layer keeps the *lower* layer's name, visibility and
    /// opacity, and the pixels it ends up holding are the two layers'
    /// existing combined result. Merging must never change what is on
    /// screen, only how many layers produced it. An invisible or
    /// zero-opacity upper layer therefore contributes nothing at all, which
    /// is what makes "merge down" safe to press.
    pub fn merge_layer_down(&mut self, index: usize) -> Result<(), DocumentError> {
        self.check_layer(index)?;
        if index == 0 {
            return Err(DocumentError::NoLayerBelow);
        }

        let upper = self.layers[index].clone();
        if upper.contributes() {
            let lower = &mut self.layers[index - 1];
            for i in (0..lower.pixels.len()).step_by(4) {
                let src_a = scale_u8(upper.pixels[i + 3], upper.opacity);
                if src_a == 0 {
                    continue;
                }
                let src =
                    [upper.pixels[i], upper.pixels[i + 1], upper.pixels[i + 2], src_a];
                let dst =
                    [lower.pixels[i], lower.pixels[i + 1], lower.pixels[i + 2], lower.pixels[i + 3]];
                let blended = over(src, dst);
                lower.pixels[i..i + 4].copy_from_slice(&blended);
            }
        }

        self.layers.remove(index);
        if self.active >= self.layers.len() {
            self.active = self.layers.len() - 1;
        }
        Ok(())
    }

    /// Crops every layer to `(x, y, width, height)`, in document
    /// coordinates. Regions outside the old canvas become transparent, so a
    /// crop that extends past an edge is a legal "grow" rather than an
    /// error.
    pub fn crop(&mut self, x: i64, y: i64, width: u32, height: u32) -> Result<(), DocumentError> {
        if width == 0 || height == 0 {
            return Err(DocumentError::ZeroSize { width, height });
        }
        for layer in &mut self.layers {
            let mut out = vec![0u8; width as usize * height as usize * 4];
            for row in 0..height as i64 {
                for col in 0..width as i64 {
                    let (sx, sy) = (x + col, y + row);
                    if sx < 0 || sy < 0 || sx >= self.width as i64 || sy >= self.height as i64 {
                        continue;
                    }
                    let si = (sy as usize * self.width as usize + sx as usize) * 4;
                    let di = (row as usize * width as usize + col as usize) * 4;
                    out[di..di + 4].copy_from_slice(&layer.pixels[si..si + 4]);
                }
            }
            layer.pixels = out;
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// Resamples every layer to `width x height` with nearest-neighbour
    /// sampling.
    ///
    /// Nearest-neighbour is not a limitation here, it is the requirement:
    /// any smooth filter invents colours that were never in the palette and
    /// blurs the hard pixel edges the whole editor exists to preserve. The
    /// source index uses integer arithmetic (`dst * src / dst_size`) so the
    /// mapping is exact and identical on every platform.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), DocumentError> {
        if width == 0 || height == 0 {
            return Err(DocumentError::ZeroSize { width, height });
        }
        let (sw, sh) = (self.width as usize, self.height as usize);
        for layer in &mut self.layers {
            let mut out = vec![0u8; width as usize * height as usize * 4];
            for row in 0..height as usize {
                let sy = row * sh / height as usize;
                for col in 0..width as usize {
                    let sx = col * sw / width as usize;
                    let si = (sy * sw + sx) * 4;
                    let di = (row * width as usize + col) * 4;
                    out[di..di + 4].copy_from_slice(&layer.pixels[si..si + 4]);
                }
            }
            layer.pixels = out;
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    // ---------------------------------------------------------------- pixels

    fn index_of(&self, x: i64, y: i64) -> Result<usize, DocumentError> {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return Err(DocumentError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        Ok((y as usize * self.width as usize + x as usize) * 4)
    }

    /// Whether `(x, y)` lies inside the canvas.
    pub fn in_bounds(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    /// Reads the color at `(x, y)` **on the active layer**.
    pub fn get_pixel(&self, x: i64, y: i64) -> Result<Color, DocumentError> {
        self.get_pixel_on(self.active, x, y)
    }

    /// Writes `color` at `(x, y)` **on the active layer**. This is a
    /// *replace*, not a blend: writing a fully transparent color erases.
    pub fn set_pixel(&mut self, x: i64, y: i64, color: Color) -> Result<(), DocumentError> {
        let layer = self.active;
        self.set_pixel_on(layer, x, y, color)
    }

    /// Reads the color at `(x, y)` on a specific layer.
    pub fn get_pixel_on(&self, layer: usize, x: i64, y: i64) -> Result<Color, DocumentError> {
        self.check_layer(layer)?;
        let i = self.index_of(x, y)?;
        let p = &self.layers[layer].pixels;
        Ok([p[i], p[i + 1], p[i + 2], p[i + 3]])
    }

    /// Writes `color` at `(x, y)` on a specific layer.
    pub fn set_pixel_on(
        &mut self,
        layer: usize,
        x: i64,
        y: i64,
        color: Color,
    ) -> Result<(), DocumentError> {
        self.check_layer(layer)?;
        let i = self.index_of(x, y)?;
        self.layers[layer].pixels[i..i + 4].copy_from_slice(&color);
        Ok(())
    }

    /// Reads the *composited* color at `(x, y)` — what the user actually
    /// sees. This is what the eyedropper samples.
    pub fn composited_pixel(&self, x: i64, y: i64) -> Result<Color, DocumentError> {
        let i = self.index_of(x, y)?;
        let buf = self.composite();
        Ok([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]])
    }

    // ------------------------------------------------------------ composite

    /// Flattens the visible layer stack, bottom to top, into one packed
    /// RGBA8 buffer using straight-alpha "over" compositing in **integer
    /// math** (no floating point anywhere), so the result is bit-exact on
    /// every platform and every run.
    ///
    /// Two exactness properties this relies on, both asserted by tests:
    /// - compositing over a fully transparent backdrop returns the source
    ///   pixel unchanged (so a single opaque-opacity visible layer composites
    ///   to its own buffer byte-for-byte — this is what keeps Phase 0's
    ///   `ship.pxc` PNG hash stable across the layer reshape);
    /// - `opacity == 255` is an exact no-op on the source alpha.
    pub fn composite(&self) -> Vec<u8> {
        let len = self.width as usize * self.height as usize * 4;

        // Fast, exactly-identical path: exactly one contributing layer at
        // full opacity. Avoids any per-pixel arithmetic at all.
        let mut contributors = self.layers.iter().filter(|l| l.contributes());
        if let Some(only) = contributors.next() {
            if contributors.next().is_none() && only.opacity == OPAQUE {
                return only.pixels.clone();
            }
        }

        let mut out = vec![0u8; len];
        for layer in self.layers.iter().filter(|l| l.contributes()) {
            let op = layer.opacity;
            for i in (0..len).step_by(4) {
                let src_a = scale_u8(layer.pixels[i + 3], op);
                if src_a == 0 {
                    continue;
                }
                let src = [layer.pixels[i], layer.pixels[i + 1], layer.pixels[i + 2], src_a];
                let dst = [out[i], out[i + 1], out[i + 2], out[i + 3]];
                let blended = over(src, dst);
                out[i..i + 4].copy_from_slice(&blended);
            }
        }
        out
    }

    /// A deterministic content hash (FNV-1a, 64-bit) over the document's
    /// dimensions, active-layer index, and every layer's name, visibility,
    /// opacity and pixel buffer. Used by determinism tests to compare two
    /// independently produced documents without comparing raw buffers.
    pub fn content_hash(&self) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        let fold = |bytes: &[u8], hash: &mut u64| {
            for &byte in bytes {
                *hash ^= byte as u64;
                *hash = hash.wrapping_mul(FNV_PRIME);
            }
        };
        fold(&self.width.to_le_bytes(), &mut hash);
        fold(&self.height.to_le_bytes(), &mut hash);
        fold(&(self.active as u64).to_le_bytes(), &mut hash);
        fold(&(self.layers.len() as u64).to_le_bytes(), &mut hash);
        for layer in &self.layers {
            fold(&(layer.name.len() as u64).to_le_bytes(), &mut hash);
            fold(layer.name.as_bytes(), &mut hash);
            fold(&[layer.visible as u8, layer.opacity], &mut hash);
            fold(&layer.pixels, &mut hash);
        }
        hash
    }
}

/// Multiplies two 0..=255 values as if they were 0.0..=1.0 fractions,
/// rounding to nearest. Exact: `scale_u8(v, 255) == v` for all `v`.
fn scale_u8(value: u8, factor: u8) -> u8 {
    ((value as u32 * factor as u32 + 127) / 255) as u8
}

/// Straight-alpha "over" compositing of `src` onto `dst`, in integer math.
/// Exact identity when `dst` is fully transparent.
fn over(src: Color, dst: Color) -> Color {
    let sa = src[3] as u32;
    if sa == 255 || dst[3] == 0 {
        // Fully opaque source, or nothing underneath: the source wins
        // outright. Handling this explicitly (rather than falling through to
        // the general formula) is what makes the single-layer case
        // byte-identical to the raw buffer.
        return src;
    }
    let da = dst[3] as u32;
    // out_a = sa + da * (1 - sa), in 0..=255 fixed point.
    let inv = 255 - sa;
    let da_contrib = (da * inv + 127) / 255;
    let out_a = sa + da_contrib;
    if out_a == 0 {
        return TRANSPARENT;
    }
    let channel = |s: u8, d: u8| -> u8 {
        let num = s as u32 * sa + d as u32 * da_contrib;
        ((num + out_a / 2) / out_a) as u8
    };
    [channel(src[0], dst[0]), channel(src[1], dst[1]), channel(src[2], dst[2]), out_a as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_is_transparent_with_one_layer() {
        let doc = Document::new(4, 3).unwrap();
        assert_eq!(doc.width(), 4);
        assert_eq!(doc.height(), 3);
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.active_layer().name(), "Layer 1");
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(doc.get_pixel(x, y).unwrap(), TRANSPARENT);
            }
        }
    }

    #[test]
    fn zero_size_is_rejected() {
        assert_eq!(Document::new(0, 5), Err(DocumentError::ZeroSize { width: 0, height: 5 }));
        assert_eq!(Document::new(5, 0), Err(DocumentError::ZeroSize { width: 5, height: 0 }));
    }

    #[test]
    fn set_and_get_pixel_round_trips() {
        let mut doc = Document::new(8, 8).unwrap();
        let red = [255, 0, 0, 255];
        doc.set_pixel(3, 4, red).unwrap();
        assert_eq!(doc.get_pixel(3, 4).unwrap(), red);
        assert_eq!(doc.get_pixel(3, 3).unwrap(), TRANSPARENT);
        assert_eq!(doc.get_pixel(4, 4).unwrap(), TRANSPARENT);
    }

    #[test]
    fn out_of_bounds_access_errors() {
        let doc = Document::new(2, 2).unwrap();
        assert!(matches!(doc.get_pixel(-1, 0), Err(DocumentError::OutOfBounds { .. })));
        assert!(matches!(doc.get_pixel(0, 2), Err(DocumentError::OutOfBounds { .. })));
        let mut doc = doc;
        assert!(matches!(
            doc.set_pixel(2, 0, [1, 2, 3, 4]),
            Err(DocumentError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn content_hash_is_deterministic_and_sensitive_to_pixels() {
        let doc_a = Document::new(4, 4).unwrap();
        let doc_b = Document::new(4, 4).unwrap();
        assert_eq!(doc_a.content_hash(), doc_b.content_hash());

        let mut doc_c = doc_a.clone();
        doc_c.set_pixel(0, 0, [1, 1, 1, 1]).unwrap();
        assert_ne!(doc_a.content_hash(), doc_c.content_hash());
    }

    #[test]
    fn content_hash_is_sensitive_to_dimensions() {
        let doc_a = Document::new(4, 4).unwrap();
        let doc_b = Document::new(2, 8).unwrap();
        assert_ne!(doc_a.content_hash(), doc_b.content_hash());
    }

    #[test]
    fn content_hash_is_sensitive_to_every_layer_field() {
        let base = {
            let mut d = Document::new(2, 2).unwrap();
            d.add_layer("second");
            d
        };

        let mut renamed = base.clone();
        renamed.rename_layer(1, "other").unwrap();
        assert_ne!(base.content_hash(), renamed.content_hash(), "name must affect the hash");

        let mut hidden = base.clone();
        hidden.set_layer_visible(1, false).unwrap();
        assert_ne!(base.content_hash(), hidden.content_hash(), "visibility must affect the hash");

        let mut faded = base.clone();
        faded.set_layer_opacity(1, 128).unwrap();
        assert_ne!(base.content_hash(), faded.content_hash(), "opacity must affect the hash");

        let mut reselected = base.clone();
        reselected.select_layer(0).unwrap();
        assert_ne!(
            base.content_hash(),
            reselected.content_hash(),
            "the active layer is document state and must affect the hash"
        );
    }

    #[test]
    fn add_layer_inserts_above_active_and_selects_it() {
        let mut doc = Document::new(2, 2).unwrap();
        let idx = doc.add_layer("top");
        assert_eq!(idx, 1);
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers()[0].name(), "Layer 1");
        assert_eq!(doc.layers()[1].name(), "top");
    }

    #[test]
    fn layers_are_independent_buffers() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        assert_eq!(doc.get_pixel(0, 0).unwrap(), TRANSPARENT, "new layer starts empty");
        assert_eq!(doc.get_pixel_on(0, 0, 0).unwrap(), [255, 0, 0, 255]);
    }

    #[test]
    fn remove_layer_keeps_active_valid_and_refuses_the_last_one() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.add_layer("top");
        assert_eq!(doc.active_layer_index(), 1);
        doc.remove_layer(1).unwrap();
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.active_layer_index(), 0);
        assert_eq!(doc.remove_layer(0), Err(DocumentError::LastLayer));
    }

    #[test]
    fn layer_index_out_of_range_errors() {
        let mut doc = Document::new(2, 2).unwrap();
        assert_eq!(doc.select_layer(7), Err(DocumentError::NoSuchLayer { index: 7, count: 1 }));
        assert!(matches!(doc.get_pixel_on(7, 0, 0), Err(DocumentError::NoSuchLayer { .. })));
    }

    #[test]
    fn move_layer_reorders_and_tracks_the_active_layer() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.add_layer("b");
        doc.add_layer("c");
        // Stack is [Layer 1, b, c], active = 2 ("c").
        doc.move_layer(2, 0).unwrap();
        assert_eq!(
            doc.layers().iter().map(|l| l.name()).collect::<Vec<_>>(),
            vec!["c", "Layer 1", "b"]
        );
        assert_eq!(doc.active_layer_index(), 0, "the moved layer stays active");

        // Now move a non-active layer past the active one.
        doc.select_layer(1).unwrap(); // "Layer 1"
        doc.move_layer(0, 2).unwrap(); // move "c" to the top
        assert_eq!(
            doc.layers().iter().map(|l| l.name()).collect::<Vec<_>>(),
            vec!["Layer 1", "b", "c"]
        );
        assert_eq!(doc.active_layer().name(), "Layer 1", "active layer must follow its content");
    }

    #[test]
    fn single_opaque_layer_composites_to_its_own_buffer_byte_for_byte() {
        // This is the Phase 0 regression guard in miniature: the layer
        // reshape must not perturb single-layer output by even one byte.
        let mut doc = Document::new(3, 3).unwrap();
        doc.set_pixel(0, 0, [12, 34, 56, 255]).unwrap();
        doc.set_pixel(1, 1, [200, 100, 50, 128]).unwrap();
        assert_eq!(doc.composite(), doc.active_layer().pixels());
    }

    #[test]
    fn top_layer_composites_over_bottom_layer() {
        let mut doc = Document::new(2, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap(); // bottom: red
        doc.set_pixel(1, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        doc.set_pixel(0, 0, [0, 0, 255, 255]).unwrap(); // top: blue over the left pixel

        let buf = doc.composite();
        assert_eq!(&buf[0..4], &[0, 0, 255, 255], "top layer must win");
        assert_eq!(&buf[4..8], &[255, 0, 0, 255], "bottom shows where the top is empty");
    }

    #[test]
    fn hidden_layer_is_excluded_from_the_composite() {
        let mut doc = Document::new(1, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        doc.set_pixel(0, 0, [0, 0, 255, 255]).unwrap();
        assert_eq!(&doc.composite()[0..4], &[0, 0, 255, 255]);

        doc.set_layer_visible(1, false).unwrap();
        assert_eq!(&doc.composite()[0..4], &[255, 0, 0, 255], "hiding the top reveals the bottom");
    }

    #[test]
    fn layer_opacity_blends_toward_the_layer_below() {
        let mut doc = Document::new(1, 1).unwrap();
        doc.set_pixel(0, 0, [0, 0, 0, 255]).unwrap(); // bottom: black
        doc.add_layer("top");
        doc.set_pixel(0, 0, [255, 255, 255, 255]).unwrap(); // top: white
        doc.set_layer_opacity(1, 128).unwrap();

        let px = &doc.composite()[0..4];
        assert_eq!(px[3], 255, "the stack is still fully opaque");
        // 128/255 white over black, rounded: ~128. Allow no slack — this is
        // integer math and must be exactly reproducible.
        assert_eq!(px[0], 128);
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
    }

    #[test]
    fn zero_opacity_layer_contributes_nothing() {
        let mut doc = Document::new(1, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        doc.set_pixel(0, 0, [0, 0, 255, 255]).unwrap();
        doc.set_layer_opacity(1, 0).unwrap();
        assert_eq!(&doc.composite()[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn composite_of_all_hidden_layers_is_transparent() {
        let mut doc = Document::new(1, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.set_layer_visible(0, false).unwrap();
        assert_eq!(&doc.composite()[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn composited_pixel_reads_through_the_stack() {
        let mut doc = Document::new(2, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        // The active layer is empty at (0,0), but the composite is not.
        assert_eq!(doc.get_pixel(0, 0).unwrap(), TRANSPARENT);
        assert_eq!(doc.composited_pixel(0, 0).unwrap(), [255, 0, 0, 255]);
    }

    #[test]
    fn scale_u8_is_exact_at_the_endpoints() {
        for v in 0..=255u8 {
            assert_eq!(scale_u8(v, 255), v, "full opacity must be an exact no-op");
            assert_eq!(scale_u8(v, 0), 0);
        }
    }

    #[test]
    fn over_is_identity_against_a_transparent_backdrop() {
        for a in [0u8, 1, 64, 128, 254, 255] {
            let src = [37, 111, 200, a];
            assert_eq!(over(src, TRANSPARENT), src);
        }
    }
}

#[cfg(test)]
mod phase_1_5_tests {
    use super::*;

    fn painted_on(doc: &Document, layer: usize) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        for y in 0..doc.height() as i64 {
            for x in 0..doc.width() as i64 {
                if doc.get_pixel_on(layer, x, y).unwrap()[3] != 0 {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn duplicate_layer_produces_an_independent_copy() {
        let mut doc = Document::new(4, 4).unwrap();
        doc.set_pixel(1, 1, [1, 2, 3, 255]).unwrap();

        let at = doc.duplicate_layer(0).unwrap();
        assert_eq!(at, 1);
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers()[1].name(), "Layer 1 copy");
        assert_eq!(doc.get_pixel_on(1, 1, 1).unwrap(), [1, 2, 3, 255]);

        // Independent: editing the copy must not touch the original.
        doc.set_pixel(1, 1, [9, 9, 9, 255]).unwrap();
        assert_eq!(doc.get_pixel_on(0, 1, 1).unwrap(), [1, 2, 3, 255]);
    }

    #[test]
    fn merge_down_does_not_change_the_visible_result() {
        let mut doc = Document::new(3, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.set_pixel(1, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        doc.set_pixel(1, 0, [0, 0, 255, 255]).unwrap();
        doc.set_pixel(2, 0, [0, 255, 0, 255]).unwrap();

        let before = doc.composite();
        doc.merge_layer_down(1).unwrap();
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.composite(), before, "merging must be visually invisible");
    }

    #[test]
    fn merge_down_honours_the_upper_layers_opacity_and_visibility() {
        let mut doc = Document::new(1, 1).unwrap();
        doc.set_pixel(0, 0, [255, 0, 0, 255]).unwrap();
        doc.add_layer("top");
        doc.set_pixel(0, 0, [0, 0, 255, 255]).unwrap();
        doc.set_layer_visible(1, false).unwrap();

        let before = doc.composite();
        doc.merge_layer_down(1).unwrap();
        assert_eq!(doc.composite(), before);
        assert_eq!(
            doc.get_pixel_on(0, 0, 0).unwrap(),
            [255, 0, 0, 255],
            "a hidden layer must contribute nothing when merged"
        );
    }

    #[test]
    fn merging_the_bottom_layer_is_refused() {
        let mut doc = Document::new(2, 2).unwrap();
        assert_eq!(doc.merge_layer_down(0), Err(DocumentError::NoLayerBelow));
    }

    #[test]
    fn crop_keeps_the_requested_window_on_every_layer() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.set_pixel(2, 2, [1, 1, 1, 255]).unwrap();
        doc.set_pixel(7, 7, [2, 2, 2, 255]).unwrap();
        doc.add_layer("second");
        doc.set_pixel(3, 3, [3, 3, 3, 255]).unwrap();

        doc.crop(2, 2, 4, 4).unwrap();
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(doc.get_pixel_on(0, 0, 0).unwrap(), [1, 1, 1, 255], "(2,2) is now (0,0)");
        assert_eq!(painted_on(&doc, 0), vec![(0, 0)], "(7,7) was cropped away");
        assert_eq!(painted_on(&doc, 1), vec![(1, 1)], "every layer is cropped alike");
    }

    #[test]
    fn cropping_outside_the_canvas_grows_with_transparency() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.set_pixel(0, 0, [5, 5, 5, 255]).unwrap();
        doc.crop(-1, -1, 4, 4).unwrap();
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(doc.get_pixel(1, 1).unwrap(), [5, 5, 5, 255]);
        assert_eq!(doc.get_pixel(0, 0).unwrap(), TRANSPARENT);
        assert_eq!(doc.get_pixel(3, 3).unwrap(), TRANSPARENT);
    }

    #[test]
    fn crop_and_resize_reject_a_zero_dimension() {
        let mut doc = Document::new(4, 4).unwrap();
        assert!(matches!(doc.crop(0, 0, 0, 4), Err(DocumentError::ZeroSize { .. })));
        assert!(matches!(doc.resize(4, 0), Err(DocumentError::ZeroSize { .. })));
    }

    #[test]
    fn doubling_the_size_replicates_each_pixel_into_a_block() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.set_pixel(0, 0, [10, 20, 30, 255]).unwrap();
        doc.resize(4, 4).unwrap();
        assert_eq!((doc.width(), doc.height()), (4, 4));
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(doc.get_pixel(x, y).unwrap(), [10, 20, 30, 255], "({x},{y})");
            }
        }
        assert_eq!(doc.get_pixel(2, 0).unwrap(), TRANSPARENT);
    }

    #[test]
    fn halving_the_size_samples_the_nearest_source_pixel() {
        let mut doc = Document::new(4, 4).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                doc.set_pixel(x, y, [(x * 10) as u8, (y * 10) as u8, 0, 255]).unwrap();
            }
        }
        doc.resize(2, 2).unwrap();
        // dst 0 -> src 0, dst 1 -> src 2 (integer 1*4/2).
        assert_eq!(doc.get_pixel(0, 0).unwrap(), [0, 0, 0, 255]);
        assert_eq!(doc.get_pixel(1, 1).unwrap(), [20, 20, 0, 255]);
    }

    #[test]
    fn resize_is_exact_and_reversible_for_integer_scales() {
        let mut doc = Document::new(3, 3).unwrap();
        doc.set_pixel(1, 1, [7, 8, 9, 255]).unwrap();
        let before = doc.composite();
        doc.resize(9, 9).unwrap();
        doc.resize(3, 3).unwrap();
        assert_eq!(doc.composite(), before, "3x up then down must round-trip exactly");
    }

    #[test]
    fn resize_applies_to_every_layer_and_keeps_metadata() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.add_layer("second");
        doc.set_layer_opacity(1, 128).unwrap();
        doc.set_pixel(0, 0, [1, 1, 1, 255]).unwrap();
        doc.resize(4, 4).unwrap();
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layers()[1].opacity(), 128);
        assert_eq!(doc.layers()[1].name(), "second");
        assert_eq!(doc.layers()[0].pixels().len(), 4 * 4 * 4);
    }
}
