// Runtime monitoring interface for Wizard Desktop
// Provides typed surfaces for runtime state and updates

import { createSignal, createResource, onCleanup, Accessor } from "solid-js";

/// Runtime monitoring types (matching Rust definitions)
export interface RuntimeState {
  agents: Record<string, AgentInfo>;
  work_queue: QueuedWork[];
  process_metrics: ProcessMetrics;
  last_updated: string; // ISO timestamp
}

export interface AgentInfo {
  agent_id: string;
  unit_id: string;
  status: AgentStatus;
  started_at: string; // ISO timestamp
  last_activity: string; // ISO timestamp
  pid?: number;
  memory_usage?: number; // bytes
  cpu_usage?: number; // percentage
}

export type AgentStatus = 
  | { type: "Starting" }
  | { type: "Running" }
  | { type: "Stopping" }
  | { type: "Failed"; error: string };

export interface QueuedWork {
  unit_id: string;
  priority: WorkPriority;
  queued_at: string; // ISO timestamp
  estimated_duration?: number; // seconds
}

export type WorkPriority = "Low" | "Normal" | "High" | "Critical";

export interface ProcessMetrics {
  total_memory_usage: number; // bytes
  total_cpu_usage: number; // percentage
  active_processes: number;
  uptime: number; // seconds
}

export interface RuntimeEvent {
  type: "RuntimeStateChanged";
  state: RuntimeState;
} | {
  type: "AgentStatusChanged";
  agent_id: string;
  status: AgentStatus;
  timestamp: string;
} | {
  type: "WorkQueued";
  unit_id: string;
  priority: WorkPriority;
  timestamp: string;
} | {
  type: "WorkDequeued";
  unit_id: string;
  timestamp: string;
} | {
  type: "AgentSpawned";
  agent_id: string;
  unit_id: string;
} | {
  type: "AgentExited";
  agent_id: string;
  exit_code?: number;
} | {
  type: "ArtifactGenerated";
  artifact_id: string;
  unit_id: string;
  artifact_type: string;
  path: string;
  timestamp: string;
} | {
  type: "ReviewRequested";
  review_id: string;
  unit_id: string;
  review_type: string;
  timestamp: string;
} | {
  type: "ReviewCompleted";
  review_id: string;
  decision: string;
  notes?: string;
  timestamp: string;
} | {
  type: "VerificationResult";
  unit_id: string;
  verification_id: string;
  result: string;
  timestamp: string;
}

/// Runtime monitoring service
export class RuntimeMonitor {
  private eventListeners: ((event: RuntimeEvent) => void)[] = [];
  private runtimeState: RuntimeState | null = null;
  private updateInterval: number | null = null;

  constructor() {
    this.startPolling();
  }

  /// Subscribe to runtime events
  subscribe(listener: (event: RuntimeEvent) => void): () => void {
    this.eventListeners.push(listener);
    return () => {
      const index = this.eventListeners.indexOf(listener);
      if (index > -1) {
        this.eventListeners.splice(index, 1);
      }
    };
  }

  /// Get current runtime state
  getCurrentState(): RuntimeState | null {
    return this.runtimeState;
  }

  /// Start polling for runtime updates
  private startPolling() {
    this.updateInterval = window.setInterval(async () => {
      try {
        const newState = await this.fetchRuntimeState();
        if (newState && (!this.runtimeState || newState.last_updated !== this.runtimeState.last_updated)) {
          this.runtimeState = newState;
          this.notifyListeners({
            type: "RuntimeStateChanged",
            state: newState
          });
        }
      } catch (error) {
        console.warn("Failed to fetch runtime state:", error);
      }
    }, 1000); // Poll every second
  }

  /// Stop polling
  destroy() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    this.eventListeners.length = 0;
  }

  /// Notify all listeners of an event
  private notifyListeners(event: RuntimeEvent) {
    this.eventListeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        console.error("Error in runtime event listener:", error);
      }
    });
  }

  /// Fetch runtime state from backend (mock for now)
  private async fetchRuntimeState(): Promise<RuntimeState> {
    // TODO: Replace with actual Tauri IPC call
    // For now, simulate runtime state
    const now = new Date().toISOString();
    
    return {
      agents: {
        "agent-1": {
          agent_id: "agent-1",
          unit_id: "unit-1",
          status: { type: "Running" },
          started_at: now,
          last_activity: now,
          pid: 12345,
          memory_usage: 52428800, // 50MB
          cpu_usage: 15.5
        },
        "agent-2": {
          agent_id: "agent-2", 
          unit_id: "unit-2",
          status: { type: "Starting" },
          started_at: now,
          last_activity: now
        }
      },
      work_queue: [
        {
          unit_id: "unit-3",
          priority: "Normal",
          queued_at: now,
          estimated_duration: 120
        }
      ],
      process_metrics: {
        total_memory_usage: 104857600, // 100MB
        total_cpu_usage: 25.5,
        active_processes: 2,
        uptime: 3600 // 1 hour
      },
      last_updated: now
    };
  }
}

/// Global runtime monitor instance
let globalMonitor: RuntimeMonitor | null = null;

/// Get or create the global runtime monitor
export function getRuntimeMonitor(): RuntimeMonitor {
  if (!globalMonitor) {
    globalMonitor = new RuntimeMonitor();
  }
  return globalMonitor;
}

/// Create a reactive signal for runtime state
export function createRuntimeState(): [Accessor<RuntimeState | null>, RuntimeMonitor] {
  const monitor = getRuntimeMonitor();
  const [state, setState] = createSignal<RuntimeState | null>(monitor.getCurrentState());

  const unsubscribe = monitor.subscribe((event) => {
    if (event.type === "RuntimeStateChanged") {
      setState(event.state);
    }
  });

  onCleanup(unsubscribe);

  return [state, monitor];
}

/// Helper function to format memory usage
export function formatMemory(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

/// Helper function to format duration
export function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  } else if (seconds < 3600) {
    const minutes = Math.floor(seconds / 60);
    return `${minutes}m ${seconds % 60}s`;
  } else {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  }
}

/// Helper function to get status color
export function getStatusColor(status: AgentStatus): string {
  switch (status.type) {
    case "Starting":
      return "#ffa500"; // orange
    case "Running":
      return "#50c878"; // green
    case "Stopping":
      return "#ffd700"; // yellow
    case "Failed":
      return "#ff6b6b"; // red
    default:
      return "#888888"; // gray
  }
}

/// Helper function to get priority color
export function getPriorityColor(priority: WorkPriority): string {
  switch (priority) {
    case "Low":
      return "#888888"; // gray
    case "Normal":
      return "#4a9eff"; // blue
    case "High":
      return "#ffa500"; // orange
    case "Critical":
      return "#ff6b6b"; // red
    default:
      return "#888888"; // gray
  }
}

/// Review and artifact data types
export interface ReviewData {
  id: string;
  unit_id: string;
  review_type: string;
  status: string;
  created_at: string;
  decision?: string;
  notes?: string;
  checklist?: ReviewChecklistItem[];
}

export interface ReviewChecklistItem {
  description: string;
  checked: boolean;
  notes?: string;
  required: boolean;
}

export interface ArtifactData {
  id: string;
  unit_id: string;
  artifact_type: string;
  path: string;
  size: number;
  created_at: string;
  reviewed: boolean;
  review_status?: string;
  metadata?: Record<string, string>;
}

export interface VerificationData {
  id: string;
  unit_id: string;
  result: string;
  details: VerificationDetails;
  timestamp: string;
}

export interface VerificationDetails {
  checks: VerificationCheck[];
  summary: string;
  verified_artifacts: string[];
  issues: VerificationIssue[];
}

export interface VerificationCheck {
  name: string;
  status: string;
  description: string;
  duration: number; // seconds
  output?: string;
}

export interface VerificationIssue {
  severity: string;
  message: string;
  file?: string;
  line?: number;
  suggestion?: string;
}

/// Review monitoring service
export class ReviewMonitor {
  private reviewData: ReviewData[] = [];
  private eventListeners: ((reviews: ReviewData[]) => void)[] = [];
  private updateInterval: number | null = null;

  constructor() {
    this.startPolling();
  }

  /// Subscribe to review updates
  subscribe(listener: (reviews: ReviewData[]) => void): () => void {
    this.eventListeners.push(listener);
    // Send current data immediately
    listener(this.reviewData);
    return () => {
      const index = this.eventListeners.indexOf(listener);
      if (index > -1) {
        this.eventListeners.splice(index, 1);
      }
    };
  }

  /// Get current review data
  getCurrentReviews(): ReviewData[] {
    return [...this.reviewData];
  }

  /// Start polling for review updates
  private startPolling() {
    this.updateInterval = window.setInterval(async () => {
      try {
        const newReviews = await this.fetchReviews();
        if (JSON.stringify(newReviews) !== JSON.stringify(this.reviewData)) {
          this.reviewData = newReviews;
          this.notifyListeners();
        }
      } catch (error) {
        console.warn("Failed to fetch review data:", error);
      }
    }, 2000); // Poll every 2 seconds
  }

  /// Stop polling
  destroy() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    this.eventListeners.length = 0;
  }

  /// Notify all listeners
  private notifyListeners() {
    this.eventListeners.forEach(listener => {
      try {
        listener(this.reviewData);
      } catch (error) {
        console.error("Error in review event listener:", error);
      }
    });
  }

  /// Fetch review data from backend (mock for now)
  private async fetchReviews(): Promise<ReviewData[]> {
    // TODO: Replace with actual Tauri IPC call
    // For now, simulate review data
    const now = new Date().toISOString();
    
    return [
      {
        id: "review-1",
        unit_id: "unit-1", 
        review_type: "Code",
        status: "Pending",
        created_at: now,
        checklist: [
          {
            description: "Code follows project style guidelines",
            checked: false,
            required: true
          },
          {
            description: "No obvious security vulnerabilities",
            checked: false,
            required: true
          }
        ]
      },
      {
        id: "review-2",
        unit_id: "unit-2",
        review_type: "Documentation",
        status: "Completed",
        created_at: now,
        decision: "Approve",
        notes: "Documentation is clear and comprehensive"
      }
    ];
  }
}

/// Artifact monitoring service
export class ArtifactMonitor {
  private artifactData: ArtifactData[] = [];
  private eventListeners: ((artifacts: ArtifactData[]) => void)[] = [];
  private updateInterval: number | null = null;

  constructor() {
    this.startPolling();
  }

  /// Subscribe to artifact updates
  subscribe(listener: (artifacts: ArtifactData[]) => void): () => void {
    this.eventListeners.push(listener);
    // Send current data immediately
    listener(this.artifactData);
    return () => {
      const index = this.eventListeners.indexOf(listener);
      if (index > -1) {
        this.eventListeners.splice(index, 1);
      }
    };
  }

  /// Get current artifact data
  getCurrentArtifacts(): ArtifactData[] {
    return [...this.artifactData];
  }

  /// Start polling for artifact updates
  private startPolling() {
    this.updateInterval = window.setInterval(async () => {
      try {
        const newArtifacts = await this.fetchArtifacts();
        if (JSON.stringify(newArtifacts) !== JSON.stringify(this.artifactData)) {
          this.artifactData = newArtifacts;
          this.notifyListeners();
        }
      } catch (error) {
        console.warn("Failed to fetch artifact data:", error);
      }
    }, 2000); // Poll every 2 seconds
  }

  /// Stop polling
  destroy() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    this.eventListeners.length = 0;
  }

  /// Notify all listeners
  private notifyListeners() {
    this.eventListeners.forEach(listener => {
      try {
        listener(this.artifactData);
      } catch (error) {
        console.error("Error in artifact event listener:", error);
      }
    });
  }

  /// Fetch artifact data from backend (mock for now)
  private async fetchArtifacts(): Promise<ArtifactData[]> {
    // TODO: Replace with actual Tauri IPC call
    // For now, simulate artifact data
    const now = new Date().toISOString();
    
    return [
      {
        id: "artifact-1",
        unit_id: "unit-1",
        artifact_type: "CodeFile",
        path: "src/lib.rs",
        size: 1024 * 50, // 50KB
        created_at: now,
        reviewed: true,
        review_status: "Approve",
        metadata: {
          language: "rust"
        }
      },
      {
        id: "artifact-2",
        unit_id: "unit-2",
        artifact_type: "Documentation", 
        path: "docs/api.md",
        size: 1024 * 25, // 25KB
        created_at: now,
        reviewed: false
      },
      {
        id: "artifact-3",
        unit_id: "unit-1",
        artifact_type: "Test",
        path: "tests/integration.rs",
        size: 1024 * 15, // 15KB
        created_at: now,
        reviewed: false
      }
    ];
  }
}

/// Global monitor instances
let globalReviewMonitor: ReviewMonitor | null = null;
let globalArtifactMonitor: ArtifactMonitor | null = null;

/// Get or create the global review monitor
export function getReviewMonitor(): ReviewMonitor {
  if (!globalReviewMonitor) {
    globalReviewMonitor = new ReviewMonitor();
  }
  return globalReviewMonitor;
}

/// Get or create the global artifact monitor
export function getArtifactMonitor(): ArtifactMonitor {
  if (!globalArtifactMonitor) {
    globalArtifactMonitor = new ArtifactMonitor();
  }
  return globalArtifactMonitor;
}

/// Create a reactive signal for review state
export function createReviewState(): [Accessor<ReviewData[] | null>, ReviewMonitor] {
  const monitor = getReviewMonitor();
  const [state, setState] = createSignal<ReviewData[] | null>(monitor.getCurrentReviews());

  const unsubscribe = monitor.subscribe((reviews) => {
    setState(reviews);
  });

  onCleanup(unsubscribe);

  return [state, monitor];
}

/// Create a reactive signal for artifact state
export function createArtifactState(): [Accessor<ArtifactData[] | null>, ArtifactMonitor] {
  const monitor = getArtifactMonitor();
  const [state, setState] = createSignal<ArtifactData[] | null>(monitor.getCurrentArtifacts());

  const unsubscribe = monitor.subscribe((artifacts) => {
    setState(artifacts);
  });

  onCleanup(unsubscribe);

  return [state, monitor];
}