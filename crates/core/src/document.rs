//! The single-layer RGBA8 pixel document.
//!
//! `Document` is the only mutable state `pixelcad-core` owns. It is plain
//! data plus deterministic operations: no wall-clock reads, no unseeded
//! randomness, no iteration-order-dependent compositing. Every method here
//! must produce the same output for the same input, every time, on every
//! platform, so that the CLI and the GUI (and the test harness) agree
//! byte-for-byte.

/// An RGBA8 color, stored as `[r, g, b, a]`.
pub type Color = [u8; 4];

/// Fully transparent black — the default fill for a freshly created canvas.
pub const TRANSPARENT: Color = [0, 0, 0, 0];

/// A single-layer, CPU-authoritative RGBA8 pixel document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    width: u32,
    height: u32,
    /// Packed RGBA8 pixels, row-major, top-left origin. Length is always
    /// exactly `width * height * 4`.
    pixels: Vec<u8>,
}

/// Errors returned by fallible `Document` operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("pixel coordinate ({x}, {y}) is out of bounds for a {width}x{height} document")]
    OutOfBounds { x: i64, y: i64, width: u32, height: u32 },
    #[error("canvas dimensions must be non-zero (got {width}x{height})")]
    ZeroSize { width: u32, height: u32 },
}

impl Document {
    /// Creates a new document of the given size, filled with
    /// [`TRANSPARENT`]. Both dimensions must be non-zero.
    pub fn new(width: u32, height: u32) -> Result<Self, DocumentError> {
        if width == 0 || height == 0 {
            return Err(DocumentError::ZeroSize { width, height });
        }
        let len = width as usize * height as usize * 4;
        Ok(Self {
            width,
            height,
            pixels: TRANSPARENT.iter().copied().cycle().take(len).collect(),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw packed RGBA8 buffer, row-major, top-left origin.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

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

    /// Reads the color at `(x, y)`.
    pub fn get_pixel(&self, x: i64, y: i64) -> Result<Color, DocumentError> {
        let i = self.index_of(x, y)?;
        Ok([self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]])
    }

    /// Writes `color` at `(x, y)`.
    pub fn set_pixel(&mut self, x: i64, y: i64, color: Color) -> Result<(), DocumentError> {
        let i = self.index_of(x, y)?;
        self.pixels[i..i + 4].copy_from_slice(&color);
        Ok(())
    }

    /// A deterministic content hash (FNV-1a, 64-bit) over the document's
    /// dimensions and pixel buffer. Used by determinism tests to compare two
    /// independently produced documents without comparing raw buffers.
    pub fn content_hash(&self) -> u64 {
        // FNV-1a: no external dependency, fully deterministic, stable across
        // platforms and Rust versions.
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
        fold(&self.pixels, &mut hash);
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_is_transparent() {
        let doc = Document::new(4, 3).unwrap();
        assert_eq!(doc.width(), 4);
        assert_eq!(doc.height(), 3);
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
        // Neighbours remain untouched.
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
}
