/**
 * canvas.ts — Wizard canvas state machine
 *
 * The canvas is the primary spatial surface in Wizard. It holds typed cards
 * arranged at world-space coordinates, grouped into focus rooms.
 *
 * Responsibilities:
 *  - Maintain the full set of cards and their positions
 *  - Track the active focus room and the semantic zoom level
 *  - Provide the pan/zoom viewport transform
 *  - Dispatch canvas actions (add/remove/focus/move cards)
 *
 * This module is intentionally framework-agnostic; the SolidJS reactive layer
 * is a thin wrapper in main.ts.
 */

import { createSignal, createMemo, Accessor } from "solid-js";
import type { TypedCard, CardId, CardKind } from "./cards";
import type { FocusRoom, FocusRoomId } from "./focus-room";
import { ZOOM_LEVELS, ZoomLevel, zoomLevelFor } from "./semantic-zoom";

// ---------------------------------------------------------------------------
// Viewport
// ---------------------------------------------------------------------------

/** World-space offset + uniform scale that maps canvas coords → screen pixels. */
export interface Viewport {
  /** Horizontal pan in world units (positive = shifted right). */
  offsetX: number;
  /** Vertical pan in world units (positive = shifted down). */
  offsetY: number;
  /** Zoom scale factor. 1.0 = 100 %. */
  scale: number;
}

export const DEFAULT_VIEWPORT: Viewport = {
  offsetX: 0,
  offsetY: 0,
  scale: 1.0,
};

/** Clamp scale to sane min / max so cards never vanish or explode. */
export const SCALE_MIN = 0.08;
export const SCALE_MAX = 4.0;

// ---------------------------------------------------------------------------
// Canvas state
// ---------------------------------------------------------------------------

export interface CanvasState {
  /** All cards on the canvas, keyed by id. */
  cards: Map<CardId, TypedCard>;
  /** All focus rooms, keyed by id. */
  rooms: Map<FocusRoomId, FocusRoom>;
  /** Currently active focus room id (undefined = whole canvas visible). */
  activeFocusRoomId: FocusRoomId | undefined;
  /** Current viewport transform. */
  viewport: Viewport;
  /** Id of the card that currently holds keyboard focus, if any. */
  focusedCardId: CardId | undefined;
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

export type CanvasAction =
  | { type: "ADD_CARD"; card: TypedCard }
  | { type: "REMOVE_CARD"; cardId: CardId }
  | { type: "MOVE_CARD"; cardId: CardId; x: number; y: number }
  | { type: "FOCUS_CARD"; cardId: CardId | undefined }
  | { type: "SET_VIEWPORT"; viewport: Partial<Viewport> }
  | { type: "PAN"; dx: number; dy: number }
  | { type: "ZOOM"; scaleDelta: number; pivotX: number; pivotY: number }
  | { type: "ENTER_FOCUS_ROOM"; roomId: FocusRoomId }
  | { type: "EXIT_FOCUS_ROOM" }
  | { type: "ADD_ROOM"; room: FocusRoom }
  | { type: "REMOVE_ROOM"; roomId: FocusRoomId }
  | { type: "ASSIGN_CARD_TO_ROOM"; cardId: CardId; roomId: FocusRoomId | undefined };

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

export function canvasReducer(state: CanvasState, action: CanvasAction): CanvasState {
  switch (action.type) {
    case "ADD_CARD": {
      const cards = new Map(state.cards);
      cards.set(action.card.id, action.card);
      return { ...state, cards };
    }

    case "REMOVE_CARD": {
      const cards = new Map(state.cards);
      cards.delete(action.cardId);
      const focusedCardId =
        state.focusedCardId === action.cardId ? undefined : state.focusedCardId;
      return { ...state, cards, focusedCardId };
    }

    case "MOVE_CARD": {
      const existing = state.cards.get(action.cardId);
      if (!existing) return state;
      const cards = new Map(state.cards);
      cards.set(action.cardId, { ...existing, x: action.x, y: action.y });
      return { ...state, cards };
    }

    case "FOCUS_CARD":
      return { ...state, focusedCardId: action.cardId };

    case "SET_VIEWPORT": {
      const viewport = {
        ...state.viewport,
        ...action.viewport,
        scale: clampScale(action.viewport.scale ?? state.viewport.scale),
      };
      return { ...state, viewport };
    }

    case "PAN":
      return {
        ...state,
        viewport: {
          ...state.viewport,
          offsetX: state.viewport.offsetX + action.dx,
          offsetY: state.viewport.offsetY + action.dy,
        },
      };

    case "ZOOM": {
      const nextScale = clampScale(state.viewport.scale + action.scaleDelta);
      // Adjust offset so the zoom pivots around the screen point (pivotX, pivotY).
      const scaleRatio = nextScale / state.viewport.scale;
      const offsetX =
        action.pivotX - scaleRatio * (action.pivotX - state.viewport.offsetX);
      const offsetY =
        action.pivotY - scaleRatio * (action.pivotY - state.viewport.offsetY);
      return { ...state, viewport: { offsetX, offsetY, scale: nextScale } };
    }

    case "ENTER_FOCUS_ROOM": {
      const room = state.rooms.get(action.roomId);
      if (!room) return state;
      // Animate viewport to frame the room (instant for now; animation can wrap this).
      const viewport = viewportForRoom(room);
      return { ...state, activeFocusRoomId: action.roomId, viewport };
    }

    case "EXIT_FOCUS_ROOM":
      return { ...state, activeFocusRoomId: undefined };

    case "ADD_ROOM": {
      const rooms = new Map(state.rooms);
      rooms.set(action.room.id, action.room);
      return { ...state, rooms };
    }

    case "REMOVE_ROOM": {
      const rooms = new Map(state.rooms);
      rooms.delete(action.roomId);
      const activeFocusRoomId =
        state.activeFocusRoomId === action.roomId
          ? undefined
          : state.activeFocusRoomId;
      return { ...state, rooms, activeFocusRoomId };
    }

    case "ASSIGN_CARD_TO_ROOM": {
      const existing = state.cards.get(action.cardId);
      if (!existing) return state;
      const cards = new Map(state.cards);
      cards.set(action.cardId, { ...existing, roomId: action.roomId });
      return { ...state, cards };
    }

    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Derived selectors
// ---------------------------------------------------------------------------

/** Cards visible in the current focus room (or all cards if none active). */
export function visibleCards(state: CanvasState): TypedCard[] {
  const all = Array.from(state.cards.values());
  if (!state.activeFocusRoomId) return all;
  return all.filter((c) => c.roomId === state.activeFocusRoomId);
}

/** Semantic zoom level derived from the current scale. */
export function currentZoomLevel(state: CanvasState): ZoomLevel {
  return zoomLevelFor(state.viewport.scale);
}

// ---------------------------------------------------------------------------
// SolidJS reactive store
// ---------------------------------------------------------------------------

export interface CanvasStore {
  state: Accessor<CanvasState>;
  dispatch: (action: CanvasAction) => void;
  /** Derived: cards visible in the active focus room. */
  visibleCards: Accessor<TypedCard[]>;
  /** Derived: semantic zoom level. */
  zoomLevel: Accessor<ZoomLevel>;
}

function emptyState(): CanvasState {
  return {
    cards: new Map(),
    rooms: new Map(),
    activeFocusRoomId: undefined,
    viewport: DEFAULT_VIEWPORT,
    focusedCardId: undefined,
  };
}

/**
 * Create a reactive Wizard canvas store.
 *
 * Call once at the application root; pass the store down via context or props.
 */
export function createCanvasStore(initial?: Partial<CanvasState>): CanvasStore {
  const [state, setState] = createSignal<CanvasState>({
    ...emptyState(),
    ...initial,
  });

  function dispatch(action: CanvasAction) {
    setState((prev) => canvasReducer(prev, action));
  }

  const visible = createMemo(() => visibleCards(state()));
  const zoom = createMemo(() => currentZoomLevel(state()));

  return { state, dispatch, visibleCards: visible, zoomLevel: zoom };
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

function clampScale(s: number): number {
  return Math.min(SCALE_MAX, Math.max(SCALE_MIN, s));
}

/**
 * Compute a viewport that frames a focus room at a comfortable zoom level.
 * The room is centred with a small margin.
 */
function viewportForRoom(room: FocusRoom): Viewport {
  const MARGIN = 40; // px padding around room bounds
  const w = room.width + MARGIN * 2;
  const h = room.height + MARGIN * 2;

  // Target: fit room into an assumed 1280×800 viewport.
  const sceneW = 1280;
  const sceneH = 800;
  const scale = clampScale(Math.min(sceneW / w, sceneH / h));

  // Centre the room.
  const offsetX = (sceneW - room.width * scale) / 2 - room.x * scale;
  const offsetY = (sceneH - room.height * scale) / 2 - room.y * scale;

  return { offsetX, offsetY, scale };
}
