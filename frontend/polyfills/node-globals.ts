
import { Buffer } from 'buffer';

// Add Buffer to the global scope
if (typeof window !== 'undefined') {
  (window as any).Buffer = Buffer;
  (window as any).global = window;
  (window as any).process = {
    env: { DEBUG: undefined },
    version: '',
    nextTick: (callback: () => void) => setTimeout(callback, 0)
  };
}

// Import other necessary polyfills
import 'crypto-browserify';
import 'stream-browserify';
import 'process/browser';