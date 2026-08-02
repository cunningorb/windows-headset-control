//! The headset icon, as pixels. Pure: no OS, no window, no device.
//!
//! One definition serves three consumers — the tray's runtime `HICON`, the
//! multi-size `.ico` embedded in the executable, and the tests that keep those
//! two from drifting apart.

/// Straight-alpha BGRA. Two opaque colours and full transparency: the shape is
/// not anti-aliased, which is what lets the `.ico` stay exact.
pub const FILL: u32 = 0xFF_F0F0F0;
pub const OUTLINE: u32 = 0xFF_1A1A1A;
pub const CLEAR: u32 = 0x00_000000;

// The 32-pixel drawing these came from, divided by 32. At n = 32 they reproduce
// it exactly; at any other size they scale it.
const RING_CX: f32 = 0.5;
const RING_CY: f32 = 0.593_75;
const RING_INNER: f32 = 0.328_125;
const RING_OUTER: f32 = 0.421_875;
const CUP_TOP: f32 = 0.468_75;
const CUP_BOTTOM: f32 = 0.843_75;
const CUP_LEFT_X0: f32 = 0.093_75;
const CUP_LEFT_X1: f32 = 0.281_25;
// The right cup is not given its own constants. It is mirrored from the left,
// because rounding two exactly-mirrored fractions independently does not
// produce mirrored integers — at n = 16 it put the cups at 2..5 and 12..15,
// which is off by one and visibly lopsided.

pub fn icon_pixels(n: usize) -> Vec<u32> {
    let nf = n as f32;
    let bound = |f: f32| (f * nf).round() as i32;
    let (cup_top, cup_bottom) = (bound(CUP_TOP), bound(CUP_BOTTOM));
    let (l0, l1) = (bound(CUP_LEFT_X0), bound(CUP_LEFT_X1));
    // Mirrored from the left cup rather than rounded from CUP_RIGHT_X0/X1
    // independently. The fractions are exact mirrors, but rounding them
    // separately is not: at n = 16 that put the left cup at 2..5 and the right
    // at 12..15, which is off by one and visibly lopsided.
    let (r0, r1) = (n as i32 - l1, n as i32 - l0);

    let mut shape = vec![false; n * n];
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            // Headband: an arc centred low, so only its upper half is drawn.
            let dx = (x as f32 + 0.5) / nf - RING_CX;
            let dy = (y as f32 + 0.5) / nf - RING_CY;
            let r = (dx * dx + dy * dy).sqrt();
            let band = (RING_INNER..=RING_OUTER).contains(&r) && dy < 0.0;

            // Ear cups, with their four outer corners notched off so the ends
            // read as rounded rather than square.
            let in_left = (l0..l1).contains(&x);
            let in_right = (r0..r1).contains(&x);
            let in_rows = (cup_top..cup_bottom).contains(&y);
            let end_row = y == cup_top || y == cup_bottom - 1;
            let end_col = x == l0 || x == l1 - 1 || x == r0 || x == r1 - 1;
            let cup = (in_left || in_right) && in_rows && !(end_row && end_col);

            if band || cup {
                shape[y as usize * n + x as usize] = true;
            }
        }
    }

    // Outline every clear pixel that touches the shape, so the glyph reads on a
    // light taskbar as well as a dark one.
    let mut px = vec![CLEAR; n * n];
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            let i = y as usize * n + x as usize;
            if shape[i] {
                px[i] = FILL;
                continue;
            }
            let touches = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..n as i32).contains(&nx)
                        && (0..n as i32).contains(&ny)
                        && shape[ny as usize * n + nx as usize]
                })
            });
            if touches {
                px[i] = OUTLINE;
            }
        }
    }
    px
}

/// Sizes Windows asks for: taskbar and tray, Start menu, and Explorer's larger
/// views. 256 costs about 256 KB uncompressed and is what keeps the largest
/// views sharp.
pub const ICON_SIZES: [usize; 5] = [16, 32, 48, 128, 256];

/// Encodes the icon as a Windows `.ico`.
///
/// The format is a 6-byte header, one 16-byte directory entry per image, then
/// the images. Each image is a DIB whose declared height is **twice** the real
/// height: the format expects a colour bitmap followed by an AND mask. The mask
/// is left all-zero because the 32-bit colour data carries its own alpha.
///
/// Written by hand rather than with an image crate: this is the whole format,
/// and the project takes no dependency it can avoid.
pub fn encode_ico(sizes: &[usize]) -> Vec<u8> {
    const HEADER: usize = 6;
    const ENTRY: usize = 16;
    const DIB_HEADER: usize = 40;

    let mask_stride = |n: usize| n.div_ceil(32) * 4;
    let image_len = |n: usize| DIB_HEADER + n * n * 4 + n * mask_stride(n);

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());

    let mut offset = HEADER + ENTRY * sizes.len();
    for &n in sizes {
        // 256 does not fit in a byte and is encoded as zero.
        let dim = if n >= 256 { 0u8 } else { n as u8 };
        out.push(dim); // width
        out.push(dim); // height
        out.push(0); // palette size: none
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(image_len(n) as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image_len(n);
    }

    for &n in sizes {
        let px = icon_pixels(n);

        out.extend_from_slice(&(DIB_HEADER as u32).to_le_bytes()); // biSize
        out.extend_from_slice(&(n as i32).to_le_bytes()); // biWidth
        out.extend_from_slice(&((n * 2) as i32).to_le_bytes()); // biHeight
        out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
        out.extend_from_slice(&0u32.to_le_bytes()); // biCompression: BI_RGB
        out.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
        out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

        // Bottom-up, which is what a positive biHeight means.
        for y in (0..n).rev() {
            for x in 0..n {
                out.extend_from_slice(&px[y * n + x].to_le_bytes());
            }
        }
        out.resize(out.len() + n * mask_stride(n), 0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u32], n: usize, x: usize, y: usize) -> u32 {
        px[y * n + x]
    }

    fn u16_at(b: &[u8], o: usize) -> u16 {
        u16::from_le_bytes([b[o], b[o + 1]])
    }
    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
    }

    #[test]
    fn the_ico_header_declares_an_icon_with_one_entry_per_size() {
        let ico = encode_ico(&ICON_SIZES);
        assert_eq!(u16_at(&ico, 0), 0, "reserved");
        assert_eq!(u16_at(&ico, 2), 1, "type 1 = icon");
        assert_eq!(u16_at(&ico, 4) as usize, ICON_SIZES.len());
    }

    #[test]
    fn every_directory_entry_points_inside_the_file() {
        // A wrong offset or length yields an icon Windows silently refuses to
        // load, which looks exactly like "the icon didn't work" with no error.
        let ico = encode_ico(&ICON_SIZES);
        for (i, size) in ICON_SIZES.iter().enumerate() {
            let e = 6 + i * 16;
            let declared = ico[e] as usize;
            assert_eq!(
                declared,
                if *size == 256 { 0 } else { *size },
                "256 is encoded as 0; {size} was encoded as {declared}"
            );
            assert_eq!(u16_at(&ico, e + 4), 1, "planes");
            assert_eq!(u16_at(&ico, e + 6), 32, "bit count");
            let len = u32_at(&ico, e + 8) as usize;
            let off = u32_at(&ico, e + 12) as usize;
            assert!(
                off + len <= ico.len(),
                "entry {i} runs past the end of the file"
            );
            assert_eq!(
                u32_at(&ico, off),
                40,
                "each image starts with a 40-byte header"
            );
            assert_eq!(u32_at(&ico, off + 4) as usize, *size, "biWidth");
            assert_eq!(
                u32_at(&ico, off + 8) as usize,
                size * 2,
                "biHeight is doubled: colour bitmap plus AND mask"
            );
        }
    }

    #[test]
    fn the_committed_icon_matches_what_the_code_generates() {
        // The one that actually catches a forgotten regeneration.
        let committed = include_bytes!("../../assets/headset.ico");
        assert_eq!(
            committed.as_slice(),
            encode_ico(&ICON_SIZES).as_slice(),
            "assets/headset.ico is stale; regenerate it with \
             `cargo run -p headset-tray -- --export-icon crates/headset-tray/assets/headset.ico`"
        );
    }

    #[test]
    fn every_pixel_is_fill_outline_or_clear() {
        // The ICO encoder relies on there being no partial alpha.
        for n in [16, 32, 48, 128] {
            for p in icon_pixels(n) {
                assert!(
                    p == FILL || p == OUTLINE || p == CLEAR,
                    "size {n} produced an unexpected colour {p:#010x}"
                );
            }
        }
    }

    #[test]
    fn the_shape_is_symmetric_about_the_vertical_axis() {
        // Two ear cups and a centred arc: a left/right asymmetry means a
        // constant was scaled wrongly.
        for n in [16, 32, 48, 128] {
            let px = icon_pixels(n);
            for y in 0..n {
                for x in 0..n {
                    assert_eq!(
                        at(&px, n, x, y),
                        at(&px, n, n - 1 - x, y),
                        "size {n} differs at ({x},{y}) and its mirror"
                    );
                }
            }
        }
    }

    #[test]
    fn the_icon_is_neither_empty_nor_solid_at_every_size() {
        for n in [16, 32, 48, 128, 256] {
            let px = icon_pixels(n);
            assert_eq!(px.len(), n * n, "size {n} produced the wrong pixel count");
            // Measured on FILL, not on everything non-clear. The outline is one
            // pixel wide at every size, so at 16 it is proportionally huge and
            // more than half the canvas is non-clear while the shape itself is
            // still a thin headset.
            let fill = px.iter().filter(|p| **p == FILL).count();
            assert!(fill > n * n / 50, "size {n} drew almost nothing: {fill}px");
            assert!(
                fill < n * n / 2,
                "size {n} drew almost everything: {fill}px"
            );
        }
    }

    #[test]
    fn the_thirty_two_pixel_rendering_is_pinned() {
        // This is the icon that has been shipping. Changing the drawing must
        // fail here, and the fix is to re-run `--export-icon` and commit the
        // regenerated file along with an updated expectation.
        let px = icon_pixels(32);
        let drawn = px.iter().filter(|p| **p != CLEAR).count();
        let fill = px.iter().filter(|p| **p == FILL).count();
        assert_eq!((drawn, fill), (PINNED_DRAWN, PINNED_FILL));

        // The ear cups sit at the sides, the arc across the top, and the middle
        // of the band area is hollow.
        assert_eq!(at(&px, 32, 5, 20), FILL, "left ear cup");
        assert_eq!(at(&px, 32, 26, 20), FILL, "right ear cup");
        assert_eq!(at(&px, 32, 16, 7), FILL, "top of the headband arc");
        assert_eq!(at(&px, 32, 16, 20), CLEAR, "the middle is open");
    }

    /// Filled in from the first run. See the test above.
    const PINNED_DRAWN: usize = 370;
    const PINNED_FILL: usize = 228;
}
