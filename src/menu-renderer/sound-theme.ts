//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to Kando, the cross-platform         //
//   | \_ |  | | \| |__/ |__|    pie menu. Read more on github.com/kando-menu/kando     //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: Simon Schneegans <code@simonschneegans.de>
// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

import { SoundThemeDescription, MenuInteractionType } from '../common';

/**
 * Plays menu interaction sounds using the browser's built-in Web Audio API. Buffers are
 * fetched lazily and cached in memory; when sound is disabled or the menu is hidden, the
 * AudioContext is not created or is suspended so idle overhead stays zero.
 */
export class SoundTheme {
  private description: SoundThemeDescription | null = null;
  private volume = 0.5;
  private reloadCounter = 0;
  private context: AudioContext | null = null;
  private buffers = new Map<string, Promise<AudioBuffer | null>>();
  private suspendTimer: ReturnType<typeof setTimeout> | null = null;
  private activeSources = 0;

  /** Loads the given sound theme description and clears previously cached buffers. */
  public loadDescription(description: SoundThemeDescription) {
    this.description = description;
    this.reloadCounter += 1;
    this.buffers.clear();
    if (description.id === 'none' && this.context) {
      const ctx = this.context;
      this.context = null;
      this.activeSources = 0;
      if (this.suspendTimer) {
        clearTimeout(this.suspendTimer);
        this.suspendTimer = null;
      }
      void ctx.close().catch(() => {});
    }
  }

  /** Sets the overall playback volume (0.0 to 1.0). */
  public setVolume(volume: number) {
    this.volume = Math.max(0, Math.min(1, volume));
  }

  /** Suspends the audio context shortly after the menu closes to save resources. */
  public scheduleSuspend(delayMs = 600) {
    if (!this.context) {
      return;
    }
    if (this.suspendTimer) {
      clearTimeout(this.suspendTimer);
    }
    this.suspendTimer = setTimeout(() => {
      this.suspendTimer = null;
      if (this.activeSources > 0) {
        // Wait for any longer custom sound clip to finish before suspending.
        this.scheduleSuspend(400);
        return;
      }
      if (this.context && this.context.state === 'running') {
        void this.context.suspend().catch(() => {});
      }
    }, delayMs);
  }

  /** Plays the sound associated with the given interaction, if configured. */
  public playSound(type: MenuInteractionType) {
    if (!this.description || this.description.id === 'none' || this.volume <= 0) {
      return;
    }
    const sound = this.description.sounds?.[type];
    if (!sound || !sound.file || !this.description.directory) {
      return;
    }

    const volume = (sound.volume ?? 1) * this.volume;
    if (volume <= 0) {
      return;
    }

    if (this.suspendTimer) {
      clearTimeout(this.suspendTimer);
      this.suspendTimer = null;
    }

    const ctx = this.ensureContext();
    if (!ctx) {
      return;
    }

    const src = `${this.description.directory}/${encodeURIComponent(this.description.id)}/${sound.file}?v=${this.reloadCounter}`;
    const minRate = sound.minPitch ?? 1;
    const maxRate = sound.maxPitch ?? 1;
    const rate =
      minRate === maxRate ? minRate : Math.random() * (maxRate - minRate) + minRate;

    const resumePromise =
      ctx.state === 'suspended' ? ctx.resume().catch(() => {}) : Promise.resolve();

    void Promise.all([resumePromise, this.loadBuffer(ctx, src)]).then(([, buffer]) => {
      if (!buffer || this.context !== ctx) {
        return;
      }
      try {
        const source = ctx.createBufferSource();
        source.buffer = buffer;
        source.playbackRate.value = rate;
        const gain = ctx.createGain();
        gain.gain.value = volume;
        source.connect(gain);
        gain.connect(ctx.destination);
        this.activeSources += 1;
        source.onended = () => {
          this.activeSources = Math.max(0, this.activeSources - 1);
        };
        source.start(0);
      } catch {
        // Ignore transient Web Audio errors.
      }
    });
  }

  private ensureContext(): AudioContext | null {
    if (!this.context && typeof AudioContext !== 'undefined') {
      this.context = new AudioContext();
    }
    return this.context;
  }

  private loadBuffer(ctx: AudioContext, url: string): Promise<AudioBuffer | null> {
    let pending = this.buffers.get(url);
    if (!pending) {
      pending = fetch(url)
        .then((response) => (response.ok ? response.arrayBuffer() : null))
        .then((data) => (data ? ctx.decodeAudioData(data) : null))
        .catch((): AudioBuffer | null => null)
        .then((buffer) => {
          if (!buffer) {
            this.buffers.delete(url);
          }
          return buffer;
        });
      this.buffers.set(url, pending);
    }
    return pending;
  }
}
