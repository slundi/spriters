//! 8-bit palette PNG encoding (`--png8`), with its colour quantiser.

use image::RgbaImage;
use std::io::Write;

/// Encode `canvas` as an 8-bit palette PNG (`--png8`) into `writer`: up to 256 colours,
/// transparency kept through the `tRNS` chunk.
pub(super) fn encode_png8(canvas: &RgbaImage, writer: &mut impl Write) -> std::io::Result<()> {
    let (palette, indices) = quantize(canvas);
    let mut encoder = png::Encoder::new(writer, canvas.width(), canvas.height());
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(
        palette
            .iter()
            .flat_map(|color| [color[0], color[1], color[2]])
            .collect::<Vec<u8>>(),
    );
    // One alpha per palette entry; trailing opaque entries may be left out.
    let mut alpha: Vec<u8> = palette.iter().map(|color| color[3]).collect();
    while alpha.last() == Some(&255) {
        alpha.pop();
    }
    if !alpha.is_empty() {
        encoder.set_trns(alpha);
    }
    let png_error = |e: png::EncodingError| match e {
        png::EncodingError::IoError(e) => e,
        other => std::io::Error::other(other),
    };
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(&indices).map_err(png_error)?;
    // Writes the end chunk and flushes; dropping the writer would ignore their errors.
    writer.finish().map_err(png_error)
}

/// Reduce `canvas` to a palette of at most 256 RGBA colours and one palette index per pixel.
///
/// Up to 256 distinct colours are kept exactly. Beyond, the visible pixels are quantised
/// with [`median_cut`] into 255 colours, entry 0 being reserved for full transparency so that
/// it stays exact. Fully transparent pixels always share a single entry, whatever their RGB.
fn quantize(canvas: &RgbaImage) -> (Vec<[u8; 4]>, Vec<u8>) {
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    let normalized = |pixel: &image::Rgba<u8>| if pixel.0[3] == 0 { CLEAR } else { pixel.0 };

    let mut exact: std::collections::HashMap<[u8; 4], u8> = std::collections::HashMap::new();
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let fits = canvas.pixels().all(|pixel| {
        let color = normalized(pixel);
        if exact.contains_key(&color) {
            return true;
        }
        if palette.len() == 256 {
            return false;
        }
        exact.insert(color, palette.len() as u8);
        palette.push(color);
        true
    });
    if fits {
        let indices = canvas
            .pixels()
            .map(|pixel| exact[&normalized(pixel)])
            .collect();
        return (palette, indices);
    }

    let mut counts: std::collections::HashMap<[u8; 4], u32> = std::collections::HashMap::new();
    for pixel in canvas.pixels().filter(|pixel| pixel.0[3] != 0) {
        *counts.entry(pixel.0).or_default() += 1;
    }
    let (visible_palette, visible_index) = median_cut(counts.into_iter().collect(), 255);
    let palette = std::iter::once(CLEAR).chain(visible_palette).collect();
    let indices = canvas
        .pixels()
        .map(|pixel| {
            if pixel.0[3] == 0 {
                0
            } else {
                visible_index[&pixel.0] + 1
            }
        })
        .collect();
    (palette, indices)
}

/// Median-cut quantisation of distinct RGBA `colors` (with their pixel count) into at most
/// `max_colors` entries: the box with the widest channel is split at its weighted median until
/// there are enough boxes, each box then giving the weighted average of its colours.
///
/// Returns the palette and the palette index of every input colour. Deterministic: the result
/// does not depend on the order of `colors`.
fn median_cut(
    mut colors: Vec<([u8; 4], u32)>,
    max_colors: usize,
) -> (Vec<[u8; 4]>, std::collections::HashMap<[u8; 4], u8>) {
    /// Channel with the widest value range in `colors`, and that range.
    fn widest_channel(colors: &[([u8; 4], u32)]) -> (usize, u8) {
        (0..4)
            .map(|channel| {
                let values = colors.iter().map(|(color, _)| color[channel]);
                let range = values.clone().max().unwrap_or(0) - values.min().unwrap_or(0);
                (channel, range)
            })
            .max_by_key(|&(channel, range)| (range, std::cmp::Reverse(channel)))
            .unwrap_or((0, 0))
    }

    colors.sort_unstable();
    let mut boxes = vec![colors];
    while boxes.len() < max_colors {
        let Some((index, channel)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, colors)| colors.len() > 1)
            .map(|(index, colors)| (index, widest_channel(colors)))
            .max_by_key(|&(index, (_, range))| (range, std::cmp::Reverse(index)))
            .map(|(index, (channel, _))| (index, channel))
        else {
            break;
        };
        let mut colors = boxes.remove(index);
        colors.sort_unstable_by_key(|&(color, _)| (color[channel], color));
        // Weighted median, keeping at least one colour on each side.
        let total: u64 = colors.iter().map(|&(_, count)| count as u64).sum();
        let mut seen = 0u64;
        let split = colors
            .iter()
            .position(|&(_, count)| {
                seen += count as u64;
                seen * 2 >= total
            })
            .map_or(1, |position| position + 1)
            .clamp(1, colors.len() - 1);
        let upper = colors.split_off(split);
        boxes.push(colors);
        boxes.push(upper);
    }

    let mut index_of = std::collections::HashMap::new();
    let palette = boxes
        .iter()
        .enumerate()
        .map(|(index, colors)| {
            let total: u64 = colors.iter().map(|&(_, count)| count as u64).sum();
            let mut average = [0u8; 4];
            for (channel, value) in average.iter_mut().enumerate() {
                let sum: u64 = colors
                    .iter()
                    .map(|&(color, count)| color[channel] as u64 * count as u64)
                    .sum();
                *value = ((sum + total / 2) / total) as u8;
            }
            for &(color, _) in colors {
                index_of.insert(color, index as u8);
            }
            average
        })
        .collect();
    (palette, index_of)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_keeps_few_colours_exactly() {
        let mut canvas = RgbaImage::new(4, 2);
        canvas.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        canvas.put_pixel(1, 0, image::Rgba([0, 0, 255, 128]));
        // Fully transparent pixels share one entry whatever their RGB.
        canvas.put_pixel(2, 0, image::Rgba([9, 9, 9, 0]));
        let (palette, indices) = quantize(&canvas);

        assert_eq!(palette.len(), 3);
        for (pixel, index) in canvas.pixels().zip(&indices) {
            let expected = if pixel.0[3] == 0 {
                [0, 0, 0, 0]
            } else {
                pixel.0
            };
            assert_eq!(palette[*index as usize], expected);
        }
    }

    #[test]
    fn test_quantize_many_colours_keeps_transparency_exact() {
        // 32×32 gradient (1024 colours) with a transparent first row.
        let canvas = RgbaImage::from_fn(32, 32, |x, y| {
            if y == 0 {
                image::Rgba([200, 10, 10, 0])
            } else {
                image::Rgba([(x * 8) as u8, (y * 8) as u8, 128, 255])
            }
        });
        let (palette, indices) = quantize(&canvas);

        assert!(palette.len() <= 256);
        assert_eq!(
            palette[0],
            [0, 0, 0, 0],
            "entry 0 is reserved for transparency"
        );
        for (pixel, index) in canvas.pixels().zip(&indices) {
            let color = palette[*index as usize];
            if pixel.0[3] == 0 {
                assert_eq!(*index, 0);
            } else {
                assert_ne!(*index, 0, "a visible pixel must not become transparent");
                for channel in 0..3 {
                    let diff = (color[channel] as i32 - pixel.0[channel] as i32).abs();
                    assert!(diff <= 24, "{:?} -> {:?}", pixel.0, color);
                }
            }
        }
    }

    #[test]
    fn test_median_cut_is_deterministic_and_bounded() {
        let colors: Vec<([u8; 4], u32)> = (0..600u32)
            .map(|value| {
                (
                    [(value % 256) as u8, (value / 3) as u8, 7, 255],
                    value % 5 + 1,
                )
            })
            .collect();
        let mut reversed = colors.clone();
        reversed.reverse();

        let (palette, index_of) = median_cut(colors.clone(), 255);
        assert_eq!(palette.len(), 255);
        assert_eq!(
            median_cut(reversed, 255),
            (palette.clone(), index_of.clone())
        );
        for (color, _) in &colors {
            assert!((index_of[color] as usize) < palette.len());
        }
        // Fewer colours than entries: each keeps its own entry, exactly.
        let few = vec![([1, 2, 3, 255], 4), ([200, 100, 50, 128], 1)];
        let (palette, index_of) = median_cut(few, 255);
        assert_eq!(palette.len(), 2);
        assert_eq!(
            palette[index_of[&[200, 100, 50, 128]] as usize],
            [200, 100, 50, 128]
        );
    }
}
