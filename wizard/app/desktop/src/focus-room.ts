/**
 * focus-room.ts — Focus room model for the Wizard canvas
 *
 * A focus room is a named, bounded region of the canvas that acts as a
 * logical viewport — entering a room pans/zooms the canvas to that region
 * and filters the visible card set to only cards assigned to that room.
 *
 * Rooms are a first-class grouping primitive that replace traditional folder
 * trees or tab bars. They are spatial rather than hierarchical.
 *
 * Design constraints
 * ------------------
 * • Rooms are non-overlapping by convention (the layout engine does not
 *   enforce this, but the UX is confusing if they overlap).
 * • Rooms can contain any mix of card kinds.
 * • Cards are assigned to exactly one room; unassigned cards live in the
 *   "root" area outside all rooms.
 * • Rooms can be given a "kind" label that drives colour-coding and the
 *   default card grid snap.
 *
 * Staged features
 * ---------------
 * TODO(canvas-v2): room nesting (sub-rooms) — not in v1 to keep layout simple.
 * TODO(canvas-v2): room templates (e.g. "sprint board" preset).
 * TODO(canvas-v2): room membership for review sessions.
 */

export type FocusRoomId = string & { readonly __brand: "FocusRoomId" };

export function makeFocusRoomId(raw: string): FocusRoomId {
  return raw as FocusRoomId;
}

export function generateFocusRoomId(): FocusRoomId {
  return makeFocusRoomId(`room-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
}

// ---------------------------------------------------------------------------
// Room kinds — drive colour and layout defaults
// ---------------------------------------------------------------------------

/**
 * Room kinds are intentionally open-ended; the list below covers the common
 * Wizard use-cases but custom strings are accepted.
 */
export type FocusRoomKind =
  | "unit-cluster"      // A group of related work units
  | "runtime-monitor"   // Live agent / process view
  | "knowledge-base"    // Knowledge surface entries
  | "review-queue"      // Cards pending human review
  | "inbox"             // Unsorted / newly created cards
  | "archive"           // Done / cancelled items
  | (string & {});      // Escape hatch for project-specific kinds

// ---------------------------------------------------------------------------
// Room data
// ---------------------------------------------------------------------------

export interface FocusRoom {
  id: FocusRoomId;
  /** Short display label shown on the room header and room-portal cards. */
  label: string;
  /** Optional longer description shown in room inspector. */
  description?: string;
  /** Semantic kind — drives colour and defaults. */
  kind: FocusRoomKind;
  /** World-space bounding box. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** ISO creation timestamp. */
  createdAt: string;
  /** Ordered list of card ids explicitly pinned to this room. */
  pinnedCardIds: string[];
  /** Whether the room is collapsed in the overview (room-portal only mode). */
  collapsed: boolean;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function makeFocusRoom(
  label: string,
  kind: FocusRoomKind,
  geo: { x: number; y: number; width: number; height: number },
  overrides?: Partial<FocusRoom>
): FocusRoom {
  return {
    id: generateFocusRoomId(),
    label,
    kind,
    ...geo,
    createdAt: new Date().toISOString(),
    pinnedCardIds: [],
    collapsed: false,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Colour palette for room kinds
// ---------------------------------------------------------------------------

export function roomKindColor(kind: FocusRoomKind): string {
  const palette: Partial<Record<FocusRoomKind, string>> = {
    "unit-cluster": "#4a9eff",
    "runtime-monitor": "#ff6b6b",
    "knowledge-base": "#50c878",
    "review-queue": "#8b5cf6",
    inbox: "#ffa500",
    archive: "#555555",
  };
  return palette[kind] ?? "#888888";
}

// ---------------------------------------------------------------------------
// Focus-room list helpers
// ---------------------------------------------------------------------------

/**
 * Return an ordered list of rooms suitable for the sidebar / breadcrumb.
 * Archived rooms are pushed to the end.
 */
export function sortedRooms(rooms: Map<FocusRoomId, FocusRoom>): FocusRoom[] {
  const all = Array.from(rooms.values());
  const live = all.filter((r) => r.kind !== "archive");
  const archived = all.filter((r) => r.kind === "archive");
  return [...live, ...archived];
}

/**
 * Return the label that should appear in the breadcrumb / status bar.
 * Falls back to "Canvas" when no room is active.
 */
export function activeRoomLabel(
  rooms: Map<FocusRoomId, FocusRoom>,
  activeId: FocusRoomId | undefined
): string {
  if (!activeId) return "Canvas";
  return rooms.get(activeId)?.label ?? "Canvas";
}

// ---------------------------------------------------------------------------
// Pre-built room presets — used when bootstrapping a new project canvas
// ---------------------------------------------------------------------------

/**
 * Generate the default set of focus rooms for a fresh Wizard project canvas.
 * Positions are laid out on an 8 000 × 4 000 world-unit canvas.
 *
 * Layout (left → right, top → bottom):
 *
 *   [ Inbox (600×600) ]  [ Active Work (1600×1200) ]  [ Knowledge (1000×900) ]
 *                        [ Review Queue (1600×600) ]
 *                        [ Archive (1600×500) ]
 */
export function defaultFocusRooms(): FocusRoom[] {
  return [
    makeFocusRoom("Inbox", "inbox", {
      x: 0,
      y: 0,
      width: 700,
      height: 700,
    }),
    makeFocusRoom("Active Work", "unit-cluster", {
      x: 760,
      y: 0,
      width: 1600,
      height: 1200,
    }),
    makeFocusRoom("Runtime Monitor", "runtime-monitor", {
      x: 2420,
      y: 0,
      width: 1000,
      height: 700,
    }),
    makeFocusRoom("Knowledge", "knowledge-base", {
      x: 2420,
      y: 760,
      width: 1000,
      height: 800,
    }),
    makeFocusRoom("Review Queue", "review-queue", {
      x: 760,
      y: 1260,
      width: 1600,
      height: 600,
    }),
    makeFocusRoom("Archive", "archive", {
      x: 760,
      y: 1920,
      width: 1600,
      height: 500,
    }),
  ];
}
