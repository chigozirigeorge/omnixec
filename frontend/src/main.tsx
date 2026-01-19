// Polyfills for Node.js globals used by wallet-selector bundles.
import './polyfills/node-globals';
import { createRoot } from 'react-dom/client';
import App from "./App.tsx";
import "./index.css";
import { ErrorBoundary } from './components/ErrorBoundary.tsx';

// Log environment info
console.log('🚀 Starting OmniXec frontend...');
console.log('Environment:', import.meta.env.MODE);
console.log('Base URL:', import.meta.env.BASE_URL);

// Get root element
const rootElement = document.getElementById("root");

// Error boundary fallback UI
const ErrorFallback = ({ error }: { error: Error }) => (
  <div style={{ padding: '20px', color: 'red' }}>
    <h1>Something went wrong</h1>
    <p>{error.message}</p>
    <pre>{error.stack}</pre>
  </div>
);

// Handle missing root element
if (!rootElement) {
  const errorMessage = 'Root element with id "root" not found in the DOM';
  console.error(errorMessage);
  document.body.innerHTML = `
    <div style="font-family: Arial, sans-serif; padding: 20px; color: #ff4444;">
      <h1>Application Error</h1>
      <p>${errorMessage}</p>
      <p>Please check if the HTML file contains an element with id="root".</p>
    </div>
  `;
} else {
  try {
    // Create root and render app
    console.log('Creating React root...');
    const root = createRoot(rootElement);
    
    // Render the app with error boundary
    console.log('Rendering App...');
    root.render(
      <ErrorBoundary FallbackComponent={ErrorFallback}>
        <App />
      </ErrorBoundary>
    );
    
    console.log('✅ App rendered successfully');
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    const errorStack = error instanceof Error ? error.stack : String(error);
    
    console.error('❌ Error rendering App:', error);
    
    // Render error to the page
    rootElement.innerHTML = `
      <div style="font-family: Arial, sans-serif; padding: 20px; color: #ff4444;">
        <h1>Critical Error</h1>
        <p>${errorMessage}</p>
        <pre style="white-space: pre-wrap; background: #f8f8f8; padding: 10px; border-radius: 4px;">
          ${errorStack}
        </pre>
        <p>Check the browser console for more details.</p>
      </div>
    `;
    
    // Re-throw in development for better error overlay
    if (import.meta.env.DEV) {
      throw error;
    }
  }
}

// Log successful initialization
console.log('Application initialized in', import.meta.env.MODE, 'mode');
