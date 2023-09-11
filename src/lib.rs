pub mod imageutils {
    use image::{Rgb, Rgba};

    pub fn merge_colors(foreground: &Rgba<u8>, background: &Rgb<u8>) -> Vec<u8> {
        // Foreground is opaque, just return the color.
        if foreground.0[3] == 255 {
            return vec![foreground.0[0], foreground.0[1], foreground.0[2]];
        }

        // Convert the factor from u8 to f32, so that 0 is 0.0 and 255 is 1.0.
        let factor = foreground.0[3] as f32 / 255.0;

        // Function for mixing foreground and background colors.
        let map_channel = |fg_color: f32, bg_color: f32| {
            (bg_color as f32 * (1.0 - factor)) + fg_color as f32 * factor
        };

        // Zip over fb and bg colors, converting to the output color.
        foreground
            .0
            .into_iter()
            .zip(background.0)
            .map(|(fg, bg)| (fg as f32 / 255.0, bg as f32 / 255.0))
            .map(|(fg, bg)| map_channel(fg, bg))
            .map(|r| (r * 255.0).round() as u8)
            .collect::<Vec<_>>()
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn merges_colors_correctly() {
            let fg = image::Rgba([255, 0, 0, 128]);
            let bg = image::Rgb([0, 255, 0]);
            let result = super::merge_colors(&fg, &bg);
            assert_eq!(result, vec![128, 127, 0]);
        }
    }
}
