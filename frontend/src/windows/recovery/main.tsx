import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Recovery } from "./Recovery";
import "@/styles/globals.css";

// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Recovery />
  </StrictMode>,
);
