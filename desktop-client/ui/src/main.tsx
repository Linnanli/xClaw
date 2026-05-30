import { createRoot } from "react-dom/client";
import App from "./app/App.tsx";
import "./styles/index.css";

async function bootstrap() {
	if ((window as Window & { Cypress?: unknown }).Cypress) {
		const { setupCypressTauriMock } = await import('./app/testing/cypressTauriMock');
		setupCypressTauriMock();
	}

	createRoot(document.getElementById("root")!).render(<App />);
}

void bootstrap();
