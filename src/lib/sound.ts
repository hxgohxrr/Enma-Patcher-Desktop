
let ctx: AudioContext | null = null;
let enabled = true;

export type MelodyMode = "android" | "ios" | "default";

let mode: MelodyMode = "default";

export function setMelody(m: MelodyMode) {
  mode = m;
}

const ANDROID_MOTIFS: number[][] = [
  [523.25, 659.25],
  [587.33, 739.99],
  [659.25, 783.99],
  [440.0, 554.37],
];

const IOS_MOTIFS: number[][] = [
  [880.0, 1174.66],
  [783.99, 1046.5],
  [1046.5, 1318.5],
  [987.77, 1174.66],
];

export function setSoundEnabled(v: boolean) {
  enabled = v;
}

function ac(): AudioContext | null {
  try {
    if (!ctx) {
      const AC = window.AudioContext;
      if (!AC) return null;
      ctx = new AC();
    }
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

export function unlockAudio() {
  ac();
}

function tone(
  freq: number,
  durMs: number,
  type: OscillatorType,
  gain: number,
  delayMs = 0,
  slideTo?: number
) {
  if (!enabled) return;
  const c = ac();
  if (!c) return;
  try {
    const t0 = c.currentTime + delayMs / 1000;
    const osc = c.createOscillator();
    const g = c.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t0);
    if (slideTo) osc.frequency.exponentialRampToValueAtTime(slideTo, t0 + durMs / 1000);
    g.gain.setValueAtTime(0.0001, t0);
    g.gain.exponentialRampToValueAtTime(gain, t0 + 0.012);
    g.gain.exponentialRampToValueAtTime(0.0001, t0 + durMs / 1000);
    osc.connect(g).connect(c.destination);
    osc.start(t0);
    osc.stop(t0 + durMs / 1000 + 0.05);
  } catch {
  }
}

export const sound = {
  tab() {
    tone(620, 70, "sine", 0.05);
    tone(880, 80, "sine", 0.04, 45);
  },
  toggle(on: boolean) {
    if (on) tone(740, 70, "triangle", 0.055);
    else tone(520, 70, "triangle", 0.05);
  },
  press() {
    const motifs = mode === "ios" ? IOS_MOTIFS : ANDROID_MOTIFS;
    const motif = motifs[Math.floor(Math.random() * motifs.length)];
    const type = mode === "ios" ? "sine" : "triangle";
    const base = mode === "ios" ? 0.045 : 0.05;
    motif.forEach((f, i) => {
      const cents = Math.random() * 36 - 18;
      tone(f * Math.pow(2, cents / 1200), 110, type, base, i * 65);
    });
  },
  done() {
    tone(880, 120, "sine", 0.05);
    tone(1174, 140, "sine", 0.04, 70);
  },
  success() {
    tone(659, 160, "sine", 0.06);
    tone(784, 160, "sine", 0.06, 90);
    tone(1046, 260, "sine", 0.055, 180);
  },
  error() {
    tone(220, 180, "sawtooth", 0.03, 0, 150);
  },
};
