import {
	action,
	type DialAction,
	type KeyAction,
	type KeyDownEvent,
	type WillAppearEvent,
	type WillDisappearEvent,
} from "@elgato/streamdeck";

import { feishin, type StateChange } from "../feishin";
import { svgImage, svgWithCover, svgWithTitle } from "../images";
import { FeishinAction } from "./feishin-action";
import { formatTime } from "./seek";

type PlayPauseSettings = {
	/** Show the cover of the song instead of the icon (default: off). */
	showCover?: boolean;
	/** Show the time of the track on the key (default: on). */
	showTime?: boolean;
	/** Where the time goes (default: below the icon). */
	timeAt?: "below" | "above";
	/** Time left (default) or time played. */
	timeShows?: "left" | "played";
};

/** Icon per state: play while paused, pause while playing. */
const PLAY_PAUSE_ICONS = ["imgs/actions/play-pause/play.svg", "imgs/actions/play-pause/pause.svg"];
/** How tall the play triangle is with its outline, in units of its viewBox. Pause uses it too, so the time stays put. */
const PLAY_HEIGHT = 15.5;

/**
 * Plays or pauses; state 0 shows "play" (paused), state 1 shows "pause" (playing). Shows the time of the track under or
 * above the icon (unless disabled), drawn into the image like the volume keys do. Or the cover of the song: dark with
 * the play icon while paused, and the time at the bottom or top edge.
 */
@action({ UUID: "de.felitendo.feishin.playpause" })
export class PlayPause extends FeishinAction<PlayPauseSettings> {
	protected override readonly watches: readonly StateChange[] = ["status", "song", "position", "cover"];

	/**
	 * State and title last sent per key; position updates arrive several times a second, but the time only changes once
	 * per second.
	 */
	readonly #sent = new Map<string, { state: 0 | 1; title: string | undefined }>();

	override onKeyDown(ev: KeyDownEvent<PlayPauseSettings>): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	override onWillAppear(ev: WillAppearEvent<PlayPauseSettings>): Promise<void> | void {
		this.#sent.delete(ev.action.id);
		return super.onWillAppear(ev);
	}

	override onWillDisappear(ev: WillDisappearEvent<PlayPauseSettings>): Promise<void> | void {
		this.#sent.delete(ev.action.id);
		return super.onWillDisappear(ev);
	}

	protected override async render(action: DialAction | KeyAction, settings: PlayPauseSettings): Promise<void> {
		if (!action.isKey()) {
			return;
		}

		const time = settings.showTime === false ? "" : trackTime(settings.timeShows === "played");
		const above = settings.timeAt === "above";
		const cover = settings.showCover ? feishin.cover() : undefined;
		for (const [state, icon] of PLAY_PAUSE_ICONS.entries()) {
			const image = cover
				? svgWithCover(cover, state === 0 ? icon : "", time, above)
				: time
					? svgWithTitle(icon, time, PLAY_HEIGHT, above)
					: undefined;
			await this.setStateImage(action, state as 0 | 1, image);
		}

		// The key draws the time itself, so the title stays empty meanwhile.
		const next = { state: feishin.state.status === "playing" ? 1 : 0, title: time ? "" : undefined } as const;
		const sent = this.#sent.get(action.id);
		this.#sent.set(action.id, next);
		if (sent?.state !== next.state) {
			await action.setState(next.state);
		}
		if (!sent || sent.title !== next.title) {
			await action.setTitle(next.title);
		}
	}
}

/**
 * The time left ("-2:31") or played ("1:05") of the current track, or "" while nothing is playing or paused.
 */
function trackTime(played: boolean): string {
	const { song, status } = feishin.state;
	if (!feishin.isConnected || !song || status === "stopped") {
		return "";
	}

	const position = Math.floor(feishin.position);
	if (played) {
		return formatTime(position);
	}

	return song.duration > 0 ? `-${formatTime(song.duration - position)}` : "";
}

@action({ UUID: "de.felitendo.feishin.next" })
export class Next extends FeishinAction {
	override onKeyDown(ev: KeyDownEvent): void {
		this.run(ev.action, () => feishin.next());
	}
}

@action({ UUID: "de.felitendo.feishin.previous" })
export class Previous extends FeishinAction {
	override onKeyDown(ev: KeyDownEvent): void {
		this.run(ev.action, () => feishin.previous());
	}
}

/**
 * Toggles shuffle; state 0 = off, state 1 = on.
 */
@action({ UUID: "de.felitendo.feishin.shuffle" })
export class Shuffle extends FeishinAction {
	protected override readonly watches: readonly StateChange[] = ["shuffle"];

	override onKeyDown(ev: KeyDownEvent): void {
		this.run(ev.action, () => feishin.toggleShuffle());
	}

	protected override async render(action: DialAction | KeyAction): Promise<void> {
		if (action.isKey()) {
			await action.setState(feishin.state.shuffle ? 1 : 0);
		}
	}
}

/**
 * Cycles through the repeat modes (off → all → one); keys only support two states, so the image is swapped instead.
 */
@action({ UUID: "de.felitendo.feishin.repeat" })
export class Repeat extends FeishinAction {
	protected override readonly watches: readonly StateChange[] = ["repeat"];

	override onKeyDown(ev: KeyDownEvent): void {
		this.run(ev.action, () => feishin.toggleRepeat());
	}

	protected override async render(action: DialAction | KeyAction): Promise<void> {
		if (action.isKey()) {
			await action.setImage(svgImage(`imgs/actions/repeat/${feishin.state.repeat}.svg`));
		}
	}
}
