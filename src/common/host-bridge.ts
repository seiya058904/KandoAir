//////////////////////////////////////////////////////////////////////////////////////////
//   _  _ ____ _  _ ___  ____                                                           //
//   |_/  |__| |\ | |  \ |  |    This file belongs to KandoAir, a lightweight Windows   //
//   | \_ |  | | \| |__/ |__|    port of Kando. Read more on github.com/kando-menu/kando //
//                                                                                      //
//////////////////////////////////////////////////////////////////////////////////////////

// SPDX-FileCopyrightText: KandoAir contributors
// SPDX-License-Identifier: MIT

/**
 * The WebView2 host object which is available in pages hosted by KandoAir. See
 * https://learn.microsoft.com/microsoft-edge/webview2/reference/javascript/webview
 */
type WebViewHost = {
  postMessage(message: unknown): void;
  postMessageWithAdditionalObjects?(message: unknown, objects: ArrayLike<unknown>): void;
  addEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
};

type HostMessage =
  | { k: 'reply'; id: number; ok: boolean; result?: unknown; error?: string }
  | { k: 'event'; ch: string; args: unknown[] };

type Listener = (...args: any[]) => void; // eslint-disable-line @typescript-eslint/no-explicit-any

/**
 * The bridge between the renderer and the native KandoAir host. It replaces Electron's
 * ipcRenderer: `invoke` sends a request and resolves with the reply, `send` fires and
 * forgets, and `on` subscribes to events sent by the host. The channel names are the IPC
 * channel names of Kando.
 */
class HostBridge {
  private nextId = 1;
  private pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (e: Error) => void }
  >();
  private listeners = new Map<string, Set<Listener>>();

  constructor() {
    this.host?.addEventListener('message', (event) => this.onMessage(event.data));
  }

  /** Sends a request to the host and resolves with its reply. */
  public invoke<T>(channel: string, ...args: unknown[]): Promise<T> {
    return this.request<T>(channel, args);
  }

  /** Like invoke, but passes files whose paths the host can read (e.g. dropped files). */
  public invokeWithFiles<T>(
    channel: string,
    files: File[],
    ...args: unknown[]
  ): Promise<T> {
    return this.request<T>(channel, args, files);
  }

  /** Sends a message to the host without waiting for a reply. */
  public send(channel: string, ...args: unknown[]) {
    this.post({ k: 'send', ch: channel, args });
  }

  /** Subscribes to an event of the host. Returns a function which unsubscribes again. */
  public on(channel: string, listener: Listener): () => void {
    let set = this.listeners.get(channel);
    if (!set) {
      set = new Set();
      this.listeners.set(channel, set);
    }
    set.add(listener);
    return () => set.delete(listener);
  }

  private get host(): WebViewHost | undefined {
    return (window as unknown as { chrome?: { webview?: WebViewHost } }).chrome?.webview;
  }

  private request<T>(channel: string, args: unknown[], files?: File[]): Promise<T> {
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      const message = { k: 'invoke', id, ch: channel, args };
      if (files && files.length > 0 && this.host?.postMessageWithAdditionalObjects) {
        this.host.postMessageWithAdditionalObjects(message, files);
      } else {
        this.post(message);
      }
    });
  }

  private post(message: unknown) {
    if (!this.host) {
      console.warn('KandoAir host is not available.', message);
      return;
    }
    this.host.postMessage(message);
  }

  private onMessage(message: HostMessage) {
    if (message.k === 'reply') {
      const pending = this.pending.get(message.id);
      if (pending) {
        this.pending.delete(message.id);
        if (message.ok) {
          pending.resolve(message.result);
        } else {
          pending.reject(new Error(message.error));
        }
      }
    } else if (message.k === 'event') {
      this.listeners.get(message.ch)?.forEach((listener) => listener(...message.args));
    }
  }
}

export const hostBridge = new HostBridge();
