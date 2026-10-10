import { readFileSync } from "node:fs";

const images = new Map<string, string>();

/**
 * Reads an SVG from the plugin folder as a data URL for `setImage`. Apps resolve file paths passed to `setImage`
 * differently (OpenDeck always appends an extension, so "none.svg" becomes "none.svg.svg"), whereas data URLs work
 * everywhere.
 * @param path Path relative to the .sdPlugin folder, which is the plugin's working directory.
 */
export function svgImage(path: string): string {
	let image = images.get(path);
	if (image === undefined) {
		image = `data:image/svg+xml;base64,${readFileSync(path).toString("base64")}`;
		images.set(path, image);
	}

	return image;
}

/** Keys are drawn on 144 x 144. */
const KEY_SIZE = 144;
/** The font of the titles OpenDeck draws (font size 11 is about 22 px on a key), but bold to read it on the deck. */
const TITLE_FONT = "'Liberation Sans',Arial,sans-serif";
const TITLE_SIZE = 22;
/** How tall digits are in that size, for placing the line. */
const TITLE_INK = 16;
/** Space between icon and text. */
const TITLE_GAP = 16;

const parts = new Map<string, { viewBox: number[]; body: string }>();

/**
 * An SVG from the plugin folder with a line of text under or above it, both together in the middle of the key. Keys
 * draw the text themselves: OpenDeck puts its title at the bottom edge, far from the icon.
 * @param path Path relative to the .sdPlugin folder. The icon must be in the middle of its square viewBox.
 * @param height How tall the icon is, in units of the viewBox.
 */
export function svgWithTitle(path: string, title: string, height: number, above = false): string {
	let part = parts.get(path);
	if (part === undefined) {
		const svg = readFileSync(path, "utf8");
		const viewBox = (/viewBox="([^"]+)"/.exec(svg)?.[1] ?? `0 0 ${KEY_SIZE} ${KEY_SIZE}`).trim().split(/[\s,]+/);
		part = { viewBox: viewBox.map(Number), body: /<svg\b[^>]*>([\s\S]*)<\/svg>/.exec(svg)?.[1] ?? "" };
		parts.set(path, part);
	}

	const [x = 0, y = 0, width = KEY_SIZE] = part.viewBox;
	const scale = KEY_SIZE / width;
	// The icon moves away from the text by half of what the text needs, so both are centered together.
	const shift = (TITLE_GAP + TITLE_INK) / 2;
	const ink = height * scale;
	const baseline = above ? (KEY_SIZE - ink) / 2 + shift - TITLE_GAP : (KEY_SIZE + ink) / 2 + shift;
	const text = title.replace(/&/g, "&amp;").replace(/</g, "&lt;");
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" width="${KEY_SIZE}" height="${KEY_SIZE}" viewBox="0 0 ${KEY_SIZE} ${KEY_SIZE}">` +
		`<g transform="translate(${-x * scale} ${-y * scale + (above ? shift : -shift)}) scale(${scale})">${part.body}</g>` +
		`<text x="${KEY_SIZE / 2}" y="${Math.round(baseline * 10) / 10}" font-family="${TITLE_FONT}" font-size="${TITLE_SIZE}" font-weight="700" fill="#FFFFFF" text-anchor="middle">${text}</text></svg>`;
	return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}
