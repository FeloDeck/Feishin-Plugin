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
			"settings.volume": "Volume",
			"tiles.off": "Off",
			"tiles.below": "Below",
			"tiles.above": "Above",
			"settings.picture": "Picture",
			"picture.icon": "Icon",
			"picture.cover": "Album art",
			"settings.time": "Time",
			"settings.timeShows": "Shows",
			"shows.left": "Time left",
			"shows.played": "Time played",
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
			"settings.volume": "Lautstärke",
			"tiles.off": "Aus",
			"tiles.below": "Unten",
			"tiles.above": "Oben",
			"settings.picture": "Bild",
			"picture.icon": "Symbol",
			"picture.cover": "Albumcover",
			"settings.time": "Zeit",
			"settings.timeShows": "Zeigt",
			"shows.left": "Restzeit",
			"shows.played": "Gespielte Zeit",
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
	const picture = /** @type {HTMLElement} */ (document.getElementById("picture"));
	const timeTiles = /** @type {HTMLElement} */ (document.getElementById("time-tiles"));
	const timeShowsRow = /** @type {HTMLElement} */ (document.getElementById("time-shows-row"));
	const timeShows = /** @type {HTMLElement} */ (document.getElementById("time-shows"));

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
	const [getShowVolume, setShowVolume] = SDPIComponents.useSettings("showVolume", () => refreshTiles());
	const [getVolumeAt, setVolumeAt] = SDPIComponents.useSettings("volumeAt", () => refreshTiles());
	const [getShowCover, setShowCover] = SDPIComponents.useSettings("showCover", () => refreshTiles());
	const [getShowTime, setShowTime] = SDPIComponents.useSettings("showTime", () => refreshTiles());
	const [getTimeAt, setTimeAt] = SDPIComponents.useSettings("timeAt", () => refreshTiles());
	const [getTimeShows, setTimeShows] = SDPIComponents.useSettings("timeShows", () => refreshTiles());

	// Rows like in obs-tools: 30 % for the label, the rest for the field. sdpi-item has a fixed 95 px + 241 px grid.
	for (const item of document.querySelectorAll("sdpi-item")) {
		const style = document.createElement("style");
		// More specific than its own ".grid", which comes later.
		style.textContent = ".container.grid { grid-template-columns: 30% minmax(0, 1fr); }";
		item.shadowRoot?.append(style);
	}
	for (const source of document.querySelectorAll("select.choice")) {
		dropdown(/** @type {HTMLSelectElement} */ (source));
	}
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
		refreshTiles();

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

	async function refreshTiles() {
		renderLook({
			showVolume: await getShowVolume(),
			volumeAt: await getVolumeAt(),
			showCover: await getShowCover(),
			showTime: await getShowTime(),
			timeAt: await getTimeAt(),
			timeShows: await getTimeShows(),
		});
	}

	/**
	 * The rows of "On the key". A click renders them with the new value right away, because sdpi-components saves a
	 * setting only after a short wait.
	 * @param {Record<string, unknown>} s The settings the rows need.
	 */
	function renderLook(s) {
		const icon = VOLUME_ICONS[action];
		if (icon) {
			renderTiles(volumeTiles, s.showVolume === false ? "off" : s.volumeAt === "above" ? "above" : "below", {
				picture: (option) => keyPicture(`volume/${icon}`, 15.6, option === "off" ? "" : "50%", option),
				pick: (option) => {
					setShowVolume(option !== "off");
					if (option !== "off") {
						setVolumeAt(option);
					}
					renderLook({ ...s, showVolume: option !== "off", volumeAt: option === "off" ? s.volumeAt : option });
				},
			});
		}

		if (action === "de.felitendo.feishin.playpause") {
			const cover = s.showCover === true;
			renderSegments(picture, "picture", ["icon", "cover"], cover ? "cover" : "icon", (option) => {
				setShowCover(option === "cover");
				renderLook({ ...s, showCover: option === "cover" });
			});

			const shows = s.timeShows === "played" ? "played" : "left";
			const time = shows === "played" ? "1:05" : "-2:31";
			// The key as it looks while playing.
			renderTiles(timeTiles, s.showTime === false ? "off" : s.timeAt === "above" ? "above" : "below", {
				picture: (option) => {
					const text = option === "off" ? "" : time;
					return cover ? coverPicture(text, option) : keyPicture("play-pause/pause", 15.5, text, option);
				},
				pick: (option) => {
					setShowTime(option !== "off");
					if (option !== "off") {
						setTimeAt(option);
					}
					renderLook({ ...s, showTime: option !== "off", timeAt: option === "off" ? s.timeAt : option });
				},
			});

			timeShowsRow.hidden = s.showTime === false;
			renderSegments(timeShows, "shows", ["left", "played"], shows, (option) => {
				setTimeShows(option);
				renderLook({ ...s, timeShows: option });
			});
		}
		repaint();
	}

	/**
	 * Choices side by side.
	 * @param {HTMLElement} segments
	 * @param {string} name Their texts are "name.option".
	 * @param {string[]} options
	 * @param {string} choice
	 * @param {(option: string) => void} pick
	 */
	function renderSegments(segments, name, options, choice, pick) {
		segments.replaceChildren(
			...options.map((option) => {
				const button = document.createElement("button");
				button.type = "button";
				button.className = option === choice ? "on" : "";
				button.setAttribute("role", "radio");
				button.setAttribute("aria-checked", String(option === choice));
				button.textContent = t(`${name}.${option}`);
				button.addEventListener("click", () => pick(option));
				return button;
			}),
		);
	}

	/**
	 * Where the key shows its text: not at all, below or above the icon. Tiles with a small picture of the key.
	 * @param {HTMLElement} tiles
	 * @param {string} choice "off", "below" or "above"
	 * @param {{ picture: (option: string) => string; pick: (option: string) => void }} how The picture of the key with
	 * a choice, and what picking it does.
	 */
	function renderTiles(tiles, choice, how) {
		tiles.replaceChildren(
			...["off", "below", "above"].map((option) => {
				const tile = document.createElement("button");
				tile.type = "button";
				tile.className = option === choice ? "tile on" : "tile";
				tile.setAttribute("role", "radio");
				tile.setAttribute("aria-checked", String(option === choice));
				tile.addEventListener("click", () => how.pick(option));
				const key = document.createElement("span");
				key.className = "tile-key";
				key.innerHTML = how.picture(option);
				const name = document.createElement("span");
				name.className = "tile-name";
				name.textContent = t(`tiles.${option}`);
				tile.append(key, name);
				return tile;
			}),
		);
	}

	/**
	 * A small picture of the key, drawn like svgWithTitle in src/images.ts: icon and text together in the middle, 16 px
	 * apart. The text is under or above the icon.
	 * @param {string} icon Icon in imgs/actions, without ".svg". Its view box is 40 units wide.
	 * @param {number} height How tall the icon is, in units of its view box.
	 * @param {string} text Text on the key, "" for none.
	 * @param {string} at "below" or "above"
	 */
	function keyPicture(icon, height, text, at) {
		const ink = height * 3.6;
		const above = at === "above";
		const shift = text ? 16 : 0;
		const baseline = above ? (144 - ink) / 2 + shift - 16 : (144 + ink) / 2 + shift;
		const picture = `<image href="../imgs/actions/${icon}.svg" y="${above ? shift : -shift}" width="144" height="144"/>`;
		const title = text
			? `<text x="72" y="${Math.round(baseline * 10) / 10}" font-family="'Liberation Sans',Arial,sans-serif" font-size="22" font-weight="700" fill="#fff" text-anchor="middle">${text}</text>`
			: "";
		return `<svg viewBox="0 0 144 144">${picture}${title}</svg>`;
	}

	/**
	 * A small picture of the key with a cover, drawn like svgWithCover in src/images.ts. A made up cover stands in for
	 * the one of the song.
	 * @param {string} text Text on the key, "" for none.
	 * @param {string} at "below" or "above"
	 */
	function coverPicture(text, at) {
		const cover =
			`<rect width="144" height="144" fill="#4f46e5"/><circle cx="112" cy="30" r="64" fill="#ec4899" fill-opacity="0.85"/>` +
			`<circle cx="24" cy="128" r="56" fill="#f59e0b" fill-opacity="0.8"/>`;
		if (!text) {
			return `<svg viewBox="0 0 144 144">${cover}</svg>`;
		}

		const above = at === "above";
		// Several pictures are on the page, and ids are for the whole page.
		const id = `shade-${at}`;
		const shade =
			`<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#000" stop-opacity="${above ? 0.7 : 0}"/>` +
			`<stop offset="1" stop-color="#000" stop-opacity="${above ? 0 : 0.7}"/></linearGradient></defs>` +
			`<rect y="${above ? 0 : 84}" width="144" height="60" fill="url(#${id})"/>`;
		const title = `<text x="72" y="${above ? 30 : 130}" font-family="'Liberation Sans',Arial,sans-serif" font-size="22" font-weight="700" fill="#fff" text-anchor="middle">${text}</text>`;
		return `<svg viewBox="0 0 144 144">${cover}${shade}${title}</svg>`;
	}

	/** @type {{ root: HTMLElement; close: () => void } | null} */
	let openDropdown = null;

	function closeDropdown() {
		const current = openDropdown;
		openDropdown = null;
		current?.close();
	}

	document.addEventListener("mousedown", (e) => {
		if (openDropdown && !openDropdown.root.contains(/** @type {Node} */ (e.target))) {
			closeDropdown();
		}
	});

	/**
	 * A drop-down like in obs-tools, in place of a hidden <select class="choice"> that holds the setting
	 * (data-setting), its default (data-default) and the options. OpenDeck's WebKitGTK draws native drop-downs, and the
	 * ones of sdpi-components, in the GTK style.
	 * @param {HTMLSelectElement} source
	 */
	function dropdown(source) {
		const fallback = source.dataset.default ?? "";
		const options = [...source.options].map((o) => ({ value: o.value, label: o.textContent ?? "" }));
		let value = fallback;

		const root = document.createElement("div");
		root.className = "dropdown";
		const button = document.createElement("button");
		button.type = "button";
		button.className = "dd-button";
		const label = document.createElement("span");
		label.className = "dd-label";
		button.append(label);
		button.insertAdjacentHTML(
			"beforeend",
			'<span class="dd-chevron"><svg width="10" height="6" viewBox="0 0 10 6"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg></span>',
		);
		root.append(button);
		source.replaceWith(root);

		const [getValue, setValue] = SDPIComponents.useSettings(source.dataset.setting ?? "", (/** @type {unknown} */ v) =>
			show(v),
		);
		getValue().then(show);

		/** @param {unknown} v */
		function show(v) {
			value = v === undefined || v === null || v === "" ? fallback : String(v);
			label.textContent = options.find((o) => o.value === value)?.label ?? value;
		}

		button.addEventListener("click", () => (openDropdown?.root === root ? closeDropdown() : open()));
		button.addEventListener("keydown", (e) => {
			if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key) && openDropdown?.root !== root) {
				e.preventDefault();
				open();
			}
		});

		function open() {
			closeDropdown();
			root.classList.add("open");
			const pop = document.createElement("div");
			pop.className = "dd-pop";
			pop.setAttribute("role", "listbox");
			let active = Math.max(0, options.findIndex((o) => o.value === value));
			const items = options.map((option, i) => {
				const item = document.createElement("div");
				item.className = option.value === value ? "dd-item selected" : "dd-item";
				item.setAttribute("role", "option");
				item.innerHTML =
					option.value === value
						? '<span class="dd-check"><svg width="13" height="10" viewBox="0 0 13 10"><path d="M1.6 5.3 4.8 8.4 11.4 1.6" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/></svg></span>'
						: '<span class="dd-check"></span>';
				const text = document.createElement("span");
				text.className = "dd-text";
				text.textContent = option.label;
				item.append(text);
				item.addEventListener("mousedown", (e) => {
					e.preventDefault();
					pick(option.value);
				});
				item.addEventListener("mouseenter", () => {
					active = i;
					mark();
				});
				return item;
			});
			pop.append(...items);
			root.append(pop);

			const mark = () => items.forEach((item, i) => item.classList.toggle("active", i === active));
			/** @param {string} next */
			const pick = (next) => {
				closeDropdown();
				button.focus();
				if (next !== value) {
					show(next);
					setValue(next);
				}
			};
			/** @param {KeyboardEvent} e */
			const onKey = (e) => {
				if (e.key === "ArrowDown" || e.key === "ArrowUp") {
					active = Math.min(items.length - 1, Math.max(0, active + (e.key === "ArrowDown" ? 1 : -1)));
					mark();
				} else if (e.key === "Enter" || e.key === " ") {
					pick(options[active].value);
				} else if (e.key === "Escape" || e.key === "Tab") {
					closeDropdown();
					button.focus();
				} else {
					return;
				}
				e.preventDefault();
				e.stopPropagation();
			};

			// Open upwards when there is more room above.
			const rect = button.getBoundingClientRect();
			const below = window.innerHeight - rect.bottom;
			pop.classList.toggle("up", below < pop.offsetHeight + 8 && rect.top > below);
			mark();
			document.addEventListener("keydown", onKey, true);
			openDropdown = {
				root,
				close: () => {
					document.removeEventListener("keydown", onKey, true);
					pop.remove();
					root.classList.remove("open");
					repaint();
				},
			};
			repaint();
		}
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
