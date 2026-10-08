//! Big letters. A terminal cannot make its text larger, so the words a child has to
//! read are drawn as 5x7 bitmaps out of half-block characters: `▀` and `▄` give two
//! square pixels per cell, which makes a letter six columns wide and four rows tall.
//! Capitals, digits and a little punctuation only.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// Rows of a glyph, top to bottom, separated by spaces; `#` is an inked pixel.
const GLYPHS: &[(char, &str)] = &[
    ('A', ".###. #...# #...# ##### #...# #...# #...#"),
    ('B', "####. #...# #...# ####. #...# #...# ####."),
    ('C', ".###. #...# #.... #.... #.... #...# .###."),
    ('D', "####. #...# #...# #...# #...# #...# ####."),
    ('E', "##### #.... #.... ####. #.... #.... #####"),
    ('F', "##### #.... #.... ####. #.... #.... #...."),
    ('G', ".###. #...# #.... #.### #...# #...# .###."),
    ('H', "#...# #...# #...# ##### #...# #...# #...#"),
    ('I', "##### ..#.. ..#.. ..#.. ..#.. ..#.. #####"),
    ('J', "..### ...#. ...#. ...#. ...#. #..#. .##.."),
    ('K', "#...# #..#. #.#.. ##... #.#.. #..#. #...#"),
    ('L', "#.... #.... #.... #.... #.... #.... #####"),
    ('M', "#...# ##.## #.#.# #.#.# #...# #...# #...#"),
    ('N', "#...# ##..# ##..# #.#.# #..## #..## #...#"),
    ('O', ".###. #...# #...# #...# #...# #...# .###."),
    ('P', "####. #...# #...# ####. #.... #.... #...."),
    ('Q', ".###. #...# #...# #...# #.#.# #..#. .##.#"),
    ('R', "####. #...# #...# ####. #.#.. #..#. #...#"),
    ('S', ".#### #.... #.... .###. ....# ....# ####."),
    ('T', "##### ..#.. ..#.. ..#.. ..#.. ..#.. ..#.."),
    ('U', "#...# #...# #...# #...# #...# #...# .###."),
    ('V', "#...# #...# #...# #...# #...# .#.#. ..#.."),
    ('W', "#...# #...# #...# #.#.# #.#.# ##.## #...#"),
    ('X', "#...# #...# .#.#. ..#.. .#.#. #...# #...#"),
    ('Y', "#...# #...# .#.#. ..#.. ..#.. ..#.. ..#.."),
    ('Z', "##### ....# ...#. ..#.. .#... #.... #####"),
    ('0', ".###. #...# #..## #.#.# ##..# #...# .###."),
    ('1', "..#.. .##.. ..#.. ..#.. ..#.. ..#.. .###."),
    ('2', ".###. #...# ....# ...#. ..#.. .#... #####"),
    ('3', ".###. #...# ....# ..##. ....# #...# .###."),
    ('4', "...#. ..##. .#.#. #..#. ##### ...#. ...#."),
    ('5', "##### #.... ####. ....# ....# #...# .###."),
    ('6', ".###. #.... #.... ####. #...# #...# .###."),
    ('7', "##### ....# ...#. ..#.. ..#.. ..#.. ..#.."),
    ('8', ".###. #...# #...# .###. #...# #...# .###."),
    ('9', ".###. #...# #...# .#### ....# ....# .###."),
    ('?', ".###. #...# ....# ...#. ..#.. ..... ..#.."),
    ('!', "..#.. ..#.. ..#.. ..#.. ..#.. ..... ..#.."),
    ('.', "..... ..... ..... ..... ..... ..... ..#.."),
    (',', "..... ..... ..... ..... ..... ..#.. .#..."),
    ('\'', "..#.. ..#.. ..... ..... ..... ..... ....."),
    ('"', ".#.#. .#.#. ..... ..... ..... ..... ....."),
    ('-', "..... ..... ..... .###. ..... ..... ....."),
    (':', "..... ..#.. ..... ..... ..... ..#.. ....."),
    ('&', ".#... #.#.. #.#.. .#... #.#.# #..#. .##.#"),
    ('/', "....# ....# ...#. ..#.. .#... #.... #...."),
    ('(', "...#. ..#.. .#... .#... .#... ..#.. ...#."),
    (')', ".#... ..#.. ...#. ...#. ...#. ..#.. .#..."),
    // A star, for the stars a round earns.
    ('*', "..#.. ..#.. ##### .###. .###. ##.## #...#"),
];

/// Pixels a glyph is tall.
pub const HEIGHT: usize = 7;
/// Rows of cells a line of big letters takes at scale 1, its blank half row included.
pub const ROWS: u16 = 4;

/// The capital a character is drawn as: its own, or the plain letter under an accent.
fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'C',
        'ñ' | 'Ñ' => 'N',
        '’' | '‘' => '\'',
        _ => c.to_ascii_uppercase(),
    }
}

/// The columns of a character that are drawn: where they start in its bitmap and how
/// many there are. Letters and digits keep all five so that words line up; punctuation
/// is as narrow as its ink.
fn span(c: char) -> Option<(&'static str, usize, usize)> {
    let c = fold(c);
    if c == ' ' {
        return Some(("", 0, 3));
    }
    let bitmap = GLYPHS.iter().find(|(g, _)| *g == c)?.1;
    if c.is_ascii_alphanumeric() || c == '*' {
        return Some((bitmap, 0, 5));
    }
    let inked = |x: usize| bitmap.split(' ').any(|row| row.as_bytes()[x] == b'#');
    let first = (0..5).find(|&x| inked(x)).unwrap_or(0);
    let last = (0..5).rev().find(|&x| inked(x)).unwrap_or(4);
    Some((bitmap, first, last + 1 - first))
}

/// Whether every character of `text` has a big letter.
pub fn supported(text: &str) -> bool {
    text.chars().all(|c| span(c).is_some())
}

/// How many pixels wide `text` is at scale 1, with a pixel between letters.
pub fn width(text: &str) -> usize {
    let glyphs: usize = text.chars().filter_map(span).map(|(_, _, w)| w + 1).sum();
    glyphs.saturating_sub(1)
}

/// Breaks `text` into lines no wider than `max` pixels. `None` when it has a character
/// there is no big letter for, or a single word that is too wide.
pub fn wrap(text: &str, max: usize) -> Option<Vec<String>> {
    if !supported(text) {
        return None;
    }
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        if width(word) > max {
            return None;
        }
        match lines.last_mut() {
            Some(line) if width(&format!("{line} {word}")) <= max => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    Some(lines)
}

/// The pixels of `text`: `HEIGHT` rows, true where there is ink.
pub fn pixels(text: &str) -> Vec<Vec<bool>> {
    let mut rows = vec![Vec::new(); HEIGHT];
    for (i, (bitmap, first, w)) in text.chars().filter_map(span).enumerate() {
        for (y, row) in rows.iter_mut().enumerate() {
            if i > 0 {
                row.push(false);
            }
            let line = bitmap.split(' ').nth(y).unwrap_or("").as_bytes();
            row.extend((first..first + w).map(|x| line.get(x) == Some(&b'#')));
        }
    }
    rows
}

/// Rows of cells that `HEIGHT * scale` pixels need.
pub fn rows(scale: usize) -> u16 {
    (HEIGHT * scale).div_ceil(2) as u16
}

/// Draws `text` with its top left pixel at column `x`, pixel row `y` (two to a cell
/// row, counted from the top of the buffer), each pixel of the font `scale` pixels
/// across. Only the ink is drawn: whatever background is there stays.
pub fn draw(buf: &mut Buffer, x: i32, y: i32, text: &str, color: Color, scale: usize) {
    let bitmap = pixels(text);
    let scale = scale.max(1) as i32;
    let (w, h) = (bitmap[0].len() as i32 * scale, HEIGHT as i32 * scale);
    let ink = |px: i32, py: i32| px >= 0 && py >= y && py < y + h && bitmap[((py - y) / scale) as usize][(px / scale) as usize];
    for row in y.div_euclid(2)..=(y + h - 1).div_euclid(2) {
        for px in 0..w {
            let symbol = match (ink(px, row * 2), ink(px, row * 2 + 1)) {
                (true, true) => "█",
                (true, false) => "▀",
                (false, true) => "▄",
                (false, false) => continue,
            };
            let (Ok(cx), Ok(cy)) = (u16::try_from(x + px), u16::try_from(row)) else { continue };
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_symbol(symbol).set_fg(color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_is_five_by_seven_and_unique() {
        for (i, (c, bitmap)) in GLYPHS.iter().enumerate() {
            let rows: Vec<&str> = bitmap.split(' ').collect();
            assert_eq!(rows.len(), HEIGHT, "{c}");
            assert!(rows.iter().all(|row| row.len() == 5 && row.bytes().all(|b| b == b'#' || b == b'.')), "{c}");
            assert!(GLYPHS[..i].iter().all(|(d, b)| d != c && b != bitmap), "{c} is there twice");
        }
    }

    #[test]
    fn widths_and_wrapping() {
        assert_eq!(width("A"), 5);
        assert_eq!(width("AB"), 11);
        // A space is three pixels and a full stop one, each with a pixel beside it.
        assert_eq!(width("A B"), 5 + 1 + 3 + 1 + 5);
        assert_eq!(width("A."), 5 + 1 + 1);
        assert_eq!(width("são"), width("SAO"));
        assert!(supported("What is the capital of France?") && !supported("Köln → 東京"));
        assert_eq!(wrap("one two three", 40).unwrap(), ["ONE TWO".to_lowercase(), "three".into()]);
        assert_eq!(wrap("one", 10), None);
        assert_eq!(wrap("東", 100), None);
        assert_eq!(pixels("A B")[0].len(), width("A B"));
    }

    #[test]
    fn letters_are_drawn_with_half_blocks_and_clipped() {
        use ratatui::layout::Rect;
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 4));
        draw(&mut buf, 1, 0, "L", Color::Red, 1);
        let line = |y: u16| (0..8).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>();
        assert_eq!(line(0), " █      ");
        // The last pixel row of seven is the top half of the fourth cell row.
        assert_eq!(line(3), " ▀▀▀▀▀  ");
        // Half a cell lower, and off every edge: nothing panics.
        draw(&mut buf, -3, 1, "WWW", Color::Red, 2);
        draw(&mut buf, 6, -5, "W", Color::Red, 1);
    }
}
