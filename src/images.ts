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
 * An SVG from the plugin folder as a group that fills the key.
 * @param path Path relative to the .sdPlugin folder. The icon must be in the middle of its square viewBox.
 * @param dy How far to move it down.
 * @returns The group, and how much the icon was scaled.
 */
function iconGroup(path: string, dy: number): { group: string; scale: number } {
	let part = parts.get(path);
	if (part === undefined) {
		const svg = readFileSync(path, "utf8");
		const viewBox = (/viewBox="([^"]+)"/.exec(svg)?.[1] ?? `0 0 ${KEY_SIZE} ${KEY_SIZE}`).trim().split(/[\s,]+/);
		part = { viewBox: viewBox.map(Number), body: /<svg\b[^>]*>([\s\S]*)<\/svg>/.exec(svg)?.[1] ?? "" };
		parts.set(path, part);
	}

	const [x = 0, y = 0, width = KEY_SIZE] = part.viewBox;
	const scale = KEY_SIZE / width;
	return { group: `<g transform="translate(${-x * scale} ${-y * scale + dy}) scale(${scale})">${part.body}</g>`, scale };
}

function titleText(title: string, baseline: number): string {
	const text = title.replace(/&/g, "&amp;").replace(/</g, "&lt;");
	return `<text x="${KEY_SIZE / 2}" y="${Math.round(baseline * 10) / 10}" font-family="${TITLE_FONT}" font-size="${TITLE_SIZE}" font-weight="700" fill="#FFFFFF" text-anchor="middle">${text}</text>`;
}

/**
 * An SVG from the plugin folder with a line of text under or above it, both together in the middle of the key. Keys
 * draw the text themselves: OpenDeck puts its title at the bottom edge, far from the icon.
 * @param path Path relative to the .sdPlugin folder. The icon must be in the middle of its square viewBox.
 * @param height How tall the icon is, in units of the viewBox.
 */
export function svgWithTitle(path: string, title: string, height: number, above = false): string {
	// The icon moves away from the text by half of what the text needs, so both are centered together.
	const shift = (TITLE_GAP + TITLE_INK) / 2;
	const { group, scale } = iconGroup(path, above ? shift : -shift);
	const ink = height * scale;
	const baseline = above ? (KEY_SIZE - ink) / 2 + shift - TITLE_GAP : (KEY_SIZE + ink) / 2 + shift;
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" width="${KEY_SIZE}" height="${KEY_SIZE}" viewBox="0 0 ${KEY_SIZE} ${KEY_SIZE}">` +
		`${group}${titleText(title, baseline)}</svg>`;
	return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}

/** Space between the edge of the key and the text on a cover. */
const COVER_MARGIN = 14;
/** How far the shade behind the text on a cover reaches into the key. */
const SHADE = 60;

/**
 * A cover that fills the key, darker with an icon in the middle (like the play icon while paused), and with a line of
 * text at the bottom or top edge on a shade.
 * @param cover The cover as a data URL.
 * @param icon Path of the icon relative to the .sdPlugin folder, or "" for none.
 * @param title Text on the key, "" for none.
 */
export function svgWithCover(cover: string, icon: string, title: string, above = false): string {
	let svg =
		`<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${KEY_SIZE}" height="${KEY_SIZE}" viewBox="0 0 ${KEY_SIZE} ${KEY_SIZE}">` +
		`<image width="${KEY_SIZE}" height="${KEY_SIZE}" preserveAspectRatio="xMidYMid slice" xlink:href="${cover}"/>`;
	if (icon) {
		svg += `<rect width="${KEY_SIZE}" height="${KEY_SIZE}" fill="#000000" fill-opacity="0.5"/>${iconGroup(icon, 0).group}`;
	}
	if (title) {
		const [from, to] = above ? [0.7, 0] : [0, 0.7];
		svg +=
			`<defs><linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#000000" stop-opacity="${from}"/>` +
			`<stop offset="1" stop-color="#000000" stop-opacity="${to}"/></linearGradient></defs>` +
			`<rect y="${above ? 0 : KEY_SIZE - SHADE}" width="${KEY_SIZE}" height="${SHADE}" fill="url(#shade)"/>` +
			titleText(title, above ? COVER_MARGIN + TITLE_INK : KEY_SIZE - COVER_MARGIN);
	}
	return `data:image/svg+xml;base64,${Buffer.from(`${svg}</svg>`).toString("base64")}`;
}

/** Font, colors and glyphs of the status image in the obs-tools plugin, so both look the same. */
const STATUS_FONT = "'Inter','Noto Sans','DejaVu Sans','Liberation Sans',sans-serif";
const GRAY = "#6B6B6B";
const MUTED = "#9A9AA2";
const AMBER = "#F5A524";
const POWER =
	`<g transform="translate(44 22) scale(2.333)" fill="none" stroke="${GRAY}" stroke-width="2.862" stroke-linecap="round" stroke-linejoin="round">` +
	`<g transform="translate(12 12) scale(0.839) translate(-12 -13.103)"><path d="M12 3v9"/><path d="M18.4 6.6a9 9 0 1 1-12.8 0"/></g></g>`;
const LOCK =
	`<g transform="translate(44 22) scale(2.333)" fill="none" stroke="${AMBER}" stroke-width="2.248" stroke-linecap="round" stroke-linejoin="round">` +
	`<g transform="translate(12 12) scale(1.067) translate(-12 -12.396)"><rect x="5" y="11" width="14" height="10" rx="2" fill="${AMBER}" stroke="none"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></g></g>`;

/**
 * Shown on every key while Feishin can't be reached: a gray power glyph, or an amber lock when Feishin is there but
 * something is wrong (like the password), with a word or two under it.
 */
export function statusImage(label: string, problem: boolean): string {
	// Smaller font for longer text. Half the font size per letter, so "Feishin offline" still fits at 17 px.
	const size = Math.max(12, Math.min(20, Math.floor(132 / (Math.max(1, label.length) * 0.5))));
	const text = label.replace(/&/g, "&amp;").replace(/</g, "&lt;");
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" width="${KEY_SIZE}" height="${KEY_SIZE}" viewBox="0 0 ${KEY_SIZE} ${KEY_SIZE}">${problem ? LOCK : POWER}` +
		`<text x="${KEY_SIZE / 2}" y="112" font-family="${STATUS_FONT}" font-size="${size}" font-weight="700" fill="${problem ? AMBER : MUTED}" text-anchor="middle">${text}</text></svg>`;
	return `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`;
}
