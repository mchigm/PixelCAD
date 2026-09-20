//! Built-in bitmap typefaces for stamping text into a pixel document.
//!
//! A blueprint editor has to put legible labels on a canvas whose pixels are
//! the unit of truth. Anti-aliased, hinted, DPI-aware system text cannot do
//! that: it is floating point, it is platform-dependent, and it would drag a
//! font-shaping dependency into the one crate every other crate builds on.
//! So text here is a *bitmap* operation — a glyph is a rectangle of bits, and
//! rasterising it is a bit test per pixel.
//!
//! **Why the data is shaped the way it is.** Each glyph is a 5x7 cell stored
//! as `[u8; 7]`, one byte per scanline, of which only the low five bits are
//! significant (bit 4 = leftmost pixel, bit 0 = rightmost). That is 7 bytes
//! per glyph and 665 bytes for the whole printable-ASCII range, small enough
//! to live in `.rodata` and be indexed by `code - 32` with no lookup table.
//! [`Font::Bold`] is *derived* from the same rows at render time with
//! `row | (row >> 1)`, not stored separately: a second table would double the
//! data and, worse, could silently drift out of sync with the first when
//! someone fixes a glyph.
//!
//! Deterministic by construction: pure integer arithmetic, no floating point,
//! no allocation-order or iteration-order sensitivity. The same string always
//! produces the same ordered sequence of plotted coordinates, on every
//! platform and every run.

/// Pixels across one glyph cell. Only this many low bits of a row are used.
const GLYPH_COLS: usize = 5;

/// Scanlines in one glyph cell.
const GLYPH_ROWS: usize = 7;

/// Advance width of a cell: 5 ink columns plus 1 column of letter spacing.
const ADVANCE_WIDTH: u32 = 6;

/// Height of a cell: 7 ink rows plus 1 row of line spacing.
const CELL_HEIGHT: u32 = 8;

/// First character code with a glyph entry (space).
const FIRST_CODE: u32 = 32;

/// Last character code with a glyph entry (tilde).
const LAST_CODE: u32 = 126;

/// Drawn in place of any character outside [`FIRST_CODE`]`..=`[`LAST_CODE`].
/// A filled cell outline is deliberately loud: an unrenderable character in a
/// blueprint label must be obvious on screen, never silently dropped.
static FALLBACK_5X7: [u8; 7] =
    [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];

/// Largest honoured text scale.
///
/// A scale is a pixel-block multiplier, so the cost of rasterising is
/// `O(scale^2)` per glyph pixel. Without a ceiling, `scale = u32::MAX` is not
/// a slow render, it is a non-terminating one. Clamping (rather than
/// saturating the *result*) is the honest contract: a caller asking for a
/// 4-billion-pixel glyph has a bug, and answering "u32::MAX pixels wide"
/// would just move the overflow into their code. Everything that reports a
/// size and everything that draws applies the same clamp, so [`measure`] and
/// [`rasterize`] can never disagree about what will appear.
pub const MAX_SCALE: u32 = 64;

/// The 95 printable-ASCII glyphs, indexed by `code - 32`.
///
/// Every entry is seven scanlines top to bottom; within a scanline bit 4 is
/// the leftmost pixel and bit 0 the rightmost, so the binary literals read as
/// the glyph does. Uppercase and digits fill all seven rows, lowercase sits on
/// rows 2..=6, and descenders are folded into row 6 rather than hanging below
/// the cell — legibility of a 5x7 label beats typographic purity, and keeping
/// all ink inside the cell keeps layout arithmetic exact.
// `static`, not `const`, and this is load-bearing rather than stylistic: a
// `const` is substituted into every use site, so each `&GLYPHS_5X7[i]` would
// borrow a freshly materialised temporary. Two lookups of the same character
// would then return different addresses, and the 665-byte table would be
// duplicated into every caller instead of living once in `.rodata`.
static GLYPHS_5X7: [[u8; 7]; 95] = [
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // ' ' (32)
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100], // '!' (33)
    [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '"' (34)
    [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010], // '#' (35)
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100], // '$' (36)
    [0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011], // '%' (37)
    [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101], // '&' (38)
    [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '\'' (39)
    [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010], // '(' (40)
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000], // ')' (41)
    [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000], // '*' (42)
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000], // '+' (43)
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000], // ',' (44)
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000], // '-' (45)
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100], // '.' (46)
    [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000], // '/' (47)
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // '0' (48)
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // '1' (49)
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // '2' (50)
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // '3' (51)
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // '4' (52)
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // '5' (53)
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // '6' (54)
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // '7' (55)
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // '8' (56)
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // '9' (57)
    [0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000], // ':' (58)
    [0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000], // ';' (59)
    [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010], // '<' (60)
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000], // '=' (61)
    [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000], // '>' (62)
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100], // '?' (63)
    [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110], // '@' (64)
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // 'A' (65)
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // 'B' (66)
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110], // 'C' (67)
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100], // 'D' (68)
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // 'E' (69)
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // 'F' (70)
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111], // 'G' (71)
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // 'H' (72)
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 'I' (73)
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100], // 'J' (74)
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // 'K' (75)
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // 'L' (76)
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001], // 'M' (77)
    [0b10001, 0b11001, 0b11001, 0b10101, 0b10011, 0b10011, 0b10001], // 'N' (78)
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // 'O' (79)
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // 'P' (80)
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101], // 'Q' (81)
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // 'R' (82)
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110], // 'S' (83)
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // 'T' (84)
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // 'U' (85)
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // 'V' (86)
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001], // 'W' (87)
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001], // 'X' (88)
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100], // 'Y' (89)
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // 'Z' (90)
    [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110], // '[' (91)
    [0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001], // '\\' (92)
    [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110], // ']' (93)
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000], // '^' (94)
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111], // '_' (95)
    [0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // '`' (96)
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111], // 'a' (97)
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110], // 'b' (98)
    [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110], // 'c' (99)
    [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111], // 'd' (100)
    [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110], // 'e' (101)
    [0b00110, 0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000], // 'f' (102)
    [0b00000, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b01110], // 'g' (103)
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001], // 'h' (104)
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110], // 'i' (105)
    [0b00010, 0b00000, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100], // 'j' (106)
    [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010], // 'k' (107)
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 'l' (108)
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101], // 'm' (109)
    [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001], // 'n' (110)
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110], // 'o' (111)
    [0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000], // 'p' (112)
    [0b00000, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b00001], // 'q' (113)
    [0b00000, 0b00000, 0b10110, 0b11000, 0b10000, 0b10000, 0b10000], // 'r' (114)
    [0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110], // 's' (115)
    [0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01001, 0b00110], // 't' (116)
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01111], // 'u' (117)
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100], // 'v' (118)
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010], // 'w' (119)
    [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001], // 'x' (120)
    [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110], // 'y' (121)
    [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111], // 'z' (122)
    [0b00011, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00011], // '{' (123)
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // '|' (124)
    [0b11000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b11000], // '}' (125)
    [0b00000, 0b00000, 0b01001, 0b10101, 0b10010, 0b00000, 0b00000], // '~' (126)
];

/// A built-in bitmap typeface.
///
/// Both faces share one 5x7 outline table and one 6x8 metric box, so swapping
/// between them never reflows a label — only its weight changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    /// The plain 5x7 face: one pixel of stroke weight.
    Small,
    /// The same outlines emboldened one pixel to the right, for headings and
    /// callouts that must survive being read at a distance.
    Bold,
}

impl Font {
    /// Every built-in face, in a fixed order suitable for menus and for
    /// exhaustive tests.
    pub const ALL: [Font; 2] = [Font::Small, Font::Bold];

    /// Parses a face name as it appears in a `.pxc` script. Case-insensitive;
    /// returns `None` rather than guessing, so a typo surfaces as a parse
    /// error instead of a silently wrong label.
    pub fn from_name(name: &str) -> Option<Font> {
        // Compared byte-wise against ASCII literals: no locale, no Unicode
        // case folding, no allocation, identical on every platform.
        if name.eq_ignore_ascii_case("small") {
            Some(Font::Small)
        } else if name.eq_ignore_ascii_case("bold") {
            Some(Font::Bold)
        } else {
            None
        }
    }

    /// The canonical lowercase name, the exact spelling [`Font::from_name`]
    /// round-trips and the spelling serialisation must emit.
    pub fn name(self) -> &'static str {
        match self {
            Font::Small => "small",
            Font::Bold => "bold",
        }
    }

    /// Horizontal advance from one glyph origin to the next, in pixels.
    /// Includes the one column of letter spacing, so it is wider than the
    /// 5-pixel ink cell.
    pub fn glyph_width(self) -> u32 {
        ADVANCE_WIDTH
    }

    /// Vertical advance from one baseline cell to the next, in pixels.
    /// Includes the one row of line spacing, so it is taller than the
    /// 7-pixel ink cell.
    pub fn glyph_height(self) -> u32 {
        CELL_HEIGHT
    }
}

/// Horizontal alignment of a rendered line, relative to the anchor x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    /// The anchor is the left edge of the line's advance box.
    Left,
    /// The anchor is the horizontal centre of the line's advance box.
    Center,
    /// The anchor is the right edge of the line's advance box.
    Right,
}

impl TextAlign {
    /// Parses an alignment name as it appears in a `.pxc` script.
    /// Case-insensitive; returns `None` for anything unrecognised.
    pub fn from_name(name: &str) -> Option<TextAlign> {
        if name.eq_ignore_ascii_case("left") {
            Some(TextAlign::Left)
        } else if name.eq_ignore_ascii_case("center") {
            Some(TextAlign::Center)
        } else if name.eq_ignore_ascii_case("right") {
            Some(TextAlign::Right)
        } else {
            None
        }
    }

    /// The canonical lowercase name, as [`TextAlign::from_name`] accepts it.
    pub fn name(self) -> &'static str {
        match self {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        }
    }
}

/// The scanlines of `ch`, or the fallback box if `ch` has no glyph.
fn glyph_data(ch: char) -> &'static [u8; 7] {
    let code = ch as u32;
    if code >= FIRST_CODE && code <= LAST_CODE {
        &GLYPHS_5X7[(code - FIRST_CODE) as usize]
    } else {
        &FALLBACK_5X7
    }
}

/// One scanline of `ch` as rendered by `font`, bit 4 leftmost.
///
/// [`Font::Bold`] is synthesised here: `row >> 1` moves every ink pixel one
/// column right (bit 4 is the left edge), and OR-ing it back in widens each
/// stroke to two pixels. The shift drops the pixel that would land in column
/// 5, which is exactly what keeps the emboldened glyph inside its 5-pixel
/// cell and preserves the shared advance width.
fn glyph_row(font: Font, ch: char, row: usize) -> u8 {
    let bits = glyph_data(ch)[row];
    match font {
        Font::Small => bits,
        Font::Bold => bits | (bits >> 1),
    }
}

/// Number of characters in the longest line, and the number of lines.
fn line_metrics(text: &str) -> (u32, u32) {
    let mut widest = 0u32;
    let mut lines = 0u32;
    for line in text.split('\n') {
        let count = u32::try_from(line.chars().count()).unwrap_or(u32::MAX);
        if count > widest {
            widest = count;
        }
        lines = lines.saturating_add(1);
    }
    (widest, lines)
}

/// The pixel footprint of rendered text, in pixels, before clipping.
///
/// The returned box is the *advance* box: it spans `glyph_width` per character
/// and `glyph_height` per line, so it includes the trailing column of letter
/// spacing and the trailing row of line spacing. Actual ink is therefore
/// always strictly inside it, never outside — which is what makes it safe to
/// use for hit-testing and for sizing a scratch buffer.
///
/// An empty string has no footprint at all, `(0, 0)`. A trailing `\n` creates
/// a real (blank) second line and so does count toward the height.
///
/// `scale` is clamped to `1..=`[`MAX_SCALE`]. Absurdly long input saturates
/// rather than overflowing, so this can never panic.
pub fn measure(text: &str, font: Font, scale: u32) -> (u32, u32) {
    if text.is_empty() {
        return (0, 0);
    }
    let scale = scale.clamp(1, MAX_SCALE);
    let (columns, lines) = line_metrics(text);
    let width = columns.saturating_mul(font.glyph_width()).saturating_mul(scale);
    let height = lines.saturating_mul(font.glyph_height()).saturating_mul(scale);
    (width, height)
}

/// Rasterises `text` and calls `plot(x, y)` for every pixel that is ink.
///
/// - `x`, `y` is the anchor: top-left for [`TextAlign::Left`], top-centre for
///   [`TextAlign::Center`], top-right for [`TextAlign::Right`]. Centring uses
///   truncating integer division of the line's advance width, so a line of odd
///   pixel width lands half a pixel to the right of true centre — always the
///   same way, on every platform.
/// - `scale` multiplies every glyph pixel into a `scale` x `scale` block; it
///   is clamped to `1..=`[`MAX_SCALE`], so `0` behaves as `1` and an absurd
///   value renders at the ceiling instead of never returning.
/// - `\n` starts a new line; lines are separated by
///   `font.glyph_height() * scale`. Each line is aligned independently.
/// - Characters outside printable ASCII render as a visible fallback box;
///   nothing is ever skipped silently and nothing panics.
/// - `plot` is called at most once per pixel, in strict top-to-bottom then
///   left-to-right order. Callers may rely on that order.
pub fn rasterize<F: FnMut(i64, i64)>(
    text: &str,
    font: Font,
    scale: u32,
    x: i64,
    y: i64,
    align: TextAlign,
    mut plot: F,
) {
    let scale = i64::from(scale.clamp(1, MAX_SCALE));
    let advance = i64::from(font.glyph_width()).saturating_mul(scale);
    let line_step = i64::from(font.glyph_height()).saturating_mul(scale);

    for (line_index, line) in text.split('\n').enumerate() {
        let columns = i64::try_from(line.chars().count()).unwrap_or(i64::MAX);
        let line_width = columns.saturating_mul(advance);
        let origin_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => x.saturating_sub(line_width / 2),
            TextAlign::Right => x.saturating_sub(line_width),
        };
        let line_top = y.saturating_add((line_index as i64).saturating_mul(line_step));

        // Rows are the outer loop so that emitted pixels are globally ordered
        // top-to-bottom and, within a row, left-to-right across the whole
        // line. Iterating glyph-by-glyph would be marginally faster but would
        // hand callers an interleaved order they could not depend on.
        for row in 0..GLYPH_ROWS {
            for sub_y in 0..scale {
                let py = line_top
                    .saturating_add((row as i64).saturating_mul(scale))
                    .saturating_add(sub_y);
                for (index, ch) in line.chars().enumerate() {
                    let bits = glyph_row(font, ch, row);
                    if bits == 0 {
                        continue;
                    }
                    let cell_x = origin_x.saturating_add((index as i64).saturating_mul(advance));
                    for col in 0..GLYPH_COLS {
                        if bits & (1 << (GLYPH_COLS - 1 - col)) == 0 {
                            continue;
                        }
                        let px = cell_x.saturating_add((col as i64).saturating_mul(scale));
                        for sub_x in 0..scale {
                            plot(px.saturating_add(sub_x), py);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collects every plotted pixel, preserving `rasterize`'s emission order.
    fn ink(
        text: &str,
        font: Font,
        scale: u32,
        x: i64,
        y: i64,
        align: TextAlign,
    ) -> Vec<(i64, i64)> {
        let mut pixels = Vec::new();
        rasterize(text, font, scale, x, y, align, |px, py| pixels.push((px, py)));
        pixels
    }

    /// Left-aligned ink at the origin — the common case in these tests.
    fn ink_at_origin(text: &str, font: Font, scale: u32) -> Vec<(i64, i64)> {
        ink(text, font, scale, 0, 0, TextAlign::Left)
    }

    fn bounds(pixels: &[(i64, i64)]) -> (i64, i64, i64, i64) {
        let min_x = pixels.iter().map(|p| p.0).min().expect("no ink");
        let max_x = pixels.iter().map(|p| p.0).max().expect("no ink");
        let min_y = pixels.iter().map(|p| p.1).min().expect("no ink");
        let max_y = pixels.iter().map(|p| p.1).max().expect("no ink");
        (min_x, max_x, min_y, max_y)
    }

    fn printable() -> impl Iterator<Item = char> {
        (FIRST_CODE..=LAST_CODE).map(|c| char::from_u32(c).expect("ASCII is always a char"))
    }

    #[test]
    fn the_table_covers_exactly_printable_ascii() {
        assert_eq!(GLYPHS_5X7.len(), 95);
        assert_eq!(LAST_CODE - FIRST_CODE + 1, GLYPHS_5X7.len() as u32);

        for ch in printable() {
            let index = (ch as u32 - FIRST_CODE) as usize;
            // `glyph_data` must resolve to the table entry, not the fallback.
            assert!(
                std::ptr::eq(glyph_data(ch), &GLYPHS_5X7[index]),
                "{ch:?} did not resolve to its own table entry"
            );
        }
    }

    #[test]
    fn every_row_fits_in_five_bits() {
        for (index, glyph) in GLYPHS_5X7.iter().enumerate() {
            for (row, bits) in glyph.iter().enumerate() {
                assert_eq!(
                    bits & !0b11111,
                    0,
                    "glyph {index} row {row} sets a bit outside the 5-pixel cell"
                );
            }
        }
        for bits in FALLBACK_5X7 {
            assert_eq!(bits & !0b11111, 0, "fallback sets a bit outside the cell");
        }
    }

    #[test]
    fn space_is_blank_and_dense_glyphs_are_not() {
        for font in Font::ALL {
            assert!(ink_at_origin(" ", font, 1).is_empty(), "{font:?} space is not blank");
            assert!(ink_at_origin("    ", font, 4).is_empty(), "{font:?} spaces are not blank");

            // A dense glyph must be substantially inked, not one stray pixel.
            for ch in ['#', 'W', '@', 'M'] {
                let count = ink_at_origin(&ch.to_string(), font, 1).len();
                assert!(count >= 15, "{font:?} {ch:?} produced only {count} pixels");
            }
        }
    }

    #[test]
    fn every_printable_glyph_except_space_has_ink() {
        for ch in printable().filter(|&c| c != ' ') {
            let count = ink_at_origin(&ch.to_string(), Font::Small, 1).len();
            assert!(count > 0, "{ch:?} rendered nothing");
        }
    }

    #[test]
    fn measure_agrees_with_the_rasterised_bounding_box() {
        let samples = [
            "A",
            "I",
            "Hello, World!",
            "PIXELCAD 0123",
            "AB\nCD",
            "SECTION A-A\nSCALE 1:4",
            "wide first line\nx",
        ];
        for text in samples {
            for scale in [1u32, 2, 3] {
                for font in Font::ALL {
                    let (width, height) = measure(text, font, scale);
                    let pixels = ink_at_origin(text, font, scale);
                    let (min_x, max_x, min_y, max_y) = bounds(&pixels);

                    // Nothing may escape the measured box.
                    assert!(min_x >= 0 && min_y >= 0, "{text:?} plotted before the origin");
                    assert!(
                        (max_x as u32) < width && (max_y as u32) < height,
                        "{text:?} at scale {scale} overflowed {width}x{height}"
                    );

                    // ...and the box may not be loose by a whole extra cell,
                    // which is what "agrees with the bounding box" means once
                    // inter-glyph and inter-line spacing are accounted for.
                    let slack_x = width - (max_x as u32 + 1);
                    let slack_y = height - (max_y as u32 + 1);
                    assert!(
                        slack_x < font.glyph_width() * scale,
                        "{text:?} width {width} is {slack_x} px looser than the ink"
                    );
                    assert!(
                        slack_y < font.glyph_height() * scale,
                        "{text:?} height {height} is {slack_y} px looser than the ink"
                    );
                }
            }
        }
    }

    #[test]
    fn measure_is_the_advance_box() {
        assert_eq!(measure("", Font::Small, 1), (0, 0));
        assert_eq!(measure("A", Font::Small, 1), (6, 8));
        assert_eq!(measure("A", Font::Bold, 1), (6, 8));
        assert_eq!(measure("ABC", Font::Small, 1), (18, 8));
        assert_eq!(measure("ABC", Font::Small, 3), (54, 24));
        // The longest line sets the width; a trailing newline adds a line.
        assert_eq!(measure("A\nBCD", Font::Small, 1), (18, 16));
        assert_eq!(measure("A\n", Font::Small, 1), (6, 16));
        // Scale 0 is clamped to 1.
        assert_eq!(measure("A", Font::Small, 0), measure("A", Font::Small, 1));
    }

    #[test]
    fn scale_squares_the_ink_and_zero_behaves_as_one() {
        for font in Font::ALL {
            for text in ["A", "PIXELCAD 0123", "AB\nCD"] {
                let base = ink_at_origin(text, font, 1);
                for scale in [2u32, 3, 4] {
                    let scaled = ink_at_origin(text, font, scale);
                    let factor = (scale * scale) as usize;
                    assert_eq!(
                        scaled.len(),
                        base.len() * factor,
                        "{text:?} at scale {scale} in {font:?}"
                    );
                }
                assert_eq!(ink_at_origin(text, font, 0), base, "scale 0 must act as scale 1");
            }
        }
    }

    #[test]
    fn multi_line_text_stacks_by_exactly_one_cell_height() {
        for font in Font::ALL {
            for scale in [1u32, 2, 3] {
                let step = i64::from(font.glyph_height() * scale);

                let (width, height) = measure("AB\nCD", font, scale);
                assert_eq!(width, 2 * font.glyph_width() * scale);
                assert_eq!(height, 2 * font.glyph_height() * scale);

                // The second line is the first line's raster, translated down
                // by exactly one cell height — and the two concatenate in
                // emission order.
                let combined = ink_at_origin("AB\nCD", font, scale);
                let mut expected = ink_at_origin("AB", font, scale);
                expected.extend(ink("CD", font, scale, 0, step, TextAlign::Left));
                assert_eq!(combined, expected);

                let first_line_bottom = ink_at_origin("AB", font, scale)
                    .iter()
                    .map(|p| p.1)
                    .max()
                    .expect("ink");
                let second_line_top = combined
                    .iter()
                    .map(|p| p.1)
                    .filter(|&py| py > first_line_bottom)
                    .min()
                    .expect("second line ink");
                // 'C' and 'D' both ink row 0, so the second line starts at the
                // full cell-height offset.
                assert_eq!(second_line_top, step);
            }
        }
    }

    #[test]
    fn alignment_only_translates_the_shape() {
        for font in Font::ALL {
            for scale in [1u32, 2, 3] {
                for text in ["A", "PIXELCAD", "SECTION A-A"] {
                    let line_width =
                        i64::from(text.chars().count() as u32 * font.glyph_width() * scale);
                    let left = ink(text, font, scale, 0, 0, TextAlign::Left);

                    let center = ink(text, font, scale, 0, 0, TextAlign::Center);
                    let expected_center: Vec<_> =
                        left.iter().map(|&(px, py)| (px - line_width / 2, py)).collect();
                    assert_eq!(center, expected_center, "center of {text:?}");

                    let right = ink(text, font, scale, 0, 0, TextAlign::Right);
                    let expected_right: Vec<_> =
                        left.iter().map(|&(px, py)| (px - line_width, py)).collect();
                    assert_eq!(right, expected_right, "right of {text:?}");
                }
            }
        }
    }

    #[test]
    fn each_line_of_a_multi_line_block_is_aligned_independently() {
        // "M" is one cell wide, "MMM" is three, so right-aligning must pull
        // the short line across to share the long line's right edge.
        let pixels = ink("MMM\nM", Font::Small, 1, 0, 0, TextAlign::Right);
        let step = i64::from(Font::Small.glyph_height());
        let short_line_min_x =
            pixels.iter().filter(|p| p.1 >= step).map(|p| p.0).min().expect("ink");
        let long_line_min_x =
            pixels.iter().filter(|p| p.1 < step).map(|p| p.0).min().expect("ink");
        assert_eq!(long_line_min_x, -18);
        assert_eq!(short_line_min_x, -6);
    }

    #[test]
    fn unsupported_characters_render_the_fallback_box() {
        let expected: Vec<(i64, i64)> = {
            let mut v = Vec::new();
            for (row, bits) in FALLBACK_5X7.iter().enumerate() {
                for col in 0..GLYPH_COLS {
                    if bits & (1 << (GLYPH_COLS - 1 - col)) != 0 {
                        v.push((col as i64, row as i64));
                    }
                }
            }
            v
        };
        assert!(!expected.is_empty());

        for ch in ['é', '\u{1F600}', '\u{0}', '\t', '\u{7f}', '\u{FFFD}', 'λ'] {
            let pixels = ink_at_origin(&ch.to_string(), Font::Small, 1);
            assert_eq!(pixels, expected, "{ch:?} did not render the fallback box");
        }

        // A fallback in the middle of a word must not disturb its neighbours.
        let mixed = ink_at_origin("AéB", Font::Small, 1);
        assert!(mixed.len() > ink_at_origin("AB", Font::Small, 1).len());
        assert_eq!(measure("AéB", Font::Small, 1), (18, 8));
    }

    #[test]
    fn rasterising_twice_produces_identical_ordered_output() {
        let text = "PIXELCAD 0123\nSECTION A-A\tédge";
        for font in Font::ALL {
            for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                for scale in [1u32, 2, 5] {
                    let first = ink(text, font, scale, -7, 13, align);
                    let second = ink(text, font, scale, -7, 13, align);
                    assert_eq!(first, second);
                    assert!(!first.is_empty());
                }
            }
        }
    }

    #[test]
    fn pixels_are_emitted_top_to_bottom_then_left_to_right() {
        let pixels = ink_at_origin("PIXELCAD\n0123", Font::Bold, 2);
        for pair in pixels.windows(2) {
            let (ax, ay) = pair[0];
            let (bx, by) = pair[1];
            assert!(
                by > ay || (by == ay && bx > ax),
                "emission order broke at ({ax}, {ay}) -> ({bx}, {by})"
            );
        }
    }

    #[test]
    fn bold_adds_ink_without_leaving_the_cell() {
        let small_i = ink_at_origin("I", Font::Small, 1);
        let bold_i = ink_at_origin("I", Font::Bold, 1);
        assert!(
            bold_i.len() > small_i.len(),
            "bold 'I' ({}) is not heavier than small 'I' ({})",
            bold_i.len(),
            small_i.len()
        );

        // Bold is a superset of small: emboldening only ever adds pixels.
        for ch in printable() {
            let text = ch.to_string();
            let small: Vec<_> = ink_at_origin(&text, Font::Small, 1);
            let bold: Vec<_> = ink_at_origin(&text, Font::Bold, 1);
            assert!(bold.len() >= small.len(), "bold {ch:?} lost ink");
            for pixel in &small {
                assert!(bold.contains(pixel), "bold {ch:?} dropped {pixel:?}");
            }
            for &(px, py) in &bold {
                assert!(
                    (0..GLYPH_COLS as i64).contains(&px),
                    "bold {ch:?} bled to column {px}, outside the 5-pixel cell"
                );
                assert!((0..GLYPH_ROWS as i64).contains(&py));
            }
        }

        // Metrics are identical, so switching weight never reflows a label.
        assert_eq!(Font::Small.glyph_width(), Font::Bold.glyph_width());
        assert_eq!(Font::Small.glyph_height(), Font::Bold.glyph_height());
        assert_eq!(
            measure("PIXELCAD 0123", Font::Small, 3),
            measure("PIXELCAD 0123", Font::Bold, 3)
        );
    }

    #[test]
    fn no_glyph_paints_outside_its_cell() {
        for scale in [1u32, 3] {
            let cell_w = GLYPH_COLS as i64 * i64::from(scale);
            let cell_h = GLYPH_ROWS as i64 * i64::from(scale);
            for font in Font::ALL {
                for ch in printable().chain(['é', '\u{1F600}']) {
                    for &(px, py) in &ink_at_origin(&ch.to_string(), font, scale) {
                        assert!(
                            (0..cell_w).contains(&px) && (0..cell_h).contains(&py),
                            "{font:?} {ch:?} at scale {scale} plotted ({px}, {py}) \
                             outside {cell_w}x{cell_h}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn glyphs_never_overlap_their_neighbours() {
        // With a 6-pixel advance and a 5-pixel cell there is always exactly
        // one blank column between glyphs, in both weights.
        for font in Font::ALL {
            let gap: Vec<i64> = ink_at_origin("MM", font, 1)
                .iter()
                .map(|p| p.0)
                .filter(|&px| px == 5)
                .collect();
            assert!(gap.is_empty(), "{font:?} inked the letter-spacing column");
        }
    }

    #[test]
    fn names_round_trip() {
        for font in Font::ALL {
            assert_eq!(Font::from_name(font.name()), Some(font));
        }
        assert_eq!(Font::from_name("SMALL"), Some(Font::Small));
        assert_eq!(Font::from_name("Bold"), Some(Font::Bold));
        assert_eq!(Font::from_name("italic"), None);
        assert_eq!(Font::from_name(""), None);

        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            assert_eq!(TextAlign::from_name(align.name()), Some(align));
        }
        assert_eq!(TextAlign::from_name("CENTER"), Some(TextAlign::Center));
        assert_eq!(TextAlign::from_name("centre"), None);
        assert_eq!(TextAlign::from_name("justify"), None);
    }

    #[test]
    fn empty_input_renders_nothing_and_never_panics() {
        for font in Font::ALL {
            assert!(ink_at_origin("", font, 1).is_empty());
            assert!(ink_at_origin("\n\n\n", font, 2).is_empty());
        }
        // Extreme anchors must saturate rather than overflow. Scale is
        // clamped to MAX_SCALE first, so these terminate promptly.
        ink("A", Font::Bold, u32::MAX, i64::MAX, i64::MAX, TextAlign::Right);
        ink("A", Font::Bold, u32::MAX, i64::MIN, i64::MIN, TextAlign::Center);
    }

    #[test]
    fn an_absurd_scale_is_clamped_rather_than_saturated() {
        // Regression guard for a real hang: `rasterize` used to clamp only
        // the lower bound, so `scale = u32::MAX` looped ~10^19 times. The
        // clamp must be identical in `measure` and `rasterize`, or the two
        // would disagree about what is on screen.
        let clamped = measure("AAAA", Font::Bold, MAX_SCALE);
        assert_eq!(measure("AAAA", Font::Bold, u32::MAX), clamped);
        assert_eq!(clamped, (4 * Font::Bold.glyph_width() * MAX_SCALE, Font::Bold.glyph_height() * MAX_SCALE));

        let drawn = ink_at_origin("A", Font::Small, u32::MAX);
        let expected = ink_at_origin("A", Font::Small, MAX_SCALE);
        assert_eq!(drawn, expected, "drawing must honour the same clamp as measuring");

        // And the bounding box of what was drawn stays inside what was
        // measured, which is the invariant the clamp exists to protect.
        let (w, h) = measure("A", Font::Small, u32::MAX);
        assert!(drawn.iter().all(|&(x, y)| x >= 0 && y >= 0 && x < w as i64 && y < h as i64));
    }

    #[test]
    fn a_glyph_lookup_is_stable_and_shared() {
        // Regression guard for GLYPHS_5X7 having been a `const`: two lookups
        // of the same character must return the *same* address, not two
        // copies of an inlined temporary.
        assert!(std::ptr::eq(glyph_data('A'), glyph_data('A')));
        assert!(!std::ptr::eq(glyph_data('A'), glyph_data('B')));
    }

    #[test]
    fn digits_and_letters_are_visually_distinct() {
        // A blueprint is unreadable if these collide, so pin the pairs that
        // most often do in a 5x7 cell.
        for (a, b) in [('0', 'O'), ('1', 'l'), ('1', 'I'), ('l', 'I'), ('5', 'S'), ('2', 'Z')] {
            assert_ne!(
                glyph_data(a),
                glyph_data(b),
                "{a:?} and {b:?} share an outline"
            );
        }
    }
}
