/**
 * cards.ts — Typed card definitions for the Wizard canvas
 *
 * Every object on the canvas is a TypedCard. Each card variant carries the
 * minimal data needed to render and interact with it; heavier data (full unit
 * specs, knowledge trees, etc.) is loaded lazily by the surface component that
 * owns the card.
 *
 * Card kinds map directly to Wizard's three runtime surfaces:
 *   • UnitCard      — a work unit (task / agent context)
 *   • RuntimeCard   — a live agent or process
 *   • KnowledgeCard — a knowledge-base entry or document excerpt
 *
 * Additional "chrome" card kinds cover navigation and grouping:
 *   • RoomPortalCard — a link to another focus room (for zoomed-out overview)
 *   • NoteCard        — an inline human annotation
 */

import type { FocusRoomId } from "./focus-room";

// ---------------------------------------------------------------------------
// Ids
// ---------------------------------------------------------------------------

export type CardId = string & { readonly __brand: "CardId" };

export function makeCardId(raw: string): CardId {
  return raw as CardId;
}

export function generateCardId(): CardId {
  return makeCardId(`card-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
}

// ---------------------------------------------------------------------------
// Card kinds
// ---------------------------------------------------------------------------

export type CardKind =
  | "unit"
  | "runtime"
  | "knowledge"
  | "room-portal"
  | "note";

// ---------------------------------------------------------------------------
// Per-kind payloads
// ---------------------------------------------------------------------------

/** A Wizard work unit (maps to a .mana/units/<id> directory). */
export interface UnitCardPayload {
  unitId: string;
  title: string;
  status: UnitStatus;
  /** Priority: higher number = higher priority. */
  priority: number;
  /** Number of open child units. */
  openChildCount: number;
  /** Ids of agents currently executing this unit. */
  activeAgentIds: string[];
  /** Abbreviated spec excerpt for card body preview. */
  specExcerpt?: string;
}

export type UnitStatus =
  | "open"
  | "in_progress"
  | "blocked"
  | "review"
  | "done"
  | "cancelled";

/** A live agent/process being monitored in the runtime surface. */
export interface RuntimeCardPayload {
  agentId: string;
  unitId: string;
  status: "starting" | "running" | "stopping" | "failed";
  memoryBytes?: number;
  cpuPercent?: number;
  /** ISO timestamp of last activity. */
  lastActivity?: string;
  /** Tail of stdout / stderr for inline preview. */
  logTail?: string[];
}

/** A knowledge-base entry or document excerpt. */
export interface KnowledgeCardPayload {
  entryId: string;
  title: string;
  /** Plain-text excerpt of the content. */
  excerpt: string;
  /** Source path relative to the project root. */
  sourcePath?: string;
  /** Semantic tags. */
  tags: string[];
  /** Confidence score 0–1 when shown as a retrieval result. */
  confidence?: number;
}

/** A portal that links to another focus room — rendered at overview zoom. */
export interface RoomPortalCardPayload {
  targetRoomId: FocusRoomId;
  targetRoomLabel: string;
  cardCount: number;
}

/** A human-authored inline note / annotation. */
export interface NoteCardPayload {
  body: string;
  /** ISO timestamp of last edit. */
  editedAt: string;
  /** Author display name (usually the local user). */
  author: string;
}

// ---------------------------------------------------------------------------
// Base card geometry
// ---------------------------------------------------------------------------

/** All canvas cards share a common geometric base. */
export interface CardBase {
  id: CardId;
  /** World-space X position (left edge). */
  x: number;
  /** World-space Y position (top edge). */
  y: number;
  /** Explicit width override; absent → kind's default width. */
  width?: number;
  /** Explicit height override; absent → auto from content. */
  height?: number;
  /** Focus room this card belongs to (undefined = root / uncategorised). */
  roomId?: FocusRoomId;
  /** Whether the card is currently "expanded" (shows full detail). */
  expanded: boolean;
  /** Wall-clock time when the card was placed on the canvas. */
  createdAt: string;
}

// ---------------------------------------------------------------------------
// Discriminated union — the actual TypedCard
// ---------------------------------------------------------------------------

export type TypedCard =
  | (CardBase & { kind: "unit"; payload: UnitCardPayload })
  | (CardBase & { kind: "runtime"; payload: RuntimeCardPayload })
  | (CardBase & { kind: "knowledge"; payload: KnowledgeCardPayload })
  | (CardBase & { kind: "room-portal"; payload: RoomPortalCardPayload })
  | (CardBase & { kind: "note"; payload: NoteCardPayload });

// ---------------------------------------------------------------------------
// Default dimensions (world units)
// ---------------------------------------------------------------------------

export const CARD_DEFAULTS: Record<CardKind, { width: number; height: number }> = {
  unit: { width: 280, height: 160 },
  runtime: { width: 280, height: 140 },
  knowledge: { width: 260, height: 130 },
  "room-portal": { width: 200, height: 110 },
  note: { width: 220, height: 100 },
};

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

function base(overrides: Partial<CardBase> & Pick<CardBase, "id">): CardBase {
  return {
    x: 0,
    y: 0,
    expanded: false,
    createdAt: new Date().toISOString(),
    ...overrides,
  };
}

export function makeUnitCard(
  payload: UnitCardPayload,
  geo?: Partial<CardBase>
): TypedCard {
  return { ...base({ id: generateCardId(), ...geo }), kind: "unit", payload };
}

export function makeRuntimeCard(
  payload: RuntimeCardPayload,
  geo?: Partial<CardBase>
): TypedCard {
  return { ...base({ id: generateCardId(), ...geo }), kind: "runtime", payload };
}

export function makeKnowledgeCard(
  payload: KnowledgeCardPayload,
  geo?: Partial<CardBase>
): TypedCard {
  return {
    ...base({ id: generateCardId(), ...geo }),
    kind: "knowledge",
    payload,
  };
}

export function makeRoomPortalCard(
  payload: RoomPortalCardPayload,
  geo?: Partial<CardBase>
): TypedCard {
  return {
    ...base({ id: generateCardId(), ...geo }),
    kind: "room-portal",
    payload,
  };
}

export function makeNoteCard(
  payload: NoteCardPayload,
  geo?: Partial<CardBase>
): TypedCard {
  return { ...base({ id: generateCardId(), ...geo }), kind: "note", payload };
}

// ---------------------------------------------------------------------------
// Type guards
// ---------------------------------------------------------------------------

export function isUnitCard(
  c: TypedCard
): c is TypedCard & { kind: "unit"; payload: UnitCardPayload } {
  return c.kind === "unit";
}

export function isRuntimeCard(
  c: TypedCard
): c is TypedCard & { kind: "runtime"; payload: RuntimeCardPayload } {
  return c.kind === "runtime";
}

export function isKnowledgeCard(
  c: TypedCard
): c is TypedCard & { kind: "knowledge"; payload: KnowledgeCardPayload } {
  return c.kind === "knowledge";
}

// ---------------------------------------------------------------------------
// Status / priority helpers for the UI layer
// ---------------------------------------------------------------------------

/** Accent colour for a given unit status. */
export function unitStatusColor(status: UnitStatus): string {
  const palette: Record<UnitStatus, string> = {
    open: "#4a9eff",
    in_progress: "#ffa500",
    blocked: "#ff6b6b",
    review: "#8b5cf6",
    done: "#50c878",
    cancelled: "#555555",
  };
  return palette[status] ?? "#888888";
}

/** Human label for a unit status. */
export function unitStatusLabel(status: UnitStatus): string {
  const labels: Record<UnitStatus, string> = {
    open: "Open",
    in_progress: "In Progress",
    blocked: "Blocked",
    review: "In Review",
    done: "Done",
    cancelled: "Cancelled",
  };
  return labels[status] ?? status;
}
