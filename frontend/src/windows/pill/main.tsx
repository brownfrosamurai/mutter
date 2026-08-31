import { createRoot } from "react-dom/client";
import { Pill } from "./Pill";
import "@/styles/globals.css";

// Deliberately NOT wrapped in <StrictMode> — see the frontend-rewrite
// plan's Phase B note: this is the app's highest-risk native/JS
// interaction (native vibrancy masking via ResizeObserver ->
// set_pill_vibrancy_layout), and two of the pill's three documented
// pre-rewrite bugs were exactly a callback-firing-more-than-once race.
// StrictMode's dev-mode double-invoke of effects is precisely that hazard
// class; Pill.tsx's own rAF-debounce (mirroring the original vanilla JS)
// is real belt-and-suspenders defense, but there's no upside to also
// fighting StrictMode here on top of it.
// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
createRoot(document.getElementById("root")!).render(<Pill />);
