import { EventEmitter } from "node:events";
import WebSocket from "ws";

/**
 * Connection settings, stored in the plugin's global settings so they apply to every Feishin action.
 * Values are kept as strings because that is what the property inspector's text fields produce.
 */
export type ConnectionSettings = {
	host?: string;
	port?: string;
	username?: string;
	password?: string;
};

/**
 * Defaults that match the setup instructions shown in the property inspector.
 */
export const DEFAULT_CONNECTION = {
	host: "localhost",
	port: 4333,
	username: "feishin",
	password: "streamdeck",
} as const;

/**
 * - `connecting`: a connection attempt is in progress.
 * - `connected`: authenticated and receiving the player state.
 * - `unreachable`: nothing is listening at host:port (Feishin closed, remote control server disabled, wrong port).
 * - `unauthorized`: Feishin rejected the username / password.
 * - `error`: anything else (e.g. another program is using the port).
 */
export type ConnectionStatus = "connecting" | "connected" | "unreachable" | "unauthorized" | "error";

export type ConnectionInfo = {
	status: ConnectionStatus;
	/** `host:port` the plugin is trying to reach. */
	address: string;
	/** Technical detail for the `error` status. */
	detail?: string;
};

export type PlaybackStatus = "playing" | "paused" | "stopped";
export type RepeatMode = "none" | "all" | "one";

export type Song = {
	name: string;
	artist: string;
	/** Duration in seconds. */
	duration: number;
};

export type PlayerState = {
	status: PlaybackStatus;
	repeat: RepeatMode;
	shuffle: boolean;
	/**
	 * 0 - 100, or `null` while unknown: Feishin only reports the volume after it has changed since Feishin started.
	 */
	volume: number | null;
	song: Song | null;
};

/**
 * Emitted with the list of properties that changed; `position` is emitted for seeks and progress updates.
 */
export type StateChange = keyof PlayerState | "position";

type Events = {
	connection: [ConnectionInfo];
	state: [Set<StateChange>];
};

/**
 * Feishin pushes the playback position every ~200ms while playing and echoes volume changes; right after a local change
 * those updates can still carry the old value, so they are ignored for a short while to keep repeated presses (or dial
 * turns) from jumping back.
 */
const SETTLE_MS = 700;
const RECONNECT_DELAYS_MS = [1000, 2000, 3000, 5000];
const UNAUTHORIZED_RETRY_MS = 15000;
const HANDSHAKE_TIMEOUT_MS = 5000;

// Close codes used by Feishin's remote control server (src/main/features/core/remote/index.ts).
const CLOSE_SERVER_SHUTDOWN = 4000;
const CLOSE_SETTINGS_CHANGED = 4002;
const CLOSE_AUTH_FAILED = 4004;

/**
 * Client for Feishin's remote control server (Settings → Window → Remote).
 */
class FeishinClient extends EventEmitter<Events> {
	#settings: Required<ConnectionSettings> = {
		host: DEFAULT_CONNECTION.host,
		port: String(DEFAULT_CONNECTION.port),
		username: DEFAULT_CONNECTION.username,
		password: DEFAULT_CONNECTION.password,
	};

	#socket: WebSocket | undefined;
	#reconnectTimer: NodeJS.Timeout | undefined;
	#attempt = 0;
	#started = false;
	#lastError: NodeJS.ErrnoException | undefined;
	#connection: ConnectionInfo = { status: "connecting", address: this.#address };

	#state: PlayerState = {
		status: "stopped",
		repeat: "none",
		shuffle: false,
		volume: null,
		song: null,
	};

	#position = 0;
	#positionUpdatedAt = Date.now();
	#seekSettleUntil = 0;
	#volumeSettleUntil = 0;
	#volumeBeforeMute: number | undefined;

	constructor() {
		super();
		// Every action type subscribes once, which is more than Node's default warning threshold of 10.
		this.setMaxListeners(0);
	}

	get connection(): ConnectionInfo {
		return this.#connection;
	}

	get isConnected(): boolean {
		return this.#connection.status === "connected";
	}

	get state(): Readonly<PlayerState> {
		return this.#state;
	}

	/**
	 * Uses a previously seen volume while Feishin hasn't reported one yet; Feishin restores its volume on start, so this
	 * is usually still correct.
	 */
	assumeVolume(volume: number): void {
		if (this.#state.volume === null && Number.isFinite(volume)) {
			this.#update({ volume: clamp(volume, 0, 100) });
		}
	}

	/**
	 * Current playback position in seconds, extrapolated while playing.
	 */
	get position(): number {
		const elapsed = this.#state.status === "playing" ? (Date.now() - this.#positionUpdatedAt) / 1000 : 0;
		const position = this.#position + elapsed;
		return this.#state.song ? Math.min(position, this.#state.song.duration) : position;
	}

	get #address(): string {
		return `${this.#settings.host}:${this.#settings.port}`;
	}

	/**
	 * Applies new connection settings; empty values fall back to {@link DEFAULT_CONNECTION}. Reconnects when anything changed.
	 */
	configure(settings: ConnectionSettings): void {
		const next = {
			host: settings.host?.trim() || DEFAULT_CONNECTION.host,
			port: String(settings.port ?? "").trim() || String(DEFAULT_CONNECTION.port),
			username: settings.username || DEFAULT_CONNECTION.username,
			password: settings.password || DEFAULT_CONNECTION.password,
		};

		const changed = (Object.keys(next) as (keyof typeof next)[]).some((key) => next[key] !== this.#settings[key]);
		this.#settings = next;

		if (changed || !this.#started) {
			this.#started = true;
			this.reconnect();
		}
	}

	/**
	 * Drops the current connection (if any) and connects again immediately.
	 */
	reconnect(): void {
		this.#attempt = 0;
		this.#teardown();
		this.#connect(true);
	}

	/**
	 * Connects immediately when currently waiting for a retry; used when the user interacts with an action.
	 */
	wake(): void {
		if (this.#started && !this.#socket) {
			this.reconnect();
		}
	}

	togglePlayPause(): boolean {
		const playing = this.#state.status === "playing";
		if (!this.#send({ event: playing ? "pause" : "play" })) {
			return false;
		}

		// Feishin confirms with a `playback` event; update right away so the key feels responsive.
		this.#update({ status: playing ? "paused" : "playing" });
		return true;
	}

	next(): boolean {
		return this.#send({ event: "next" });
	}

	previous(): boolean {
		return this.#send({ event: "previous" });
	}

	toggleShuffle(): boolean {
		return this.#send({ event: "shuffle" });
	}

	toggleRepeat(): boolean {
		return this.#send({ event: "repeat" });
	}

	/**
	 * Sets the volume (0 - 100).
	 */
	setVolume(volume: number): boolean {
		const value = Math.round(clamp(volume, 0, 100));
		if (!this.#send({ event: "volume", volume: value })) {
			return false;
		}

		this.#volumeSettleUntil = Date.now() + SETTLE_MS;
		this.#update({ volume: value });
		return true;
	}

	/**
	 * Changes the volume relative to the current level; fails while the level is unknown, rather than guessing and
	 * suddenly making it much louder or quieter.
	 */
	changeVolume(delta: number): boolean {
		return this.#state.volume !== null && this.setVolume(this.#state.volume + delta);
	}

	/**
	 * Feishin's remote API has no mute command, so muting sets the volume to 0 and unmuting restores the previous volume.
	 */
	toggleMute(): boolean {
		const volume = this.#state.volume;
		if (volume !== 0) {
			if (!this.setVolume(0)) {
				return false;
			}

			this.#volumeBeforeMute = volume ?? undefined;
			return true;
		}

		return this.setVolume(this.#volumeBeforeMute ?? 50);
	}

	/**
	 * Jumps forwards (positive) or backwards (negative) by the given number of seconds.
	 */
	seekBy(seconds: number): boolean {
		const song = this.#state.song;
		if (!song) {
			return false;
		}

		// Stop just short of the end so seeking forward never skips to the next track by accident.
		const max = Math.max(0, song.duration - 1);
		const position = clamp(this.position + seconds, 0, max);
		if (!this.#send({ event: "position", position })) {
			return false;
		}

		this.#setPosition(position);
		this.#seekSettleUntil = Date.now() + SETTLE_MS;
		this.emit("state", new Set(["position"]));
		return true;
	}

	/**
	 * @param announce Whether to report the `connecting` status; background retries keep showing the last failure instead,
	 * so the status doesn't flicker every few seconds while Feishin is closed.
	 */
	#connect(announce = false): void {
		clearTimeout(this.#reconnectTimer);
		this.#reconnectTimer = undefined;
		this.#lastError = undefined;
		if (announce || this.#connection.status === "connected") {
			this.#setConnection({ status: "connecting", address: this.#address });
		}

		let socket: WebSocket;
		try {
			socket = new WebSocket(`ws://${formatHost(this.#settings.host)}:${this.#settings.port}`, {
				handshakeTimeout: HANDSHAKE_TIMEOUT_MS,
			});
		} catch (err) {
			// Invalid host / port, e.g. "local host" or port "abc".
			this.#setConnection({ status: "error", address: this.#address, detail: (err as Error).message });
			this.#scheduleReconnect(RECONNECT_DELAYS_MS.at(-1)!);
			return;
		}

		this.#socket = socket;

		socket.on("open", () => {
			// Feishin skips authentication when no username / password is configured; the message is ignored then.
			const credentials = Buffer.from(`${this.#settings.username}:${this.#settings.password}`).toString("base64");
			socket.send(JSON.stringify({ event: "authenticate", header: `Basic ${credentials}` }));
		});

		socket.on("message", (data) => {
			try {
				this.#handleMessage(JSON.parse(data.toString()));
			} catch {
				// Ignore anything that isn't JSON.
			}
		});

		socket.on("error", (err) => {
			this.#lastError = err;
		});

		socket.on("close", (code) => {
			if (this.#socket !== socket) {
				return; // Replaced by a newer connection.
			}

			this.#socket = undefined;
			this.#handleClose(code);
		});
	}

	#handleClose(code: number): void {
		const address = this.#address;

		if (code === CLOSE_AUTH_FAILED) {
			this.#setConnection({ status: "unauthorized", address });
			this.#scheduleReconnect(UNAUTHORIZED_RETRY_MS);
			return;
		}

		if (code === CLOSE_SETTINGS_CHANGED) {
			// The remote settings were changed in Feishin (e.g. a new password); try again with a fresh connection.
			this.#attempt = 0;
			this.#setConnection({ status: "connecting", address });
			this.#scheduleReconnect(500);
			return;
		}

		const error = this.#lastError;
		const refused =
			(error?.code !== undefined && UNREACHABLE_ERRORS.has(error.code)) ||
			error?.message.includes("handshake has timed out") === true;
		if (code === CLOSE_SERVER_SHUTDOWN || refused || this.#connection.status !== "connected") {
			this.#setConnection(
				refused || !error || code === CLOSE_SERVER_SHUTDOWN
					? { status: "unreachable", address }
					: { status: "error", address, detail: error.message },
			);
		} else {
			// The connection dropped unexpectedly; reconnect quietly.
			this.#setConnection({ status: "connecting", address });
		}

		this.#scheduleReconnect(RECONNECT_DELAYS_MS[Math.min(this.#attempt++, RECONNECT_DELAYS_MS.length - 1)]);
	}

	#handleMessage(message: { event?: string; data?: unknown }): void {
		switch (message.event) {
			case "state": {
				// Sent once authenticated. Apply the state before reporting the connection, so actions never render the
				// state from before a reconnect as "connected".
				const data = (message.data ?? {}) as Record<string, unknown>;
				this.#attempt = 0;
				if (typeof data.position === "number") {
					this.#setPosition(data.position);
				}

				this.#update(
					{
						status: parseStatus(data.status) ?? this.#state.status,
						repeat: parseRepeat(data.repeat) ?? this.#state.repeat,
						shuffle: typeof data.shuffle === "boolean" ? data.shuffle : this.#state.shuffle,
						// Missing until the volume changed since Feishin started; keep what we know.
						volume: typeof data.volume === "number" ? data.volume : this.#state.volume,
						song: parseSong(data.song),
					},
					true,
				);
				this.#setConnection({ status: "connected", address: this.#address });
				break;
			}

			case "playback": {
				const status = parseStatus(message.data);
				if (status) {
					// Re-anchor the extrapolated position so pausing doesn't lose the time played since the last update.
					this.#setPosition(this.position);
					this.#update({ status });
				}
				break;
			}

			case "song":
				this.#setPosition(0);
				this.#update({ song: parseSong(message.data) }, true);
				break;

			case "position":
				if (typeof message.data === "number" && Date.now() >= this.#seekSettleUntil) {
					this.#setPosition(message.data);
					this.emit("state", new Set(["position"]));
				}
				break;

			case "volume":
				if (typeof message.data === "number" && Date.now() >= this.#volumeSettleUntil) {
					this.#update({ volume: message.data });
				}
				break;

			case "shuffle":
				if (typeof message.data === "boolean") {
					this.#update({ shuffle: message.data });
				}
				break;

			case "repeat": {
				const repeat = parseRepeat(message.data);
				if (repeat) {
					this.#update({ repeat });
				}
				break;
			}
		}
	}

	#update(partial: Partial<PlayerState>, includePosition = false): void {
		const changes = new Set<StateChange>(includePosition ? ["position"] : []);
		for (const key of Object.keys(partial) as (keyof PlayerState)[]) {
			const value = partial[key];
			if (JSON.stringify(value) !== JSON.stringify(this.#state[key])) {
				changes.add(key);
			}
		}

		Object.assign(this.#state, partial);
		if (changes.size > 0) {
			this.emit("state", changes);
		}
	}

	#setPosition(position: number): void {
		this.#position = position;
		this.#positionUpdatedAt = Date.now();
	}

	#setConnection(connection: ConnectionInfo): void {
		const previous = this.#connection;
		this.#connection = connection;
		if (
			previous.status !== connection.status ||
			previous.address !== connection.address ||
			previous.detail !== connection.detail
		) {
			this.emit("connection", connection);
		}
	}

	#send(message: Record<string, unknown>): boolean {
		if (!this.isConnected || this.#socket?.readyState !== WebSocket.OPEN) {
			return false;
		}

		this.#socket.send(JSON.stringify(message));
		return true;
	}

	#scheduleReconnect(delay: number): void {
		clearTimeout(this.#reconnectTimer);
		this.#reconnectTimer = setTimeout(() => this.#connect(), delay);
	}

	#teardown(): void {
		clearTimeout(this.#reconnectTimer);
		this.#reconnectTimer = undefined;

		const socket = this.#socket;
		this.#socket = undefined;
		if (socket) {
			socket.removeAllListeners();
			socket.on("error", () => {}); // Errors can still surface while closing.
			socket.terminate();
		}
	}
}

const UNREACHABLE_ERRORS = new Set(["ECONNREFUSED", "ECONNRESET", "EHOSTUNREACH", "ENETUNREACH", "ENOTFOUND", "EAI_AGAIN", "ETIMEDOUT"]);

function clamp(value: number, min: number, max: number): number {
	return Math.min(Math.max(value, min), max);
}

/**
 * Wraps IPv6 addresses in brackets so they can be used in a URL.
 */
function formatHost(host: string): string {
	return host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
}

function parseStatus(value: unknown): PlaybackStatus | undefined {
	// Older Feishin versions sent upper-case values, or a boolean.
	if (typeof value === "boolean") {
		return value ? "playing" : "paused";
	}

	const status = typeof value === "string" ? value.toLowerCase() : undefined;
	return status === "playing" || status === "paused" || status === "stopped" ? status : undefined;
}

function parseRepeat(value: unknown): RepeatMode | undefined {
	const repeat = typeof value === "string" ? value.toLowerCase() : undefined;
	return repeat === "none" || repeat === "all" || repeat === "one" ? repeat : undefined;
}

function parseSong(value: unknown): Song | null {
	if (!value || typeof value !== "object") {
		return null;
	}

	const song = value as Record<string, unknown>;
	return {
		name: typeof song.name === "string" ? song.name : "",
		artist: typeof song.artistName === "string" ? song.artistName : "",
		// Feishin reports the duration in milliseconds.
		duration: typeof song.duration === "number" ? song.duration / 1000 : 0,
	};
}

export const feishin = new FeishinClient();
