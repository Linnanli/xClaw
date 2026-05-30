import { createBrowserRouter } from "react-router";
import { PasswordSetup } from "./components/auth/PasswordSetup";
import { PasswordLogin } from "./components/auth/PasswordLogin";
import { MainApp } from "./components/main/MainApp";

export const router = createBrowserRouter([
  {
    path: "/",
    Component: PasswordLogin,
  },
  {
    path: "/setup",
    Component: PasswordSetup,
  },
  {
    path: "/app",
    Component: MainApp,
  },
]);
