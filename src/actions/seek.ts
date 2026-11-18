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
import { FeishinAction, numberSetting, RepeatingKeyAction } from "./feishin-action";

type SeekSettings = {
	/** Seconds to jump per key press. */
	seconds?: string;
};

type SeekDialSettings = {
	/** Seconds to jump per dial tick. */
	tickSeconds?: string;
	/** What turning the dial does (default: seek). */
	dialTurn?: "seek" | "volume";
	/** Volume percentage points per dial tick. */
	tickStep?: string;
};

/** How long the volume shows on the dial screen after a turn. */
const VOLUME_MS = 1500;

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
 * Stream Deck + dial showing the current track: rotate to seek (or change the volume), press or tap the touch strip to
 * play / pause.
 */
@action({ UUID: "de.felitendo.feishin.seekdial" })
export class SeekDial extends FeishinAction<SeekDialSettings> {
	protected override readonly watches: readonly StateChange[] = ["song", "status", "position", "volume"];

	/** Per dial: until when the volume shows in place of the time, and the timer that switches back. */
	readonly #volumeShown = new Map<string, { until: number; timer: NodeJS.Timeout }>();

	/**
	 * Last feedback sent per dial; position updates arrive several times a second, but the display only changes once
	 * per second.
	 */
	readonly #sent = new Map<string, string>();

	override onDialRotate(ev: DialRotateEvent<SeekDialSettings>): void {
		const settings = ev.payload.settings;
		if (settings.dialTurn !== "volume") {
			this.run(ev.action, () => feishin.seekBy(ev.payload.ticks * numberSetting(settings.tickSeconds, 5)));
			return;
		}

		this.#showVolume(ev.action, settings);
		this.run(ev.action, () => feishin.changeVolume(ev.payload.ticks * numberSetting(settings.tickStep, 2)));
	}

	override onDialDown(ev: DialDownEvent<SeekDialSettings>): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	override onTouchTap(ev: TouchTapEvent<SeekDialSettings>): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	/**
	 * Shows the volume in place of the time for a moment, then the time again.
	 */
	#showVolume(action: DialAction, settings: SeekDialSettings): void {
		const id = action.id;
		clearTimeout(this.#volumeShown.get(id)?.timer);
		const timer = setTimeout(() => {
			this.#volumeShown.delete(id);
			void this.render(action, settings);
		}, VOLUME_MS);
		this.#volumeShown.set(id, { until: Date.now() + VOLUME_MS, timer });
		void this.render(action, settings);
	}

	override onWillDisappear(ev: WillDisappearEvent<SeekDialSettings>): Promise<void> | void {
		this.#sent.delete(ev.action.id);
		clearTimeout(this.#volumeShown.get(ev.action.id)?.timer);
		this.#volumeShown.delete(ev.action.id);
		return super.onWillDisappear(ev);
	}

	protected override async render(action: DialAction | KeyAction, _settings?: SeekDialSettings): Promise<void> {
		if (!action.isDial()) {
			return;
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

		// Right after a turn: the volume in place of the time, and on the bar.
		const shown = this.#volumeShown.get(action.id);
		if (feishin.isConnected && shown !== undefined && shown.until > Date.now()) {
			const volume = feishin.state.volume === null ? null : Math.round(feishin.state.volume);
			const label = streamDeck.i18n.translate("volume");
			feedback = {
				...feedback,
				time: volume === null ? `${label} –` : volume === 0 ? streamDeck.i18n.translate("muted") : `${label} ${volume}%`,
				status: { enabled: true, value: `imgs/actions/volume/${volume === 0 ? "dial-muted" : "dial"}.svg` },
				progress: { value: volume ?? 0 },
			};
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
