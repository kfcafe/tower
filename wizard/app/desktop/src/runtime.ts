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