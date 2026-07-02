// Sound effects using the Web Audio API (no external file dependencies)
export const playSound = (type: string) => {
  try {
    const configStr = localStorage.getItem('vajra_sounds_config');
    const config = configStr
      ? JSON.parse(configStr)
      : { onComplete: true, onFail: false, onQueueStart: false };

    if (type === 'complete' && !config.onComplete) return;
    if (type === 'fail' && !config.onFail) return;
    if (type === 'queue' && !config.onQueueStart) return;

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const ctx = new (window.AudioContext || (window as any).webkitAudioContext)();

    if (type === 'complete') {
      // Play a happy arpeggio: C5, E5, G5, C6
      const notes = [523.25, 659.25, 783.99, 1046.5];
      const duration = 0.12;
      notes.forEach((freq, i) => {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);

        osc.type = 'sine';
        osc.frequency.setValueAtTime(freq, ctx.currentTime + i * duration);

        gain.gain.setValueAtTime(0.15, ctx.currentTime + i * duration);
        gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + (i + 1.2) * duration);

        osc.start(ctx.currentTime + i * duration);
        osc.stop(ctx.currentTime + (i + 1.5) * duration);
      });
    } else if (type === 'fail') {
      // Play a buzzer sound (dissonant low frequency)
      const osc1 = ctx.createOscillator();
      const osc2 = ctx.createOscillator();
      const gain = ctx.createGain();

      osc1.connect(gain);
      osc2.connect(gain);
      gain.connect(ctx.destination);

      osc1.type = 'sawtooth';
      osc2.type = 'sawtooth';

      osc1.frequency.setValueAtTime(130, ctx.currentTime);
      osc2.frequency.setValueAtTime(135, ctx.currentTime);

      gain.gain.setValueAtTime(0.2, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.4);

      osc1.start();
      osc2.start();
      osc1.stop(ctx.currentTime + 0.4);
      osc2.stop(ctx.currentTime + 0.4);
    } else if (type === 'queue') {
      // Play a short rising blip
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();

      osc.connect(gain);
      gain.connect(ctx.destination);

      osc.type = 'sine';
      osc.frequency.setValueAtTime(440, ctx.currentTime);
      osc.frequency.exponentialRampToValueAtTime(880, ctx.currentTime + 0.15);

      gain.gain.setValueAtTime(0.15, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.15);

      osc.start();
      osc.stop(ctx.currentTime + 0.15);
    }
  } catch (e) {
    console.error('Audio failed to play', e);
  }
};
