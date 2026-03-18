
  import { createRoot } from "react-dom/client";
  import App from "./app/App.tsx";
  import "./styles/index.css";
  
  // 导入 DLP 诊断工具（在浏览器控制台中可用）
  import "./app/utils/dlpDiagnostics";

  createRoot(document.getElementById("root")!).render(<App />);
  