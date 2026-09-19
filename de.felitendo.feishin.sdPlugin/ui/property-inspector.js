// @ts-check
/* global SDPIComponents */

/**
 * Property inspector shared by all Feishin actions: shows the connection status reported by the plugin, walks the user
 * through enabling Feishin's remote control server, and shows the settings of the selected action.
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
				"Nothing answered at {address}. Make sure Feishin is running and its remote control server is turned on – see the steps below.",
			"status.unauthorized": "Wrong username or password",
			"status.unauthorized.hint":
				"Feishin rejected the login. Use the same username and password in Feishin (Settings → Window → Remote) and in the connection settings below.",
			"status.error": "Can't connect to Feishin",
			"status.error.hint": "{detail} – check the host and port in the connection settings below.",
			"volume.unknown":
				"Feishin hasn't reported its volume yet – it only does that when the volume changes. Change the volume in Feishin once and these controls will work (only needed once).",
			"settings.volumeStep": "Step",
			"settings.showVolume": "Show volume on key",
			"settings.seekStep": "Jump by",
			"settings.dialStep": "Step per tick",
			"settings.dialSeek": "Seek per tick",
			"guide.title": "How to connect Feishin",
			"guide.step1": "Open <b>Feishin</b> on this computer and go to <b>Settings → Window → Remote</b>.",
			"guide.step2": "Turn on <b>Enable remote control server</b>.",
			"guide.step3":
				"Set the username to <code>feishin</code> and the password to <code>streamdeck</code>. Already using other credentials? Enter them under <i>Connection settings</i> below instead.",
			"guide.step4": "Keep the port at <code>4333</code>, or enter your port below.",
			"guide.done": "The status at the top turns green as soon as the connection works.",
			"connection.title": "Connection settings",
			"connection.shared": "These settings apply to all Feishin actions. Leave a field empty to use the default.",
			"connection.host": "Host",
			"connection.port": "Port",
			"connection.username": "Username",
			"connection.password": "Password",
			"connection.reconnect": "Reconnect",
			"links.guide": "Detailed guide",
			"links.issue": "Report a problem",
		},
		de: {
			"checking": "Verbindung zu Feishin wird geprüft…",
			"status.connecting": "Verbinde mit Feishin…",
			"status.connecting.hint": "{address}",
			"status.connected": "Mit Feishin verbunden",
			"status.connected.hint": "{address}",
			"status.unreachable": "Feishin ist nicht erreichbar",
			"status.unreachable.hint":
				"Unter {address} antwortet nichts. Stelle sicher, dass Feishin läuft und die Fernsteuerung aktiviert ist – siehe die Schritte unten.",
			"status.unauthorized": "Falscher Benutzername oder falsches Passwort",
			"status.unauthorized.hint":
				"Feishin hat die Anmeldung abgelehnt. Verwende in Feishin (Einstellungen → Fenster → Fernsteuerung) und unten in den Verbindungseinstellungen denselben Benutzernamen und dasselbe Passwort.",
			"status.error": "Keine Verbindung zu Feishin möglich",
			"status.error.hint": "{detail} – prüfe Host und Port unten in den Verbindungseinstellungen.",
			"volume.unknown":
				"Feishin hat seine Lautstärke noch nicht gemeldet – das passiert erst, wenn sie sich ändert. Ändere die Lautstärke einmal in Feishin, danach funktionieren diese Regler (nur einmal nötig).",
			"settings.volumeStep": "Schrittweite",
			"settings.showVolume": "Lautstärke auf der Taste anzeigen",
			"settings.seekStep": "Springen um",
			"settings.dialStep": "Schritt pro Raste",
			"settings.dialSeek": "Spulen pro Raste",
			"guide.title": "So verbindest du Feishin",
			"guide.step1": "Öffne <b>Feishin</b> auf diesem Computer und gehe zu <b>Einstellungen → Fenster → Fernsteuerung</b>.",
			"guide.step2": "Schalte <b>Server für Fernsteuerung aktivieren</b> ein.",
			"guide.step3":
				"Setze den Benutzernamen auf <code>feishin</code> und das Passwort auf <code>streamdeck</code>. Du nutzt schon andere Zugangsdaten? Dann trage sie stattdessen unten in den <i>Verbindungseinstellungen</i> ein.",
			"guide.step4": "Lass den Port auf <code>4333</code> oder trage deinen Port unten ein.",
			"guide.done": "Sobald die Verbindung steht, wird der Status oben grün.",
			"connection.title": "Verbindungseinstellungen",
			"connection.shared": "Diese Einstellungen gelten für alle Feishin-Aktionen. Leere Felder verwenden den Standardwert.",
			"connection.host": "Host",
			"connection.port": "Port",
			"connection.username": "Benutzer",
			"connection.password": "Passwort",
			"connection.reconnect": "Neu verbinden",
			"links.guide": "Ausführliche Anleitung",
			"links.issue": "Problem melden",
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
	const guide = /** @type {HTMLDetailsElement} */ (document.getElementById("guide"));
	const connectionPanel = /** @type {HTMLDetailsElement} */ (document.getElementById("connection"));
	const volumeUnknown = /** @type {HTMLElement} */ (document.getElementById("volume-unknown"));

	localize();

	client.getConnectionInfo().then(({ info, actionInfo }) => {
		strings = pickStrings(info?.application?.language);
		localize();
		if (connection) {
			renderConnection(connection);
		}

		for (const section of document.querySelectorAll(".action-section")) {
			const actions = (section.getAttribute("data-actions") ?? "").split(/\s+/);
			/** @type {HTMLElement} */ (section).hidden = !actions.includes(actionInfo.action);
		}

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

	for (const link of document.querySelectorAll("a[data-url]")) {
		link.addEventListener("click", (ev) => {
			ev.preventDefault();
			client.send("openUrl", { url: link.getAttribute("data-url") });
		});
	}

	/**
	 * @param {Connection} next
	 */
	function renderConnection(next) {
		const changed = next.status !== connection?.status;
		connection = next;

		statusEl.dataset.status = next.status;
		statusTitle.textContent = t(`status.${next.status}`);
		statusHint.textContent = t(`status.${next.status}.hint`)
			.replace("{address}", next.address)
			.replace("{detail}", next.detail || next.address);
		volumeUnknown.hidden = next.status !== "connected" || next.volumeKnown !== false;

		// Point the user at whatever helps with the current problem; don't fight them when they open / close panels.
		if (changed) {
			if (next.status === "connected") {
				guide.open = false;
			} else if (next.status === "unreachable") {
				guide.open = true;
			} else if (next.status === "unauthorized" || next.status === "error") {
				connectionPanel.open = true;
			}
		}
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
