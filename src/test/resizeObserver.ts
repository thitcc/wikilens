// Controllable ResizeObserver stub — jsdom ships none, and App's
// window-height reporter constructs one at mount, so every App suite needs
// this installed (setup.ts) before render. Tests that exercise the reporter
// drive it with fireResizeObservers(); everything else just needs the
// constructor to exist.

export class ResizeObserverStub {
  static instances: ResizeObserverStub[] = [];
  readonly targets = new Set<Element>();

  constructor(readonly callback: ResizeObserverCallback) {
    ResizeObserverStub.instances.push(this);
  }

  observe(el: Element) {
    this.targets.add(el);
  }

  unobserve(el: Element) {
    this.targets.delete(el);
  }

  disconnect() {
    this.targets.clear();
  }
}

export function installResizeObserver(): void {
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver =
    ResizeObserverStub;
}

/** Forget observers from the previous test (instances outlive cleanup()). */
export function resetResizeObservers(): void {
  ResizeObserverStub.instances.length = 0;
}

/** Fire every live observer's callback. Entries stay empty on purpose — the
 * app's reporter re-reads the DOM itself rather than trusting entries. */
export function fireResizeObservers(): void {
  for (const observer of ResizeObserverStub.instances) {
    observer.callback([], observer as unknown as ResizeObserver);
  }
}
