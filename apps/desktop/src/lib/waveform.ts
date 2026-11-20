/** Peak per bucket of the downmixed channels, scaled so the loudest bucket is 1 (silence stays all zeros). */
export function bucketPeaks(channels: ArrayLike<number>[], buckets = 48): number[] {
  const length = channels.reduce((n, c) => Math.max(n, c.length), 0);
  if (length === 0 || buckets <= 0) return [];
  const out = new Array<number>(buckets).fill(0);
  for (let b = 0; b < buckets; b += 1) {
    const from = Math.floor((b * length) / buckets);
    const to = Math.max(from + 1, Math.floor(((b + 1) * length) / buckets));
    let peak = 0;
    for (let i = from; i < to; i += 1) {
      let sum = 0;
      for (const c of channels) sum += c[i] ?? 0;
      const v = Math.abs(sum / channels.length);
      if (v > peak) peak = v;
    }
    out[b] = peak;
  }
  const max = Math.max(...out);
  return max > 0 ? out.map((v) => v / max) : out;
}

/** How many bars the playback position has passed (the bars before it are drawn highlighted). */
export function playedBars(currentTime: number, duration: number, bars: number): number {
  if (!(duration > 0) || bars <= 0) return 0;
  return Math.min(bars, Math.max(0, Math.round((currentTime / duration) * bars)));
}

const cache = new Map<string, number[]>();

/** Decode the audio and reduce it to bars; cached per message id. Rejects when the browser cannot decode it. */
export async function waveformFor(id: string, data: ArrayBuffer, buckets = 48): Promise<number[]> {
  const hit = cache.get(id);
  if (hit) return hit;
  const ctx = new OfflineAudioContext(1, 1, 44100);
  const audio = await ctx.decodeAudioData(data);
  const peaks = bucketPeaks(
    Array.from({ length: audio.numberOfChannels }, (_, i) => audio.getChannelData(i)),
    buckets,
  );
  if (peaks.length === 0) throw new Error('empty audio');
  cache.set(id, peaks);
  return peaks;
}
