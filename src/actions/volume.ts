import streamDeck, {
	action,
	type DialAction,
	type DialDownEvent,
	type DialRotateEvent,
	type KeyAction,
	type KeyDownEvent,
	type TouchTapEvent,
} from "@elgato/streamdeck";

import { feishin, type StateChange } from "../feishin";
import { svgImage } from "../images";
import { FeishinAction, numberSetting, RepeatingKeyAction } from "./feishin-action";

const DEFAULT_STEP = 5;

type VolumeKeySettings = {
	/** Percentage points per press. */
	step?: string;
	/** Show the current volume as the key's title (default: on). */
	showVolume?: boolean;
};

/**
 * Shows the volume as the key title (unless disabled, or the user has set their own title). The icon moves up to make
 * room for it, and stays centered otherwise.
 * @param raised Icon to show above the volume, per state.
 * @param setStateImage {@link FeishinAction.setStateImage} of the action.
 */
async function renderVolume(
	action: KeyAction,
	settings: VolumeKeySettings,
	raised: string[],
	setStateImage: (state: 0 | 1, image: string | undefined) => Promise<void>,
): Promise<void> {
	const { volume } = feishin.state;
	const show = settings.showVolume !== false && feishin.isConnected && volume !== null;
	await action.setTitle(show ? `${Math.round(volume)}%` : undefined);
	for (const [state, image] of raised.entries()) {
		await setStateImage(state as 0 | 1, show ? svgImage(image) : undefined);
	}
}

@action({ UUID: "de.felitendo.feishin.volumeup" })
export class VolumeUp extends RepeatingKeyAction<VolumeKeySettings> {
	protected override readonly watches: readonly StateChange[] = ["volume"];

	protected override press(settings: VolumeKeySettings): boolean {
		// Already at the limit: nothing to do, but no reason to show an alert either.
		if (feishin.isConnected && feishin.state.volume === 100) {
			return true;
		}

		return feishin.changeVolume(numberSetting(settings.step, DEFAULT_STEP));
	}

	protected override async render(action: DialAction | KeyAction, settings: VolumeKeySettings): Promise<void> {
		if (action.isKey()) {
			await renderVolume(action, settings, ["imgs/actions/volume/up-raised.svg"], (state, image) =>
				this.setStateImage(action, state, image),
			);
		}
	}
}

@action({ UUID: "de.felitendo.feishin.volumedown" })
export class VolumeDown extends RepeatingKeyAction<VolumeKeySettings> {
	protected override readonly watches: readonly StateChange[] = ["volume"];

	protected override press(settings: VolumeKeySettings): boolean {
		if (feishin.isConnected && feishin.state.volume === 0) {
			return true;
		}

		return feishin.changeVolume(-numberSetting(settings.step, DEFAULT_STEP));
	}

	protected override async render(action: DialAction | KeyAction, settings: VolumeKeySettings): Promise<void> {
		if (action.isKey()) {
			await renderVolume(action, settings, ["imgs/actions/volume/down-raised.svg"], (state, image) =>
				this.setStateImage(action, state, image),
			);
		}
	}
}

/**
 * Mutes / unmutes; state 0 = sound on, state 1 = muted.
 */
@action({ UUID: "de.felitendo.feishin.mute" })
export class Mute extends FeishinAction<VolumeKeySettings> {
	protected override readonly watches: readonly StateChange[] = ["volume"];

	override onKeyDown(ev: KeyDownEvent<VolumeKeySettings>): void {
		this.run(ev.action, () => feishin.toggleMute());
	}

	protected override async render(action: DialAction | KeyAction, settings: VolumeKeySettings): Promise<void> {
		if (action.isKey()) {
			await action.setState(feishin.isConnected && feishin.state.volume === 0 ? 1 : 0);
			const raised = ["imgs/actions/volume/unmuted-raised.svg", "imgs/actions/volume/muted-raised.svg"];
			await renderVolume(action, settings, raised, (state, image) => this.setStateImage(action, state, image));
		}
	}
}

type VolumeDialSettings = {
	/** Percentage points per dial tick. */
	tickStep?: string;
};

/**
 * Stream Deck + dial: rotate to change the volume, press or tap the touch strip to mute.
 */
@action({ UUID: "de.felitendo.feishin.volumedial" })
export class VolumeDial extends FeishinAction<VolumeDialSettings> {
	protected override readonly watches: readonly StateChange[] = ["volume"];

	override onDialRotate(ev: DialRotateEvent<VolumeDialSettings>): void {
		this.run(ev.action, () => feishin.changeVolume(ev.payload.ticks * numberSetting(ev.payload.settings.tickStep, 2)));
	}

	override onDialDown(ev: DialDownEvent<VolumeDialSettings>): void {
		this.run(ev.action, () => feishin.toggleMute());
	}

	override onTouchTap(ev: TouchTapEvent<VolumeDialSettings>): void {
		this.run(ev.action, () => feishin.toggleMute());
	}

	protected override async render(action: DialAction | KeyAction): Promise<void> {
		if (!action.isDial()) {
			return;
		}

		if (!feishin.isConnected) {
			await action.setFeedback({
				value: streamDeck.i18n.translate("offline"),
				indicator: { value: 0 },
				icon: "imgs/actions/volume/dial.svg",
			});
			return;
		}

		if (feishin.state.volume === null) {
			// See the property inspector's hint: Feishin hasn't reported its volume yet.
			await action.setFeedback({ value: "–", indicator: { value: 0 }, icon: "imgs/actions/volume/dial.svg" });
			return;
		}

		const volume = Math.round(feishin.state.volume);
		await action.setFeedback({
			value: volume === 0 ? streamDeck.i18n.translate("muted") : `${volume}%`,
			indicator: { value: volume },
			icon: volume === 0 ? "imgs/actions/volume/dial-muted.svg" : "imgs/actions/volume/dial.svg",
		});
	}
}
