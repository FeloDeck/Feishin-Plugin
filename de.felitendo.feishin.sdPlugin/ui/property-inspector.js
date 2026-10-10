// @ts-check
/* global SDPIComponents */

/**
 * Property inspector shared by all Feishin actions: shows the connection status reported by the plugin, the connection
 * settings under it (with the steps to enable Feishin's remote control server while there is no connection), and the
 * settings of the selected action in sections.
 */
(() => {
	const STRINGS = {
		en: {
			"checking": "Checking connection to Feishin…",
			"status.connecting": "Connecting to Feishin…",
			"status.connecting.hint": "{address}",
			"status.connected": "Connected to Feishin",
			"status.connected.hint": "{address}",
			"status.unreachable": "Feishin can't be reached",
			"status.unreachable.hint":
				"Nothing answered at {address}. Make sure Feishin is running and its remote control server is turned on. See the steps below.",
			"status.unauthorized": "Wrong username or password",
			"status.unauthorized.hint":
				"Feishin rejected the login. Use the same username and password in Feishin (Settings → Window → Remote) and below.",
			"status.error": "Can't connect to Feishin",
			"status.error.hint": "{detail}. Check the host and port below.",
			"status.change": "Change",
			"status.close": "Close",
			"volume.unknown":
				"Feishin hasn't reported its volume yet – it only does that when the volume changes. Change the volume in Feishin once and these controls will work (only needed once).",
			"section.controls": "Controls",
			"section.look": "On the key",
			"settings.volumeStep": "Step",
			"settings.display": "Display",
			"tiles.icon": "Icon",
			"tiles.volume": "Volume",
			"settings.seekStep": "Jump by",
			"settings.dialStep": "Step per tick",
			"settings.dialSeek": "Seek per tick",
			"guide.title": "How to connect Feishin",
			"guide.step1": "Open <b>Feishin</b> on this computer and go to <b>Settings → Window → Remote</b>.",
			"guide.step2": "Turn on <b>Enable remote control server</b>.",
			"guide.step3":
				"Set the username to <code>feishin</code> and the password to <code>streamdeck</code>. Already using other credentials? Enter them below instead.",
			"guide.step4": "Keep the port at <code>4333</code>, or enter your port below.",
			"guide.done": "The status at the top turns green as soon as the connection works.",
			"connection.shared": "These settings apply to all Feishin actions. Leave a field empty to use the default.",
			"connection.host": "Host",
			"connection.port": "Port",
			"connection.username": "Username",
			"connection.password": "Password",
			"connection.reconnect": "Reconnect",
		},
		de: {
			"checking": "Verbindung zu Feishin wird geprüft…",
			"status.connecting": "Verbinde mit Feishin…",
			"status.connecting.hint": "{address}",
			"status.connected": "Mit Feishin verbunden",
			"status.connected.hint": "{address}",
			"status.unreachable": "Feishin ist nicht erreichbar",
			"status.unreachable.hint":
				"Unter {address} antwortet nichts. Stelle sicher, dass Feishin läuft und die Fernsteuerung aktiviert ist. Siehe die Schritte unten.",
			"status.unauthorized": "Falscher Benutzername oder falsches Passwort",
			"status.unauthorized.hint":
				"Feishin hat die Anmeldung abgelehnt. Verwende in Feishin (Einstellungen → Fenster → Fernsteuerung) und unten denselben Benutzernamen und dasselbe Passwort.",
			"status.error": "Keine Verbindung zu Feishin möglich",
			"status.error.hint": "{detail}. Prüfe Host und Port unten.",
			"status.change": "Ändern",
			"status.close": "Schließen",
			"volume.unknown":
				"Feishin hat seine Lautstärke noch nicht gemeldet – das passiert erst, wenn sie sich ändert. Ändere die Lautstärke einmal in Feishin, danach funktionieren diese Regler (nur einmal nötig).",
			"section.controls": "Bedienung",
			"section.look": "Auf der Taste",
			"settings.volumeStep": "Schrittweite",
			"settings.display": "Anzeige",
			"tiles.icon": "Symbol",
			"tiles.volume": "Lautstärke",
			"settings.seekStep": "Springen um",
			"settings.dialStep": "Schritt pro Raste",
			"settings.dialSeek": "Spulen pro Raste",
			"guide.title": "So verbindest du Feishin",
			"guide.step1": "Öffne <b>Feishin</b> auf diesem Computer und gehe zu <b>Einstellungen → Fenster → Fernsteuerung</b>.",
			"guide.step2": "Schalte <b>Server für Fernsteuerung aktivieren</b> ein.",
			"guide.step3":
				"Setze den Benutzernamen auf <code>feishin</code> und das Passwort auf <code>streamdeck</code>. Du nutzt schon andere Zugangsdaten? Dann trage sie stattdessen unten ein.",
			"guide.step4": "Lass den Port auf <code>4333</code> oder trage deinen Port unten ein.",
			"guide.done": "Sobald die Verbindung steht, wird der Status oben grün.",
			"connection.shared": "Diese Einstellungen gelten für alle Feishin-Aktionen. Leere Felder verwenden den Standardwert.",
			"connection.host": "Host",
			"connection.port": "Port",
			"connection.username": "Benutzer",
			"connection.password": "Passwort",
			"connection.reconnect": "Neu verbinden",
		},
	};

	/** @type {Record<string, string>} */
	let strings = pickStrings(navigator.language);

	/** @typedef {{ status: string; address: string; detail?: string; volumeKnown?: boolean }} Connection */

	/** @type {Connection | undefined} */
	let connection;

	const client = SDPIComponents.streamDeckClient;
	const statusEl = /** @type {HTMLElement} */ (document.getElementById("status"));
	const statusTitle = /** @type {HTMLElement} */ (document.getElementById("status-title"));
	const statusHint = /** @type {HTMLElement} */ (document.getElementById("status-hint"));
	const statusAction = /** @type {HTMLButtonElement} */ (document.getElementById("status-action"));
	const connectionBox = /** @type {HTMLElement} */ (document.getElementById("connection"));
	const guide = /** @type {HTMLElement} */ (document.getElementById("guide"));
	const volumeUnknown = /** @type {HTMLElement} */ (document.getElementById("volume-unknown"));
	const volumeTiles = /** @type {HTMLElement} */ (document.getElementById("volume-tiles"));

	/**
	 * The connection box: open while there is a problem (autoOpen), unless the user opened or closed it (setupOpen).
	 * @type {boolean | null}
	 */
	let setupOpen = null;
	let autoOpen = false;

	/** The selected action. */
	let action = "";

	// The key pictures the volume tiles show, per action.
	const VOLUME_ICONS = /** @type {Record<string, string>} */ ({
		"de.felitendo.feishin.volumeup": "up",
		"de.felitendo.feishin.volumedown": "down",
		"de.felitendo.feishin.mute": "unmuted",
	});
	const [getShowVolume, setShowVolume] = SDPIComponents.useSettings("showVolume", (/** @type {unknown} */ value) =>
		renderTiles(value),
	);

	localize();
	renderBox();

	client.getConnectionInfo().then(({ info, actionInfo }) => {
		strings = pickStrings(info?.application?.language);
		action = actionInfo.action;
		localize();
		if (connection) {
			renderConnection(connection);
		}

		for (const part of document.querySelectorAll(".action-section")) {
			const actions = (part.getAttribute("data-actions") ?? "").split(/\s+/);
			/** @type {HTMLElement} */ (part).hidden = !actions.includes(action);
		}
		for (const section of document.querySelectorAll(".section")) {
			/** @type {HTMLElement} */ (section).hidden = !section.querySelector(".action-section:not([hidden])");
		}
		renderBox();
		getShowVolume().then(renderTiles);

		client.send("sendToPlugin", { event: "getConnection" });
	});

	client.sendToPropertyInspector.subscribe((/** @type {{ payload?: any }} */ ev) => {
		if (ev.payload?.event === "connection") {
			renderConnection(ev.payload);
		}
	});

	document.getElementById("reconnect")?.addEventListener("click", () => {
		client.send("sendToPlugin", { event: "reconnect" });
	});

	statusAction.addEventListener("click", () => {
		setupOpen = !(setupOpen ?? autoOpen);
		renderBox();
	});

	/**
	 * @param {Connection} next
	 */
	function renderConnection(next) {
		connection = next;

		statusEl.dataset.status = next.status;
		statusEl.classList.toggle("quiet", next.status === "connected");
		statusTitle.textContent = t(`status.${next.status}`);
		statusHint.textContent = t(`status.${next.status}.hint`)
			.replace("{address}", next.address)
			.replace("{detail}", next.detail || next.address);
		volumeUnknown.hidden = next.status !== "connected" || next.volumeKnown !== false;

		// A new problem opens the connection box, a working connection closes it. "Connecting" while Feishin is away
		// changes nothing, or the box would flicker with each try.
		const problem = ["unreachable", "unauthorized", "error"].includes(next.status);
		if ((next.status === "connected" || problem) && problem !== autoOpen) {
			autoOpen = problem;
			setupOpen = null;
		}
		guide.hidden = next.status === "connected";
		renderBox();
	}

	function renderBox() {
		const open = setupOpen ?? autoOpen;
		connectionBox.hidden = !open;
		statusAction.textContent = t(open ? "status.close" : "status.change");
		statusAction.setAttribute("aria-expanded", String(open));
		repaint();
	}

	/**
	 * Show volume on the key or not, as two tiles with a small picture of the key.
	 * @param {unknown} value
	 */
	function renderTiles(value) {
		const icon = VOLUME_ICONS[action];
		if (!icon) {
			return;
		}
		const shown = value !== false;
		volumeTiles.replaceChildren(
			...[false, true].map((show) => {
				const tile = document.createElement("button");
				tile.type = "button";
				tile.className = show === shown ? "tile on" : "tile";
				tile.setAttribute("role", "radio");
				tile.setAttribute("aria-checked", String(show === shown));
				tile.addEventListener("click", () => {
					setShowVolume(show);
					renderTiles(show);
				});
				// Drawn like the key (svgWithTitle in src/images.ts): the icon 13 px up, the volume right under it.
				const picture = `<image href="../imgs/actions/volume/${icon}.svg" y="${show ? -13 : 0}" width="144" height="144"/>`;
				const title = show
					? `<text x="72" y="113.1" font-family="'Liberation Sans',Arial,sans-serif" font-size="22" fill="#fff" text-anchor="middle">50%</text>`
					: "";
				const key = document.createElement("span");
				key.className = "tile-key";
				key.innerHTML = `<svg viewBox="0 0 144 144">${picture}${title}</svg>`;
				const name = document.createElement("span");
				name.className = "tile-name";
				name.textContent = t(show ? "tiles.volume" : "tiles.icon");
				tile.append(key, name);
				return tile;
			}),
		);
	}

	// WebKitGTK in OpenDeck sometimes paints only parts of the page after a big change and keeps old pixels elsewhere. A
	// tiny change of the opacity makes it paint the whole page again.
	function repaint() {
		document.body.style.opacity = document.body.style.opacity === "0.999" ? "" : "0.999";
	}

	function localize() {
		document.documentElement.lang = strings === STRINGS.de ? "de" : "en";
		for (const el of document.querySelectorAll("[data-i18n]")) {
			el.textContent = t(el.getAttribute("data-i18n") ?? "");
		}

		// Only ever set from the static strings above.
		for (const el of document.querySelectorAll("[data-i18n-html]")) {
			el.innerHTML = t(el.getAttribute("data-i18n-html") ?? "");
		}

		for (const el of document.querySelectorAll("[data-i18n-label]")) {
			el.setAttribute("label", t(el.getAttribute("data-i18n-label") ?? ""));
		}
	}

	/**
	 * @param {string} key
	 */
	function t(key) {
		return strings[key] ?? STRINGS.en[/** @type {keyof typeof STRINGS.en} */ (key)] ?? key;
	}

	/**
	 * @param {string | undefined} language
	 * @returns {Record<string, string>}
	 */
	function pickStrings(language) {
		return language?.toLowerCase().startsWith("de") ? STRINGS.de : STRINGS.en;
	}
})();
