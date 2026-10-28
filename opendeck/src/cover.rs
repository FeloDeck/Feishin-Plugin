// The cover for a key. Feishin sends it as big as it is on the music server, often 1000 px and more,
// but the key sends it again every second (with the time on it). So it gets cut to a square and
// shrunk to the key, as JPEG. A picture that cannot be read (like WebP) stays as it is.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::ExtendedColorType;
use image::codecs::jpeg::JpegEncoder;

/// Keys are drawn on 144 x 144.
const SIZE: usize = 144;
const QUALITY: u8 = 90;

/// The cover as a data URL, from the picture Feishin sent (base64).
pub fn shrink_cover(base64: &str) -> Option<String> {
	let data = STANDARD.decode(base64).ok()?;
	let kind = image_type(&data)?;
	if let Some(jpeg) = shrunk(&data) {
		return Some(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)));
	}
	Some(format!("data:{kind};base64,{base64}"))
}

fn shrunk(data: &[u8]) -> Option<Vec<u8>> {
	let image = image::load_from_memory(data).ok()?.to_rgba8();
	let small = shrink(image.as_raw(), image.width() as usize, image.height() as usize);
	let rgb: Vec<u8> = small.chunks(4).flat_map(|pixel| &pixel[..3]).copied().collect();
	let mut jpeg = Vec::new();
	JpegEncoder::new_with_quality(&mut jpeg, QUALITY).encode(&rgb, SIZE as u32, SIZE as u32, ExtendedColorType::Rgb8).ok()?;
	Some(jpeg)
}

/// The middle square of an RGBA picture at SIZE x SIZE: each pixel is the average of the pixels it
/// covers. Transparent parts turn black. src/cover.ts of the Stream Deck app version does the same.
fn shrink(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
	let side = width.min(height);
	let (left, top) = ((width - side) / 2, (height - side) / 2);
	let mut out = vec![0; SIZE * SIZE * 4];
	for y in 0..SIZE {
		let y0 = top + y * side / SIZE;
		let y1 = (y0 + 1).max(top + (y + 1) * side / SIZE);
		for x in 0..SIZE {
			let x0 = left + x * side / SIZE;
			let x1 = (x0 + 1).max(left + (x + 1) * side / SIZE);
			let mut sum = [0u64; 3];
			for sy in y0..y1 {
				for sx in x0..x1 {
					let i = (sy * width + sx) * 4;
					for (c, sum) in sum.iter_mut().enumerate() {
						*sum += u64::from(rgba[i + c]) * u64::from(rgba[i + 3]);
					}
				}
			}
			// Rounded: (sum + n / 2) / n, in whole numbers.
			let n = ((x1 - x0) * (y1 - y0) * 255) as u64;
			let o = (y * SIZE + x) * 4;
			for (c, sum) in sum.iter().enumerate() {
				out[o + c] = ((2 * sum + n) / (2 * n)) as u8;
			}
			out[o + 3] = 255;
		}
	}
	out
}

/// The type of a picture by its first bytes.
fn image_type(data: &[u8]) -> Option<&'static str> {
	if data.starts_with(&[0xff, 0xd8, 0xff]) {
		Some("image/jpeg")
	} else if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
		Some("image/png")
	} else if data.starts_with(b"GIF8") {
		Some("image/gif")
	} else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
		Some("image/webp")
	} else {
		None
	}
}

#[cfg(test)]
pub(crate) mod testing {
	use image::{ImageFormat, Rgba, RgbaImage};

	use super::*;

	/// A PNG as Feishin would send it: left half red, right half blue.
	pub fn png(width: u32, height: u32) -> String {
		let image = RgbaImage::from_fn(width, height, |x, _| if x < width / 2 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) });
		let mut png = std::io::Cursor::new(Vec::new());
		image.write_to(&mut png, ImageFormat::Png).unwrap_or_default();
		STANDARD.encode(png.into_inner())
	}
}

#[cfg(test)]
mod tests {
	use super::testing::png;
	use super::*;

	#[test]
	fn a_big_cover_gets_small() {
		let cover = shrink_cover(&png(1200, 900)).unwrap_or_default();
		let jpeg = STANDARD.decode(cover.strip_prefix("data:image/jpeg;base64,").unwrap_or_default()).unwrap_or_default();
		let image = image::load_from_memory(&jpeg).map(|i| i.to_rgb8()).unwrap_or_default();
		assert_eq!(image.dimensions(), (144, 144));
		// The middle square: still half red, half blue.
		let [r, _, b] = image.get_pixel(30, 72).0;
		assert!(r > 200 && b < 50, "{r} {b}");
		let [r, _, b] = image.get_pixel(114, 72).0;
		assert!(r < 50 && b > 200, "{r} {b}");
	}

	#[test]
	fn shrinking_averages_and_turns_transparent_black() {
		// 288 x 288: each pixel of the key covers 2 x 2. Rows: white, black, half transparent white.
		let width = 288;
		let mut rgba = Vec::new();
		for y in 0..width {
			for _ in 0..width {
				rgba.extend(match y % 3 {
					0 => [255, 255, 255, 255],
					1 => [0, 0, 0, 255],
					_ => [255, 255, 255, 128],
				});
			}
		}
		let small = shrink(&rgba, width, width);
		assert_eq!(small.len(), 144 * 144 * 4);
		// Rows 0 and 1: (255 + 0) / 2. Rows 2 and 3: (128 + 255) / 2.
		assert_eq!(&small[..4], [128, 128, 128, 255]);
		assert_eq!(&small[144 * 4..144 * 4 + 4], [192, 192, 192, 255]);
	}

	#[test]
	fn a_small_cover_grows() {
		let small = shrink(&[10, 20, 30, 255], 1, 1);
		assert!(small.chunks(4).all(|p| p == [10, 20, 30, 255]));
	}

	#[test]
	fn other_pictures_stay_as_they_are() {
		let webp = STANDARD.encode(b"RIFF0000WEBPVP8 ");
		assert_eq!(shrink_cover(&webp), Some(format!("data:image/webp;base64,{webp}")));
		assert_eq!(shrink_cover(&STANDARD.encode(b"hello")), None);
		assert_eq!(shrink_cover("no base64!"), None);
	}
}
