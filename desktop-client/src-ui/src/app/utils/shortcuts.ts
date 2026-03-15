// Keyboard shortcuts management

export interface Shortcut {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
  action: () => void;
}

export class ShortcutManager {
  private shortcuts: Map<string, Shortcut> = new Map();

  register(shortcut: Shortcut) {
    const key = this.getShortcutKey(shortcut);
    this.shortcuts.set(key, shortcut);
  }

  unregister(shortcut: Shortcut) {
    const key = this.getShortcutKey(shortcut);
    this.shortcuts.delete(key);
  }

  handleKeyDown(event: KeyboardEvent) {
    const key = this.getEventKey(event);
    const shortcut = this.shortcuts.get(key);
    
    if (shortcut) {
      event.preventDefault();
      shortcut.action();
    }
  }

  private getShortcutKey(shortcut: Shortcut): string {
    const parts: string[] = [];
    if (shortcut.ctrl) parts.push('ctrl');
    if (shortcut.shift) parts.push('shift');
    if (shortcut.alt) parts.push('alt');
    if (shortcut.meta) parts.push('meta');
    parts.push(shortcut.key.toLowerCase());
    return parts.join('+');
  }

  private getEventKey(event: KeyboardEvent): string {
    const parts: string[] = [];
    if (event.ctrlKey) parts.push('ctrl');
    if (event.shiftKey) parts.push('shift');
    if (event.altKey) parts.push('alt');
    if (event.metaKey) parts.push('meta');
    parts.push(event.key.toLowerCase());
    return parts.join('+');
  }
}

// Common shortcuts
export const SHORTCUTS = {
  SAVE: { key: 's', ctrl: true },
  SEARCH: { key: 'f', ctrl: true },
  NEW_THREAD: { key: 'n', ctrl: true },
  LOCK_APP: { key: 'l', ctrl: true, shift: true },
  CLEAR_LOGS: { key: 'k', ctrl: true, shift: true },
  EXPORT: { key: 'e', ctrl: true, shift: true },
};
