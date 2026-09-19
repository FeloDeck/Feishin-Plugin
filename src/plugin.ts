import streamDeck from "@elgato/streamdeck";

import { Next, PlayPause, Previous, Repeat, Shuffle } from "./actions/playback";
import { FastForward, Rewind, SeekDial } from "./actions/seek";
import { Mute, VolumeDial, VolumeDown, VolumeUp } from "./actions/volume";
import { type ConnectionSettings, feishin } from "./feishin";

type GlobalSettings = ConnectionSettings & {
	/** Last volume seen, see below. */
	lastVolume?: number;
};

// Keep this at "info" or above: "trace" would log every message, including the global settings with the password.
streamDeck.logger.setLevel("info");

streamDeck.actions.registerAction(new PlayPause());
streamDeck.actions.registerAction(new Next());
streamDeck.actions.registerAction(new Previous());
streamDeck.actions.registerAction(new Shuffle());
streamDeck.actions.registerAction(new Repeat());
streamDeck.actions.registerAction(new VolumeUp());
streamDeck.actions.registerAction(new VolumeDown());
streamDeck.actions.registerAction(new Mute());
streamDeck.actions.registerAction(new FastForward());
streamDeck.actions.registerAction(new Rewind());
streamDeck.actions.registerAction(new VolumeDial());
streamDeck.actions.registerAction(new SeekDial());

/**
 * Messages from the property inspector (ui/property-inspector.js).
 */
type UIMessage = { event: "getConnection" } | { event: "reconnect" };

streamDeck.ui.onSendToPlugin<UIMessage>((ev) => {
	switch (ev.payload.event) {
		case "getConnection":
			void sendConnectionToUI();
			break;
		case "reconnect":
			feishin.reconnect();
			break;
	}
});

// Also push the status when a property inspector becomes visible: OpenDeck loads property inspectors in the background
// and only reports them as visible when the key is selected, so their initial "getConnection" request can't be answered
// yet (Stream Deck only accepts messages for the visible one).
streamDeck.ui.onDidAppear(() => void sendConnectionToUI());

feishin.on("connection", (connection) => {
	streamDeck.logger.info(`Feishin (${connection.address}): ${connection.status}${connection.detail ? ` - ${connection.detail}` : ""}`);
	void sendConnectionToUI();
});

/**
 * Shows the connection status in the property inspector, which uses it to guide the user through the setup.
 */
async function sendConnectionToUI(): Promise<void> {
	if (streamDeck.ui.action) {
		const { status, address, detail } = feishin.connection;
		const volumeKnown = feishin.state.volume !== null;
		await streamDeck.ui.sendToPropertyInspector({ event: "connection", status, address, detail: detail ?? "", volumeKnown });
	}
}

let globalSettings: GlobalSettings = {};

// Connection settings are global so they apply to every Feishin action; the property inspector edits them. Changes
// arrive while the user is typing, so wait for a short pause before reconnecting.
let configureTimer: NodeJS.Timeout | undefined;
streamDeck.settings.onDidReceiveGlobalSettings<GlobalSettings>((ev) => {
	globalSettings = ev.settings;
	clearTimeout(configureTimer);
	configureTimer = setTimeout(() => feishin.configure(ev.settings), 600);
});

// Feishin only reports its volume after it changes, so remember the last one: Feishin restores its volume when it
// starts, which makes the remembered value correct in the usual case, and the volume actions work right away.
let saveVolumeTimer: NodeJS.Timeout | undefined;
feishin.on("state", (changes) => {
	if (!changes.has("volume")) {
		return;
	}

	void sendConnectionToUI(); // Updates the "volume unknown" hint.
	clearTimeout(saveVolumeTimer);
	saveVolumeTimer = setTimeout(() => {
		const volume = feishin.state.volume === null ? undefined : Math.round(feishin.state.volume);
		if (volume !== undefined && volume !== globalSettings.lastVolume) {
			globalSettings = { ...globalSettings, lastVolume: volume };
			void streamDeck.settings.setGlobalSettings(globalSettings);
		}
	}, 2000);
});

await streamDeck.connect();
globalSettings = await streamDeck.settings.getGlobalSettings<GlobalSettings>();
if (typeof globalSettings.lastVolume === "number") {
	feishin.assumeVolume(globalSettings.lastVolume);
}

feishin.configure(globalSettings);
