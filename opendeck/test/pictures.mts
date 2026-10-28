// Writes pictures of the Stream Deck app version (src/images.ts) into test/*.txt. The Rust version
// must draw the same (src/images.rs tests). Run it after a change to src/images.ts:
//   node opendeck/test/pictures.mts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { statusImage, svgWithCover, svgWithTitle } from "../../src/images.ts";

const here = fileURLToPath(new URL(".", import.meta.url));
// The paths of the icons are relative to the plugin folder.
process.chdir(fileURLToPath(new URL("../../de.felitendo.feishin.sdPlugin", import.meta.url)));

function write(file: string, images: string[]): void {
	writeFileSync(here + file, images.map((image) => image.replace("data:image/svg+xml;base64,", "") + "\n").join(""));
}

write("svg-with-title.txt", [
	svgWithTitle("imgs/actions/volume/up.svg", "50%", 15.6, false),
	svgWithTitle("imgs/actions/play-pause/pause.svg", "-2:31 <&>", 15.5, true),
	svgWithTitle("imgs/actions/play-pause/play.svg", "1:05", 15.5, false),
	svgWithTitle("imgs/actions/volume/muted.svg", "0%", 15.6, true),
]);
write("svg-with-cover.txt", [
	svgWithCover("data:image/jpeg;base64,AAAA", "imgs/actions/play-pause/play.svg", "-2:31", false),
	svgWithCover("data:image/jpeg;base64,AAAA", "", "1:05 <&>", true),
	svgWithCover("data:image/png;base64,BBBB", "imgs/actions/play-pause/play.svg", "", false),
	svgWithCover("data:image/png;base64,BBBB", "", "", false),
]);
write("status-image.txt", [
	statusImage("Feishin offline", false),
	statusImage("Password?", true),
	statusImage("Feishin-Fehler", true),
	statusImage("Nicht verbunden mit Feishin hier", false),
]);
