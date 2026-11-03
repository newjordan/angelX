//! Shared, off-frame normalization for character and effect sprite frames.
use angel_live::Asset;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Frame {
    Pixels {
        pixels: Vec<[u8; 4]>,
    },
    Asset {
        asset: String,
    },
    Indexed {
        rows: Vec<String>,
        palette: BTreeMap<char, [u8; 4]>,
    },
}
pub fn prepare(
    input: Vec<Frame>,
    assets: &BTreeMap<String, Arc<Asset>>,
) -> Result<(Vec<Vec<[u8; 4]>>, Vec<Arc<Asset>>), String> {
    if input.len() > 8 {
        return Err("at most eight sprite frames are supported".into());
    }
    let mut held = Vec::new();
    let mut frames = Vec::new();
    for frame in input {
        let pixels = match frame {
            Frame::Pixels { pixels } => pixels,
            Frame::Indexed { rows, palette } => {
                let width = rows.first().map_or(0, String::len);
                if !(1..=32).contains(&width)
                    || !(1..=32).contains(&rows.len())
                    || palette.len() > 64
                    || palette.keys().any(|c| !c.is_ascii_graphic() || *c == '.')
                    || rows.iter().any(|row| {
                        row.len() != width
                            || !row.is_ascii()
                            || row.chars().any(|c| c != '.' && !palette.contains_key(&c))
                    })
                {
                    return Err("indexed sprite requires a rectangular 1..32 grid and declared ASCII colours; '.' is transparent".into());
                }
                (0..1024)
                    .map(|pixel| {
                        let c = rows[(pixel / 32) * rows.len() / 32].as_bytes()
                            [(pixel % 32) * width / 32] as char;
                        palette.get(&c).copied().unwrap_or([0; 4])
                    })
                    .collect()
            }
            Frame::Asset { asset } => {
                let asset = assets
                    .values()
                    .find(|value| value.id == asset)
                    .ok_or("sprite asset is not captured in this generation")?;
                let reader = image::ImageReader::new(Cursor::new(&asset.bytes))
                    .with_guessed_format()
                    .map_err(|e| e.to_string())?;
                let (width, height) = reader.into_dimensions().map_err(|e| e.to_string())?;
                if (width, height) != (32, 32) {
                    return Err(
                        "sprite frames must be 32x32; normalize art before publishing".into(),
                    );
                }
                let image = image::load_from_memory(&asset.bytes)
                    .map_err(|e| e.to_string())?
                    .to_rgba8();
                held.push(Arc::clone(asset));
                image.pixels().map(|p| p.0).collect()
            }
        };
        if pixels.len() != 32 * 32 || !pixels.iter().any(|p| p[3] > 0) {
            return Err("sprite frame must contain 1024 RGBA pixels with a visible figure".into());
        }
        frames.push(pixels);
    }
    Ok((frames, held))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_frames_prepare_to_the_same_rgba_contract_without_asset_io() {
        let frame = Frame::Indexed {
            rows: vec!["a.".into(), ".a".into()],
            palette: BTreeMap::from([('a', [10, 20, 30, 255])]),
        };
        let (frames, assets) = prepare(vec![frame], &BTreeMap::new()).unwrap();
        assert!(assets.is_empty());
        assert_eq!(frames[0].len(), 1024);
        assert_eq!(frames[0][0], [10, 20, 30, 255]);
        assert_eq!(frames[0][31], [0; 4]);
        assert_eq!(frames[0][1023], [10, 20, 30, 255]);
    }
    #[test]
    fn malformed_or_invisible_indexed_art_is_rejected_before_publication() {
        for (rows, palette) in [
            (vec!["ab".into()], BTreeMap::from([('a', [255; 4])])),
            (
                vec!["aa".into(), "a".into()],
                BTreeMap::from([('a', [255; 4])]),
            ),
            (vec!["..".into()], BTreeMap::new()),
            (vec!["a".into()], BTreeMap::from([('a', [0; 4])])),
        ] {
            assert!(prepare(vec![Frame::Indexed { rows, palette }], &BTreeMap::new()).is_err());
        }
    }
}
