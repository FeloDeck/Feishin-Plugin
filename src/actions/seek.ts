import streamDeck, {
	action,
	type DialAction,
	type DialDownEvent,
	type DialRotateEvent,
	type KeyAction,
	type TouchTapEvent,
	type WillDisappearEvent,
} from "@elgato/streamdeck";

import { feishin, type StateChange } from "../feishin";
import { svgImage } from "../images";
import { FeishinAction, numberSetting, RepeatingKeyAction } from "./feishin-action";

type SeekSettings = {
	/** Seconds to jump per key press. */
	seconds?: string;
};

type SeekDialSettings = {
	/** Seconds to jump per dial tick. */
	tickSeconds?: string;
	/** Show the album art on the dial screen (default: on). */
	showCover?: boolean;
};

const LAYOUT_COVER = "layouts/seek-dial-cover.json";
const LAYOUT_PLAIN = "layouts/seek-dial.json";
const NO_COVER = "imgs/actions/seek/no-cover.svg";

@action({ UUID: "de.felitendo.feishin.forward" })
export class FastForward extends RepeatingKeyAction<SeekSettings> {
	protected override press(settings: SeekSettings): boolean {
		return feishin.seekBy(numberSetting(settings.seconds, 10));
	}
}

@action({ UUID: "de.felitendo.feishin.rewind" })
export class Rewind extends RepeatingKeyAction<SeekSettings> {
	protected override press(settings: SeekSettings): boolean {
		return feishin.seekBy(-numberSetting(settings.seconds, 10));
	}
}

/**
 * Stream Deck + dial showing the current track: rotate to seek, press or tap the touch strip to play / pause.
 */
@action({ UUID: "de.felitendo.feishin.seekdial" })
export class SeekDial extends FeishinAction<SeekDialSettings> {
	protected override readonly watches: readonly StateChange[] = ["song", "status", "position", "cover"];

	/**
	 * Last feedback sent per dial; position updates arrive several times a second, but the display only changes once
	 * per second.
	 */
	readonly #sent = new Map<string, string>();

	/** Layout last set per dial. */
	readonly #layout = new Map<string, string>();

	/** Cover last sent per dial. */
	readonly #cover = new Map<string, string>();

	override onDialRotate(ev: DialRotateEvent<SeekDialSettings>): void {
		this.run(ev.action, () => feishin.seekBy(ev.payload.ticks * numberSetting(ev.payload.settings.tickSeconds, 5)));
	}

	override onDialDown(ev: DialDownEvent<SeekDialSettings>): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	override onTouchTap(ev: TouchTapEvent<SeekDialSettings>): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	override onWillDisappear(ev: WillDisappearEvent<SeekDialSettings>): Promise<void> | void {
		this.#sent.delete(ev.action.id);
		this.#layout.delete(ev.action.id);
		this.#cover.delete(ev.action.id);
		return super.onWillDisappear(ev);
	}

	protected override async render(action: DialAction | KeyAction, settings: SeekDialSettings): Promise<void> {
		if (!action.isDial()) {
			return;
		}

		const withCover = settings.showCover !== false;
		const layout = withCover ? LAYOUT_COVER : LAYOUT_PLAIN;
		if (this.#layout.get(action.id) !== layout) {
			this.#layout.set(action.id, layout);
			this.#sent.delete(action.id);
			this.#cover.delete(action.id);
			await action.setFeedbackLayout(layout);
		}

		const song = feishin.state.song;
		const empty = { artist: "", time: "", status: { enabled: false }, progress: { value: 0 } };
		let feedback;
		if (!feishin.isConnected) {
			feedback = { ...empty, track: streamDeck.i18n.translate("offline") };
		} else if (!song) {
			feedback = { ...empty, track: streamDeck.i18n.translate("nothing-playing") };
		} else {
			const position = feishin.position;
			feedback = {
				track: song.name,
				artist: song.artist,
				time: `${formatTime(position)} / ${formatTime(song.duration)}`,
				status: {
					enabled: true,
					value: `imgs/actions/seek/${feishin.state.status === "playing" ? "playing" : "paused"}.svg`,
				},
				// Whole percent keeps updates to roughly one per second, together with the time.
				progress: { value: song.duration > 0 ? Math.round((position / song.duration) * 100) : 0 },
			};
		}

		// The cover is big, so it only goes out when it changed, not with every time update.
		if (withCover) {
			const cover = (feishin.isConnected && song ? feishin.cover() : undefined) ?? svgImage(NO_COVER);
			if (this.#cover.get(action.id) !== cover) {
				this.#cover.set(action.id, cover);
				await action.setFeedback({ cover });
			}
		}

		const json = JSON.stringify(feedback);
		if (this.#sent.get(action.id) !== json) {
			this.#sent.set(action.id, json);
			await action.setFeedback(feedback);
		}
	}
}

export function formatTime(seconds: number): string {
	const total = Math.max(0, Math.floor(seconds));
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = String(total % 60).padStart(2, "0");
	return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
