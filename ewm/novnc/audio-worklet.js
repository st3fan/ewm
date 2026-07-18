// EWM's audio output worklet (notes/VNC.md §4.4): Float32 PCM chunks arrive
// over the port from audio.js; this processor plays them back to back and
// emits silence on underrun — which, given the server's decay-to-silence
// speaker model, is what the signal was heading toward anyway.
class PcmPlayer extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = [];
    this.offset = 0;
    // ~8 chunks ≈ 200 ms at the server's frame cadence: a shallow buffer so
    // a backgrounded tab drops stale audio instead of building latency.
    this.port.onmessage = (e) => {
      this.queue.push(e.data);
      while (this.queue.length > 8) {
        this.queue.shift();
        this.offset = 0;
      }
    };
  }

  process(_inputs, outputs) {
    const out = outputs[0][0];
    let i = 0;
    while (i < out.length && this.queue.length > 0) {
      const chunk = this.queue[0];
      const n = Math.min(out.length - i, chunk.length - this.offset);
      out.set(chunk.subarray(this.offset, this.offset + n), i);
      i += n;
      this.offset += n;
      if (this.offset >= chunk.length) {
        this.queue.shift();
        this.offset = 0;
      }
    }
    out.fill(0, i); // underrun: silence
    return true;
  }
}

registerProcessor('pcm-player', PcmPlayer);
