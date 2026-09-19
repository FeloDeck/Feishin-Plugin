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
