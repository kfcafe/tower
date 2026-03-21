/**
 * semantic-zoom.ts — Semantic zoom level model for the Wizard canvas
 *
 * Semantic zoom means that as the user zooms out, cards progressively collapse
 * to lower-fidelity representations rather than simply shrinking the same
 * pixels. This keeps the canvas readable at any scale.
 *
 * Zoom levels (from most zoomed-in to most zoomed-out)
 * -------------------------------------------------------
 *
 *   DETAIL   (scale ≥ 0.70) — Full card: all fields, actions, log tail.
 *   NORMAL   (scale ≥ 0.35) — Standard card: title, status badge, key stats.
 *   SUMMARY  (scale ≥ 0.14) — Compact chip: title + status dot only.
 *   OVERVIEW (scale  < 0.14) — Room portals replace individual cards;
 *                              only FocusRoom bounding boxes + labels visible.
 *
 * Implementation note
 * -------------------
 * The SemanticZoom context is consumed by every card renderer to decide which
 * template to use. The canvas passes the current level through SolidJS context
 * so cards do not need to recompute it themselves.
 *
 * Staged features
 * ---------------
 * TODO(canvas-v2): per-card zoom overrides (pin a card at DETAIL regardless
 *   of global zoom — useful for "spotlight" workflows).
 * TODO(canvas-v2): animated crossfade between zoom levels instead of hard cut.
 */

// ---------------------------------------------------------------------------
// Level enum + thresholds
// ---------------------------------------------------------------------------

export type ZoomLevel = "detail" | "normal" | "summary" | "overview";

/**
 * Scale thresholds that define the lower bound for each zoom level.
 * `zoomLevelFor(scale)` uses these to map a viewport scale to a ZoomLevel.
 */
export const ZOOM_LEVELS: { level: ZoomLevel; minScale: number }[] = [
  { level: "detail", minScale: 0.70 },
  { level: "normal", minScale: 0.35 },
  { level: "summary", minScale: 0.14 },
  { level: "overview", minScale: 0.0 }, // catch-all
];

/**
 * Map a raw viewport scale factor to a ZoomLevel.
 *
 * @example
 *   zoomLevelFor(1.0) → "detail"
 *   zoomLevelFor(0.5) → "normal"
 *   zoomLevelFor(0.2) → "summary"
 *   zoomLevelFor(0.1) → "overview"
 */
export function zoomLevelFor(scale: number): ZoomLevel {
  for (const { level, minScale } of ZOOM_LEVELS) {
    if (scale >= minScale) return level;
  }
  return "overview";
}

// ---------------------------------------------------------------------------
// Per-level render decisions
// ---------------------------------------------------------------------------

/**
 * Whether individual cards should render at all, or be replaced by
 * room-level portal chips.
 */
export function shouldRenderCards(level: ZoomLevel): boolean {
  return level !== "overview";
}

/**
 * Whether to render full card body content (spec excerpt, log tail, etc.).
 */
export function shouldRenderCardBody(level: ZoomLevel): boolean {
  return level === "detail";
}

/**
 * Whether to render card action buttons (approve, move, etc.).
 */
export function shouldRenderCardActions(level: ZoomLevel): boolean {
  return level === "detail" || level === "normal";
}

/**
 * Whether to render status text labels or just colour dots.
 */
export function shouldRenderStatusLabel(level: ZoomLevel): boolean {
  return level !== "summary";
}

// ---------------------------------------------------------------------------
// Named scale presets
// ---------------------------------------------------------------------------

/**
 * Canonical scale values for programmatic navigation (keyboard shortcuts,
 * "reset zoom" button, etc.).
 */
export const SCALE_PRESETS = {
  /** Fit the active focus room to the viewport. Computed dynamically. */
  FIT_ROOM: null as null,
  /** Comfortable reading scale. */
  DETAIL: 1.0,
  /** Standard overview of a focus room. */
  NORMAL: 0.5,
  /** Summary view showing many cards at once. */
  SUMMARY: 0.25,
  /** Full canvas overview — rooms visible as portals. */
  OVERVIEW: 0.1,
} as const;

// ---------------------------------------------------------------------------
// SemanticZoom context helpers (SolidJS)
// ---------------------------------------------------------------------------

import { createContext, useContext } from "solid-js";

/** SolidJS context for the current zoom level. */
export const SemanticZoomContext = createContext<ZoomLevel>("normal");

/** Hook to read the current semantic zoom level inside any card component. */
export function useSemanticZoom(): ZoomLevel {
  return useContext(SemanticZoomContext);
}

// ---------------------------------------------------------------------------
// Zoom step helpers (for scroll wheel / keyboard zoom)
// ---------------------------------------------------------------------------

/** Discrete zoom-in step multiplier. */
export const ZOOM_IN_STEP = 1.15;
/** Discrete zoom-out step multiplier. */
export const ZOOM_OUT_STEP = 1 / ZOOM_IN_STEP;

/**
 * Compute the scale delta for a wheel event.
 * Normalises mouse wheel, trackpad pinch, and keyboard shortcuts to a
 * consistent scale-delta value suitable for CanvasAction.ZOOM.
 */
export function wheelToScaleDelta(
  deltaY: number,
  currentScale: number
): number {
  // Positive deltaY = scroll down = zoom out.
  const factor = deltaY < 0 ? ZOOM_IN_STEP : ZOOM_OUT_STEP;
  return currentScale * factor - currentScale;
}
