// Polyfills for Node.js globals expected by some browserified dependencies.
// Must be imported BEFORE any library that requires them.

// `process` polyfill (minimal)
if (typeof globalThis.process === 'undefined') {
  // eslint-disable-next-line @typescript-eslint/ban-ts-comment
  // @ts-ignore
  globalThis.process = { env: {} };
}
