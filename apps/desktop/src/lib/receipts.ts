import type { DeviceReceipt } from './engineApi';

/** "Laptop: read 12:03", "Phone: delivered", "Tablet: waiting". */
export function receiptLine(r: DeviceReceipt, time: (ms: number) => string): string {
  const name = r.deviceName?.trim() || `Device ${r.deviceId.slice(0, 4)}`;
  if (r.state === 'waiting') return `${name}: waiting`;
  return `${name}: ${r.state}${r.at > 0 && r.state === 'read' ? ` ${time(r.at)}` : ''}`;
}

/** Nearby-mode banner text: the hop count comes from the engine; no count means a direct link. */
export function nearbyText(name: string, hops: number | undefined): string {
  const via = hops && hops > 0 ? `through ${hops} nearby device${hops === 1 ? '' : 's'}` : 'directly';
  return `Nearby mode: no internet needed. This chat is going over Bluetooth, ${via} to ${name}'s device.`;
}
