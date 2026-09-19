import { action, type DialAction, type KeyAction, type KeyDownEvent } from "@elgato/streamdeck";

import { feishin, type StateChange } from "../feishin";
import { svgImage } from "../images";
import { FeishinAction } from "./feishin-action";

/**
 * Plays or pauses; state 0 shows "play" (paused), state 1 shows "pause" (playing).
 */
@action({ UUID: "de.felitendo.feishin.playpause" })
export class PlayPause extends FeishinAction {
	protected override readonly watches: readonly StateChange[] = ["status"];

	override onKeyDown(ev: KeyDownEvent): void {
		this.run(ev.action, () => feishin.togglePlayPause());
	}

	protected override async render(action: DialAction | KeyAction): Promise<void> {
		if (action.isKey()) {
			await action.setState(feishin.state.status === "playing" ? 1 : 0);
		}
	}
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
