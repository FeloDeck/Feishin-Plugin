/// <reference path="libs/js/action.js" />
/// <reference path="libs/js/stream-deck.js" />

const FEISHIN_WS_URL = 'ws://localhost:4333'; // Default URL, make this configurable
let feishinWs = null;
let currentPlaybackStatus = 'PAUSED'; // Default to paused

const playPauseAction = new Action('de.felitendo.feishin.playpause');
const nextAction = new Action('de.felitendo.feishin.next');
const previousAction = new Action('de.felitendo.feishin.previous');
const shuffleAction = new Action('de.felitendo.feishin.shuffle');
const repeatAction = new Action('de.felitendo.feishin.repeat');

/**
 * The first event fired when Stream Deck starts
 */
$SD.onConnected(({ actionInfo, appInfo, connection, messageType, port, uuid }) => {
    console.log('Stream Deck connected!');
    connectToFeishin();
});

function connectToFeishin() {
    feishinWs = new WebSocket(FEISHIN_WS_URL);

    feishinWs.onopen = function() {
        console.log('Connected to Feishin');
        // Authenticate to Feishin Client
        authenticate('feishin', 'streamdeck');
    };

    feishinWs.onmessage = function(event) {
        const data = JSON.parse(event.data);
        handleFeishinMessage(data);
    };

    feishinWs.onerror = function(error) {
        console.error('Feishin WebSocket Error:', error);
    };

    feishinWs.onclose = function() {
        console.log('Disconnected from Feishin');
        // Attempt to reconnect after a delay
        setTimeout(connectToFeishin, 5000);
    };
}

function authenticate(username, password) {
    const auth = btoa(`${username}:${password}`);
    feishinWs.send(JSON.stringify({
        event: 'authenticate',
        header: `Basic ${auth}`
    }));
}

function handleFeishinMessage(data) {
    console.log('Received message from Feishin:', data);
    switch(data.event) {
        case 'state':
            updateAllButtons(data.data);
            break;
        case 'playback':
            updatePlayPauseButton(data.data);
            break;
        case 'shuffle':
            updateShuffleButton(data.data);
            break;
        case 'repeat':
            updateRepeatButton(data.data);
            break;
    }
}

function updateAllButtons(state) {
    updatePlayPauseButton(state.status);
    updateShuffleButton(state.shuffle);
    updateRepeatButton(state.repeat);
}

function updatePlayPauseButton(status) {
    console.log('Updating play/pause button with status:', status);
    if (typeof status === 'string') {
        currentPlaybackStatus = status.toUpperCase();
    } else if (typeof status === 'boolean') {
        currentPlaybackStatus = status ? 'PLAYING' : 'PAUSED';
    } else {
        console.error('Unexpected status type:', typeof status);
        return;
    }
    
    const isPlaying = currentPlaybackStatus === 'PLAYING';
    playPauseAction.setImage(isPlaying ? 'images/pause.png' : 'images/play.png');
    console.log('Updated play/pause button. Current status:', currentPlaybackStatus);
}

function updateShuffleButton(shuffleState) {
    shuffleAction.setImage(shuffleState ? 'images/shuffle_on.png' : 'images/shuffle_off.png');
}

function updateRepeatButton(repeatState) {
    switch(repeatState) {
        case 'NONE':
            repeatAction.setImage('images/repeat_off.png');
            break;
        case 'ALL':
            repeatAction.setImage('images/repeat_all.png');
            break;
        case 'ONE':
            repeatAction.setImage('images/repeat_one.png');
            break;
    }
}

playPauseAction.onKeyUp(({ action, context, device, event, payload }) => {
    if (feishinWs && feishinWs.readyState === WebSocket.OPEN) {
        const command = currentPlaybackStatus === 'PLAYING' ? 'pause' : 'play';
        feishinWs.send(JSON.stringify({ event: command }));
        console.log('Sent command to Feishin:', command);
        
        // Temporarily update the button state
        updatePlayPauseButton(command === 'play' ? 'PLAYING' : 'PAUSED');
    }
});

nextAction.onKeyUp(({ action, context, device, event, payload }) => {
    if (feishinWs && feishinWs.readyState === WebSocket.OPEN) {
        feishinWs.send(JSON.stringify({ event: 'next' }));
    }
});

previousAction.onKeyUp(({ action, context, device, event, payload }) => {
    if (feishinWs && feishinWs.readyState === WebSocket.OPEN) {
        feishinWs.send(JSON.stringify({ event: 'previous' }));
    }
});

shuffleAction.onKeyUp(({ action, context, device, event, payload }) => {
    if (feishinWs && feishinWs.readyState === WebSocket.OPEN) {
        feishinWs.send(JSON.stringify({ event: 'shuffle' }));
    }
});

repeatAction.onKeyUp(({ action, context, device, event, payload }) => {
    if (feishinWs && feishinWs.readyState === WebSocket.OPEN) {
        feishinWs.send(JSON.stringify({ event: 'repeat' }));
    }
});

// You can add more actions here as needed, such as volume control, etc.