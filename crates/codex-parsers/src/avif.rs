//! AVIF image dimensions, read from the container rather than by decoding.
//!
//! The `image` crate gates its AVIF decoder behind the `avif-native` feature,
//! which links system libdav1d at compile time. Enabling that would require
//! libdav1d built for every target this ships to, including two cross-compiled
//! musl targets and Windows MSVC, so it is deliberately not enabled and
//! `image::ImageReader::into_dimensions()` fails on AVIF bytes with "The image
//! format Avif is not supported".
//!
//! Dimensions are declared in the container itself, in the `ispe`
//! (ImageSpatialExtentsProperty) box, so they can be read without any decoder.
//! Nothing else here needs one: page bytes are served to clients untouched, and
//! every current client decodes AVIF natively.
//!
//! Structure being walked, per ISO/IEC 14496-12 and 23008-12:
//!
//! ```text
//! meta
//! ├── pitm            primary item ID
//! └── iprp
//!     ├── ipco        ordered property list, indexed from 1
//!     │   ├── ispe    width/height
//!     │   └── ...
//!     └── ipma        item ID -> property indices
//! ```

/// Header of a box whose `size` field is the real size.
const SHORT_HEADER: usize = 8;
/// Header of a box that carries a 64-bit `largesize` after the type.
const LONG_HEADER: usize = 16;
/// Version and flags on a FullBox, before its payload.
const FULL_BOX_PREFIX: usize = 4;

/// One box: its four-character type and its payload (header excluded).
struct Boxed<'a> {
    box_type: [u8; 4],
    payload: &'a [u8],
}

/// Split `data` into the boxes directly inside it, in order.
///
/// Stops at the first malformed length rather than guessing, so a truncated file
/// yields whatever prefix parsed cleanly instead of an error.
fn boxes(data: &[u8]) -> Vec<Boxed<'_>> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    while offset + SHORT_HEADER <= data.len() {
        let size = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        let box_type = [
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ];

        let (header, size) = match size {
            // `largesize` follows the type.
            1 => {
                if offset + LONG_HEADER > data.len() {
                    break;
                }
                let large = u64::from_be_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                    data[offset + 12],
                    data[offset + 13],
                    data[offset + 14],
                    data[offset + 15],
                ]);
                match usize::try_from(large) {
                    Ok(size) => (LONG_HEADER, size),
                    Err(_) => break,
                }
            }
            // Runs to the end of the enclosing container.
            0 => (SHORT_HEADER, data.len() - offset),
            _ => (SHORT_HEADER, size),
        };

        if size < header || offset + size > data.len() {
            break;
        }

        out.push(Boxed {
            box_type,
            payload: &data[offset + header..offset + size],
        });
        offset += size;
    }

    out
}

/// Payload of the first box of `box_type` directly inside `data`.
fn find_box<'a>(data: &'a [u8], box_type: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data)
        .into_iter()
        .find(|b| &b.box_type == box_type)
        .map(|b| b.payload)
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.get(at)?,
        *data.get(at + 1)?,
        *data.get(at + 2)?,
        *data.get(at + 3)?,
    ]))
}

/// Width and height carried by an `ispe` payload.
fn parse_ispe(payload: &[u8]) -> Option<(u32, u32)> {
    let width = read_u32(payload, FULL_BOX_PREFIX)?;
    let height = read_u32(payload, FULL_BOX_PREFIX + 4)?;
    (width > 0 && height > 0).then_some((width, height))
}

/// The primary item's ID, from `pitm`.
fn primary_item_id(meta: &[u8]) -> Option<u32> {
    let pitm = find_box(meta, b"pitm")?;
    let version = *pitm.first()?;
    if version == 0 {
        read_u16(pitm, FULL_BOX_PREFIX).map(u32::from)
    } else {
        read_u32(pitm, FULL_BOX_PREFIX)
    }
}

/// Property indices associated with `item_id`, from `ipma`.
///
/// Indices are 1-based into `ipco`'s child list. The essential bit is ignored:
/// it says whether a reader must understand the property, not which property it
/// is, and the only property being looked for here is understood.
fn property_indices(iprp: &[u8], item_id: u32) -> Option<Vec<u16>> {
    let ipma = find_box(iprp, b"ipma")?;

    let version = *ipma.first()?;
    let flags = read_u32(ipma, 0)? & 0x00ff_ffff;
    let wide_indices = flags & 1 == 1;

    let entry_count = read_u32(ipma, FULL_BOX_PREFIX)?;
    let mut at = FULL_BOX_PREFIX + 4;

    for _ in 0..entry_count {
        let (entry_id, id_width) = if version < 1 {
            (read_u16(ipma, at)? as u32, 2)
        } else {
            (read_u32(ipma, at)?, 4)
        };
        at += id_width;

        let association_count = *ipma.get(at)? as usize;
        at += 1;

        let mut indices = Vec::with_capacity(association_count);
        for _ in 0..association_count {
            if wide_indices {
                indices.push(read_u16(ipma, at)? & 0x7fff);
                at += 2;
            } else {
                indices.push(u16::from(*ipma.get(at)? & 0x7f));
                at += 1;
            }
        }

        if entry_id == item_id {
            return Some(indices);
        }
    }

    None
}

/// Read the dimensions of an AVIF image without decoding it.
///
/// Resolves the primary item's `ispe` property properly, so a file that carries
/// a thumbnail item alongside the full-size image reports the full-size
/// dimensions. Falls back to the largest `ispe` in the file when the item
/// mapping cannot be resolved, which keeps a slightly unusual but readable file
/// working rather than dropping the page.
///
/// Returns `None` if the bytes are not a parseable AVIF-like container or
/// declare no usable extent.
pub fn get_avif_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let meta = find_box(data, b"meta")?;
    // `meta` is a FullBox: its children start after version and flags.
    let meta_children = meta.get(FULL_BOX_PREFIX..)?;

    let iprp = find_box(meta_children, b"iprp")?;
    let ipco = find_box(iprp, b"ipco")?;
    let properties = boxes(ipco);

    // Preferred path: the ispe the primary item actually points at.
    if let Some(item_id) = primary_item_id(meta_children)
        && let Some(indices) = property_indices(iprp, item_id)
    {
        for index in indices {
            let Some(property) = index
                .checked_sub(1)
                .and_then(|i| properties.get(usize::from(i)))
            else {
                continue;
            };
            if &property.box_type == b"ispe"
                && let Some(extent) = parse_ispe(property.payload)
            {
                return Some(extent);
            }
        }
    }

    // Fallback: the largest declared extent. An auxiliary plane matches the
    // primary image's size and a thumbnail is smaller, so the largest is the
    // right guess when the mapping is unreadable.
    properties
        .iter()
        .filter(|p| &p.box_type == b"ispe")
        .filter_map(|p| parse_ispe(p.payload))
        .max_by_key(|(w, h)| u64::from(*w) * u64::from(*h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Encode a real AVIF with the encoder the workspace already builds.
    fn encode_avif(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Avif)
            .expect("AVIF encoding should be available");
        buf.into_inner()
    }

    #[test]
    fn reads_dimensions_from_a_real_avif() {
        let data = encode_avif(23, 41);
        assert_eq!(get_avif_dimensions(&data), Some((23, 41)));
    }

    #[test]
    fn reads_dimensions_without_a_decoder() {
        // Guards the reason this module exists: if the image crate could decode
        // AVIF, the hand-rolled parser would be redundant. It cannot.
        let data = encode_avif(16, 16);
        let decoded = image::ImageReader::new(Cursor::new(&data))
            .with_guessed_format()
            .unwrap()
            .into_dimensions();
        assert!(
            decoded.is_err(),
            "no AVIF decoder is linked; if this fails, prefer the decoder"
        );
        assert_eq!(get_avif_dimensions(&data), Some((16, 16)));
    }

    #[test]
    fn rejects_non_avif_bytes() {
        assert_eq!(get_avif_dimensions(b"not an image at all"), None);
        assert_eq!(get_avif_dimensions(&[]), None);
    }

    #[test]
    fn rejects_a_truncated_container() {
        let data = encode_avif(16, 16);
        assert_eq!(get_avif_dimensions(&data[..data.len() / 2]), None);
    }

    #[test]
    fn prefers_the_primary_item_over_a_larger_unrelated_extent() {
        // Two ispe properties where the primary item maps to the second, smaller
        // one. The largest-extent fallback would return the wrong answer here,
        // so this pins the pitm/ipma resolution rather than the fallback.
        fn boxed(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
            out.extend_from_slice(box_type);
            out.extend_from_slice(payload);
            out
        }
        fn ispe(w: u32, h: u32) -> Vec<u8> {
            let mut payload = vec![0, 0, 0, 0];
            payload.extend_from_slice(&w.to_be_bytes());
            payload.extend_from_slice(&h.to_be_bytes());
            boxed(b"ispe", &payload)
        }

        let mut ipco = ispe(999, 999);
        ipco.extend_from_slice(&ispe(80, 120));
        let ipco = boxed(b"ipco", &ipco);

        // version 0, flags 0: 16-bit item IDs, 8-bit property indices.
        // One entry: item 2 -> property index 2.
        let ipma = boxed(b"ipma", &[0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 1, 2]);

        let mut iprp = ipco;
        iprp.extend_from_slice(&ipma);
        let iprp = boxed(b"iprp", &iprp);

        // version 0 pitm naming item 2.
        let pitm = boxed(b"pitm", &[0, 0, 0, 0, 0, 2]);

        let mut meta_payload = vec![0, 0, 0, 0];
        meta_payload.extend_from_slice(&pitm);
        meta_payload.extend_from_slice(&iprp);
        let meta = boxed(b"meta", &meta_payload);

        assert_eq!(get_avif_dimensions(&meta), Some((80, 120)));
    }

    #[test]
    fn falls_back_to_the_largest_extent_without_a_usable_mapping() {
        fn boxed(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
            out.extend_from_slice(box_type);
            out.extend_from_slice(payload);
            out
        }
        fn ispe(w: u32, h: u32) -> Vec<u8> {
            let mut payload = vec![0, 0, 0, 0];
            payload.extend_from_slice(&w.to_be_bytes());
            payload.extend_from_slice(&h.to_be_bytes());
            boxed(b"ispe", &payload)
        }

        // No pitm and no ipma, so only the fallback can answer.
        let mut ipco = ispe(32, 32);
        ipco.extend_from_slice(&ispe(400, 600));
        let iprp = boxed(b"iprp", &boxed(b"ipco", &ipco));

        let mut meta_payload = vec![0, 0, 0, 0];
        meta_payload.extend_from_slice(&iprp);
        let meta = boxed(b"meta", &meta_payload);

        assert_eq!(get_avif_dimensions(&meta), Some((400, 600)));
    }
}
