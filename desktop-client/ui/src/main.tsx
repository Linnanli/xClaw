import { createRoot } from "react-dom/client";
import App from "./app/App.tsx";
import "./styles/index.css";

async function bootstrap() {
	if ((window as Window & { Cypress?: unknown }).Cypress) {
		const { setupCypressTauriMock } = await import('./app/testing/cypressTauriMock');
		setupCypressTauriMock();
	}

	if (import.meta.env.DEV) {
		const { installE2EChatStreamCapture } = await import('./app/testing/e2eChatStreamCapture');
		await installE2EChatStreamCapture();
	}

	createRoot(document.getElementById("root")!).render(<App />);
}

void bootstrap();
