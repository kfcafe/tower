// Wizard Desktop - Read-only Shell with Runtime Monitoring
// This is the first real entry point for the Wizard desktop client.
// Currently implements a minimal read-only interface that demonstrates
// awareness of project snapshots and local Wizard state, now with runtime monitoring.

import { render } from "solid-js/web";
import { createSignal, createResource } from "solid-js";
import { createRuntimeState, formatMemory, formatDuration, getStatusColor, getPriorityColor } from "./runtime";

// Mock interfaces for now - will be replaced with proper Tauri IPC
interface ProjectSnapshot {
  project_name: string;
  unit_count: number;
  open_unit_count: number;
}

interface WizardLocalState {
  open_views: string[];
  last_project?: string;
}

// Mock data loaders - will be replaced with actual Tauri commands
async function loadProjectSnapshot(): Promise<ProjectSnapshot> {
  // Simulate loading project from .mana/
  return {
    project_name: "example-project",
    unit_count: 12,
    open_unit_count: 3,
  };
}

async function loadLocalState(): Promise<WizardLocalState> {
  // Simulate loading from .wizard/state.json
  return {
    open_views: ["project_home", "runtime_monitor"],
    last_project: "example-project",
  };
}

// Runtime Monitoring Component
function RuntimeMonitor() {
  const [runtimeState, monitor] = createRuntimeState();

  return (
    <section style={{ "margin-bottom": "25px" }}>
      <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
        Runtime State
      </h2>
      {!runtimeState() ? (
        <p style={{ color: "#888888" }}>Loading runtime state...</p>
      ) : (
        <div style={{ 
          background: "#2a2a2a", 
          padding: "15px", 
          "border-radius": "6px",
          "border-left": "3px solid #ff6b6b"
        }}>
          {/* Process Metrics */}
          <div style={{ "margin-bottom": "15px" }}>
            <h3 style={{ margin: "0 0 8px 0", "font-size": "14px", color: "#ffffff" }}>
              System Metrics
            </h3>
            <div style={{ display: "grid", "grid-template-columns": "1fr 1fr", gap: "10px" }}>
              <div>
                <span style={{ color: "#888888" }}>Memory: </span>
                <span>{formatMemory(runtimeState()!.process_metrics.total_memory_usage)}</span>
              </div>
              <div>
                <span style={{ color: "#888888" }}>CPU: </span>
                <span>{runtimeState()!.process_metrics.total_cpu_usage.toFixed(1)}%</span>
              </div>
              <div>
                <span style={{ color: "#888888" }}>Active Processes: </span>
                <span>{runtimeState()!.process_metrics.active_processes}</span>
              </div>
              <div>
                <span style={{ color: "#888888" }}>Uptime: </span>
                <span>{formatDuration(runtimeState()!.process_metrics.uptime)}</span>
              </div>
            </div>
          </div>

          {/* Running Agents */}
          <div style={{ "margin-bottom": "15px" }}>
            <h3 style={{ margin: "0 0 8px 0", "font-size": "14px", color: "#ffffff" }}>
              Running Agents ({Object.keys(runtimeState()!.agents).length})
            </h3>
            {Object.keys(runtimeState()!.agents).length === 0 ? (
              <p style={{ margin: "0", color: "#888888", "font-size": "12px" }}>No active agents</p>
            ) : (
              <div style={{ display: "flex", "flex-direction": "column", gap: "8px" }}>
                {Object.entries(runtimeState()!.agents).map(([agentId, agent]) => (
                  <div 
                    key={agentId}
                    style={{ 
                      background: "#1a1a1a", 
                      padding: "8px 10px", 
                      "border-radius": "4px",
                      "border-left": `3px solid ${getStatusColor(agent.status)}`
                    }}
                  >
                    <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center" }}>
                      <span style={{ "font-size": "12px", "font-weight": "bold" }}>
                        {agent.unit_id}
                      </span>
                      <span 
                        style={{ 
                          "font-size": "10px", 
                          color: getStatusColor(agent.status),
                          "text-transform": "uppercase",
                          "font-weight": "bold"
                        }}
                      >
                        {agent.status.type}
                      </span>
                    </div>
                    {agent.memory_usage && (
                      <div style={{ "font-size": "10px", color: "#888888", "margin-top": "4px" }}>
                        Memory: {formatMemory(agent.memory_usage)} | CPU: {agent.cpu_usage?.toFixed(1)}%
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Work Queue */}
          <div>
            <h3 style={{ margin: "0 0 8px 0", "font-size": "14px", color: "#ffffff" }}>
              Work Queue ({runtimeState()!.work_queue.length})
            </h3>
            {runtimeState()!.work_queue.length === 0 ? (
              <p style={{ margin: "0", color: "#888888", "font-size": "12px" }}>No queued work</p>
            ) : (
              <div style={{ display: "flex", "flex-direction": "column", gap: "6px" }}>
                {runtimeState()!.work_queue.map((work, index) => (
                  <div 
                    key={index}
                    style={{ 
                      background: "#1a1a1a", 
                      padding: "6px 8px", 
                      "border-radius": "4px",
                      "border-left": `3px solid ${getPriorityColor(work.priority)}`
                    }}
                  >
                    <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center" }}>
                      <span style={{ "font-size": "12px" }}>
                        {work.unit_id}
                      </span>
                      <span 
                        style={{ 
                          "font-size": "10px", 
                          color: getPriorityColor(work.priority),
                          "text-transform": "uppercase",
                          "font-weight": "bold"
                        }}
                      >
                        {work.priority}
                      </span>
                    </div>
                    {work.estimated_duration && (
                      <div style={{ "font-size": "10px", color: "#888888", "margin-top": "2px" }}>
                        Est. duration: {formatDuration(work.estimated_duration)}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

// Main Wizard Shell Component
function WizardShell() {
  const [project] = createResource(loadProjectSnapshot);
  const [localState] = createResource(loadLocalState);

  return (
    <div style={{ 
      padding: "20px", 
      font: "14px/1.4 -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
      background: "#1a1a1a",
      color: "#e0e0e0",
      "min-height": "100vh"
    }}>
      <header style={{ "margin-bottom": "30px" }}>
        <h1 style={{ margin: "0 0 8px 0", color: "#ffffff" }}>Wizard</h1>
        <p style={{ margin: "0", color: "#888888", "font-size": "12px" }}>
          Runtime monitoring and supervision surface • Orchestrator-owned state
        </p>
      </header>

      <main>
        <section style={{ "margin-bottom": "25px" }}>
          <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
            Project Snapshot
          </h2>
          {project.loading ? (
            <p style={{ color: "#888888" }}>Loading project data...</p>
          ) : project.error ? (
            <p style={{ color: "#ff6b6b" }}>Error loading project</p>
          ) : (
            <div style={{ 
              background: "#2a2a2a", 
              padding: "15px", 
              "border-radius": "6px",
              "border-left": "3px solid #4a9eff"
            }}>
              <p style={{ margin: "0 0 8px 0" }}>
                <strong>Name:</strong> {project()?.project_name}
              </p>
              <p style={{ margin: "0 0 8px 0" }}>
                <strong>Total units:</strong> {project()?.unit_count}
              </p>
              <p style={{ margin: "0" }}>
                <strong>Open units:</strong> {project()?.open_unit_count}
              </p>
            </div>
          )}
        </section>

        {/* Runtime Monitoring Section */}
        <RuntimeMonitor />

        <section style={{ "margin-bottom": "25px" }}>
          <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
            Local State (.wizard/)
          </h2>
          {localState.loading ? (
            <p style={{ color: "#888888" }}>Loading local state...</p>
          ) : localState.error ? (
            <p style={{ color: "#ff6b6b" }}>Error loading local state</p>
          ) : (
            <div style={{ 
              background: "#2a2a2a", 
              padding: "15px", 
              "border-radius": "6px",
              "border-left": "3px solid #50c878"
            }}>
              <p style={{ margin: "0 0 8px 0" }}>
                <strong>Open views:</strong> {localState()?.open_views.join(", ") || "none"}
              </p>
              <p style={{ margin: "0" }}>
                <strong>Last project:</strong> {localState()?.last_project || "none"}
              </p>
            </div>
          )}
        </section>

        <section>
          <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
            Status
          </h2>
          <div style={{ 
            background: "#2a2a2a", 
            padding: "15px", 
            "border-radius": "6px",
            "border-left": "3px solid #ffa500"
          }}>
            <p style={{ margin: "0 0 8px 0" }}>
              ✅ Runtime monitoring and supervision surface active:
            </p>
            <ul style={{ margin: "8px 0 0 20px", "padding-left": "0" }}>
              <li>Live agent/runtime state beyond static snapshots</li>
              <li>Typed surface for runtime updates (orchestrator-owned)</li>
              <li>Process lifecycle monitoring and work queue tracking</li>
              <li>Orchestration state separate from .wizard/ local UI state</li>
            </ul>
            <p style={{ margin: "12px 0 0 0", "font-size": "12px", color: "#888888" }}>
              Full canvas interface and agent orchestration coming in future iterations.
            </p>
          </div>
        </section>
      </main>
    </div>
  );
}

// Bootstrap the app
render(() => <WizardShell />, document.getElementById("root")!);