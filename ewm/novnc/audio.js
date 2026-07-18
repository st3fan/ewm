// EWM's speaker side-channel (notes/VNC.md §4): RFB has no audio, so the
// console page opens a second WebSocket on /audio — the same host and port
// that served the page — and plays the machine's 44.1 kHz mono i16 PCM
// through an AudioWorklet. Browsers refuse to start audio before a user
// gesture, so `arm()` is called from the page's first click or keypress
// (the same gesture that focuses the screen).

let context = null;
let workletGain = null;
let socket = null;
let muted = false;
let started = false;

// A small live-stats object, handy for debugging and the (future) level
// indicator: chunk/byte counters and whether anything non-silent arrived.
export const stats = { chunks: 0, bytes: 0, heardSignal: false };

/** Start the audio pipeline. Must be called from a user-gesture handler. */
export async function arm() {
  if (started) return;
  started = true;
  try {
    context = new AudioContext({ sampleRate: 44100 });
    await context.audioWorklet.addModule('audio-worklet.js');
    const player = new AudioWorkletNode(context, 'pcm-player', {
      outputChannelCount: [1],
    });
    workletGain = context.createGain();
    player.connect(workletGain);
    workletGain.connect(context.destination);
    workletGain.gain.value = muted ? 0 : 1;
    await context.resume();
    connect(player);
  } catch (e) {
    console.warn('EWM audio unavailable:', e);
  }
}

function connect(player) {
  const scheme = location.protocol === 'https:' ? 'wss://' : 'ws://';
  socket = new WebSocket(scheme + location.host + '/audio');
  socket.binaryType = 'arraybuffer';
  socket.onmessage = (e) => {
    if (typeof e.data === 'string') return; // the format header
    const pcm = new Int16Array(e.data);
    const f32 = new Float32Array(pcm.length);
    for (let i = 0; i < pcm.length; i++) {
      f32[i] = pcm[i] / 32768;
      if (pcm[i] !== 0) stats.heardSignal = true;
    }
    stats.chunks += 1;
    stats.bytes += e.data.byteLength;
    player.port.postMessage(f32, [f32.buffer]);
  };
}

/** Toggle mute; returns the new muted state. Arms on first use. */
export function toggleMute() {
  muted = !muted;
  if (workletGain) workletGain.gain.value = muted ? 0 : 1;
  return muted;
}

export function isMuted() {
  return muted;
}
