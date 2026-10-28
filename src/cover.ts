import jpeg from "jpeg-js";
import { PNG } from "pngjs";

/** Keys are drawn on 144 x 144. */
const SIZE = 144;
const QUALITY = 90;

/**
 * The cover for a key, as a data URL. Feishin sends it as big as it is on the music server, often 1000 px and more, but
 * the key sends it again every second (with the time on it). So it gets cut to a square and shrunk to the key, as JPEG.
 * A picture that can't be read (like WebP) stays as it is.
 * @param base64 The picture Feishin sent.
 */
export function shrinkCover(base64: string): string | undefined {
	const data = Buffer.from(base64, "base64");
	const type = imageType(data);
	try {
		const image =
			type === "image/jpeg"
				? jpeg.decode(data, { useTArray: true, formatAsRGBA: true })
				: type === "image/png"
					? PNG.sync.read(data)
					: undefined;
		if (image) {
			const small = { width: SIZE, height: SIZE, data: shrink(image.data, image.width, image.height) };
			return `data:image/jpeg;base64,${jpeg.encode(small, QUALITY).data.toString("base64")}`;
		}
	} catch {
		// Broken, or a kind of JPEG or PNG the decoders don't know.
	}

	return type ? `data:${type};base64,${base64}` : undefined;
}

/**
 * The middle square of an RGBA picture at {@link SIZE} x {@link SIZE}: each pixel is the average of the pixels it
 * covers. Transparent parts turn black. src/cover.rs of the OpenDeck version does the same.
 */
function shrink(rgba: Uint8Array, width: number, height: number): Uint8Array {
	const side = Math.min(width, height);
	const left = Math.floor((width - side) / 2);
	const top = Math.floor((height - side) / 2);
	const out = new Uint8Array(SIZE * SIZE * 4);
	for (let y = 0; y < SIZE; y++) {
		const y0 = top + Math.floor((y * side) / SIZE);
		const y1 = Math.max(y0 + 1, top + Math.floor(((y + 1) * side) / SIZE));
		for (let x = 0; x < SIZE; x++) {
			const x0 = left + Math.floor((x * side) / SIZE);
			const x1 = Math.max(x0 + 1, left + Math.floor(((x + 1) * side) / SIZE));
			const sum = [0, 0, 0];
			for (let sy = y0; sy < y1; sy++) {
				for (let sx = x0; sx < x1; sx++) {
					const i = (sy * width + sx) * 4;
					for (let c = 0; c < 3; c++) {
						sum[c] += rgba[i + c] * rgba[i + 3];
					}
				}
			}

			// Rounded: (sum + n / 2) / n, in whole numbers.
			const n = (x1 - x0) * (y1 - y0) * 255;
			const o = (y * SIZE + x) * 4;
			for (let c = 0; c < 3; c++) {
				out[o + c] = Math.floor((2 * sum[c] + n) / (2 * n));
			}
			out[o + 3] = 255;
		}
	}

	return out;
}

/**
 * The type of a picture by its first bytes.
 */
function imageType(data: Buffer): string | undefined {
	if (data.subarray(0, 3).equals(Buffer.from([0xff, 0xd8, 0xff]))) {
		return "image/jpeg";
	}
	if (data.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))) {
		return "image/png";
	}
	if (data.subarray(0, 4).toString("latin1") === "GIF8") {
		return "image/gif";
	}
	if (data.subarray(0, 4).toString("latin1") === "RIFF" && data.subarray(8, 12).toString("latin1") === "WEBP") {
		return "image/webp";
	}
	return undefined;
}
