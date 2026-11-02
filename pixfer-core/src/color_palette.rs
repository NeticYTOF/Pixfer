//! Color palette: the 8-color 3-bit alphabet and its inverse mapping.
//! (Hilariously over-engineered: a "palette" for a screen that only needs
//! 8 colors to carry 3 bits per block.)

/// A named RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The 8-color palette, index 0..7 = 3-bit values 0..7.
    pub fn palette() -> [Color; 8] {
        [
            Color::rgb(0, 0, 0),       // 0 - black
            Color::rgb(255, 255, 255), // 1 - white
            Color::rgb(255, 0, 0),     // 2 - red
            Color::rgb(0, 255, 0),     // 3 - green
            Color::rgb(0, 0, 255),     // 4 - blue
            Color::rgb(255, 0, 255),   // 5 - magenta
            Color::rgb(255, 255, 0),   // 6 - yellow
            Color::rgb(128, 128, 128), // 7 - gray
        ]
    }

    /// The 3-bit value (0..=7) for a color, or None if not in the palette.
    pub fn value(&self) -> Option<u8> {
        for (i, c) in Self::palette().iter().enumerate() {
            if c == self {
                return Some(i as u8 & 0b111);
            }
        }
        None
    }
}

/// Convert a 3-bit value (0..=7) to a palette color.
pub fn color_for_value(v: u8) -> Color {
    Color::palette()[v as usize & 0b111]
}

/// Convert a palette color back to its 3-bit value, or None.
pub fn value_for_color(c: &Color) -> Option<u8> {
    c.value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_eight_distinct_colors() {
        let p = Color::palette();
        assert_eq!(p.len(), 8);
        let mut seen = std::collections::HashSet::new();
        for c in p {
            seen.insert(c);
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn value_roundtrip() {
        for v in 0..8u8 {
            let c = color_for_value(v);
            assert_eq!(value_for_color(&c), Some(v));
        }
    }

    #[test]
    fn unknown_color_returns_none() {
        let c = Color::rgb(10, 200, 30);
        assert_eq!(value_for_color(&c), None);
    }

    #[test]
    fn value_is_three_bit() {
        for v in 0..8u8 {
            assert!(color_for_value(v).value() == Some(v));
        }
    }
}
