import {
	type DialAction,
	type DidReceiveSettingsEvent,
	type KeyAction,
	type KeyDownEvent,
	type KeyUpEvent,
	SingletonAction,
	type WillAppearEvent,
	type WillDisappearEvent,
} from "@elgato/streamdeck";
import type { JsonObject } from "@elgato/utils";

import { feishin, type StateChange } from "../feishin";

/**
 * Stream Deck's guidelines allow at most 10 updates per second per key / touch strip; turning a dial quickly or holding
 * a volume key changes the state faster than that.
 */
const MIN_RENDER_INTERVAL_MS = 100;

/**
 * Base class for all Feishin actions: keeps every visible instance in sync with Feishin's player state.
 */
export abstract class FeishinAction<T extends JsonObject = JsonObject> extends SingletonAction<T> {
	/**
	 * Player state this action displays; visible instances are re-rendered when any of it changes.
	 */
	protected readonly watches: readonly StateChange[] = [];

	/**
	 * Settings of the visible instances, so re-rendering doesn't need a round-trip to Stream Deck.
	 */
	readonly #settings = new Map<string, T>();

	/**
	 * Images last set per instance and state, see {@link setStateImage}.
	 */
	readonly #images = new Map<string, string | undefined>();

	readonly #lastRender = new Map<string, number>();
	readonly #pendingRender = new Map<string, NodeJS.Timeout>();

	constructor() {
		super();
		feishin.on("state", (changes) => {
			if (this.watches.some((key) => changes.has(key))) {
				this.#renderAll();
			}
		});
		feishin.on("connection", () => this.#renderAll());
	}

	override onWillAppear(ev: WillAppearEvent<T>): Promise<void> | void {
		feishin.wake();
		this.#settings.set(ev.action.id, ev.payload.settings);
		this.#forgetImages(ev.action.id);
		return this.#renderNow(ev.action);
	}

	override onWillDisappear(ev: WillDisappearEvent<T>): Promise<void> | void {
		this.#settings.delete(ev.action.id);
		this.#forgetImages(ev.action.id);
		clearTimeout(this.#pendingRender.get(ev.action.id));
		this.#pendingRender.delete(ev.action.id);
		this.#lastRender.delete(ev.action.id);
	}

	override onDidReceiveSettings(ev: DidReceiveSettingsEvent<T>): Promise<void> | void {
		this.#settings.set(ev.action.id, ev.payload.settings);
		return this.#renderNow(ev.action);
	}

	/**
	 * Updates the image, state, title, or touch strip of an action instance.
	 */
	protected render(_action: DialAction<T> | KeyAction<T>, _settings: T): Promise<void> | void {}

	/**
	 * Sets the image of one state, skipping the message when it's already showing (renders happen on every volume or
	 * position change).
	 * @param image Image to show; `undefined` restores the image from the manifest.
	 */
	protected async setStateImage(action: KeyAction<T>, state: 0 | 1, image: string | undefined): Promise<void> {
		const key = `${action.id}/${state}`;
		if (this.#images.has(key) && this.#images.get(key) === image) {
			return;
		}

		this.#images.set(key, image);
		await action.setImage(image, { state });
	}

	/**
	 * Runs a Feishin command, showing the warning triangle on the key / dial when it couldn't be sent (usually because
	 * Feishin isn't connected; the property inspector explains how to fix that).
	 */
	protected run(action: DialAction<T> | KeyAction<T>, command: () => boolean): void {
		if (!command()) {
			feishin.wake();
			void action.showAlert();
		}
	}

	#forgetImages(id: string): void {
		this.#images.delete(`${id}/0`);
		this.#images.delete(`${id}/1`);
	}

	#renderAll(): void {
		for (const action of this.actions) {
			this.#requestRender(action);
		}
	}

	/**
	 * Renders now, or once {@link MIN_RENDER_INTERVAL_MS} has passed since the last render; the delayed render uses the
	 * state at that time, so the last change is never lost.
	 */
	#requestRender(action: DialAction<T> | KeyAction<T>): void {
		if (this.#pendingRender.has(action.id)) {
			return;
		}

		const wait = (this.#lastRender.get(action.id) ?? 0) + MIN_RENDER_INTERVAL_MS - Date.now();
		if (wait <= 0) {
			void this.#renderNow(action);
			return;
		}

		this.#pendingRender.set(
			action.id,
			setTimeout(() => {
				this.#pendingRender.delete(action.id);
				void this.#renderNow(action);
			}, wait),
		);
	}

	async #renderNow(action: DialAction<T> | KeyAction<T>): Promise<void> {
		const settings = this.#settings.get(action.id);
		if (settings) {
			this.#lastRender.set(action.id, Date.now());
			await this.render(action, settings);
		}
	}
}

const REPEAT_DELAY_MS = 400;
const REPEAT_INTERVAL_MS = 150;
const REPEAT_MAX = 100;

/**
 * A key that repeats its command while held down (volume and seek keys).
 */
export abstract class RepeatingKeyAction<T extends JsonObject = JsonObject> extends FeishinAction<T> {
	readonly #timers = new Map<string, NodeJS.Timeout>();

	protected abstract press(settings: T): boolean;

	override onKeyDown(ev: KeyDownEvent<T>): void {
		const { action } = ev;
		const settings = ev.payload.settings;
		this.#stop(action.id);
		this.run(action, () => this.press(settings));

		// Multi-actions don't report when the key is released.
		if (ev.payload.isInMultiAction || !feishin.isConnected) {
			return;
		}

		let count = 0;
		const repeat = (): void => {
			if (++count > REPEAT_MAX || !this.press(settings)) {
				this.#stop(action.id);
				return;
			}

			this.#timers.set(action.id, setTimeout(repeat, REPEAT_INTERVAL_MS));
		};

		this.#timers.set(action.id, setTimeout(repeat, REPEAT_DELAY_MS));
	}

	override onKeyUp(ev: KeyUpEvent<T>): void {
		this.#stop(ev.action.id);
	}

	override onWillDisappear(ev: WillDisappearEvent<T>): Promise<void> | void {
		this.#stop(ev.action.id);
		return super.onWillDisappear(ev);
	}

	#stop(id: string): void {
		clearTimeout(this.#timers.get(id));
		this.#timers.delete(id);
	}
}

/**
 * Reads a numeric setting (the property inspector stores select values as strings).
 */
export function numberSetting(value: unknown, fallback: number): number {
	const number = Number(value);
	return value !== undefined && value !== "" && Number.isFinite(number) && number > 0 ? number : fallback;
}
