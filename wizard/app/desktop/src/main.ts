// Wizard Desktop - Read-only Shell with Runtime Monitoring
// This is the first real entry point for the Wizard desktop client.
// Currently implements a minimal read-only interface that demonstrates
// awareness of project snapshots and local Wizard state, now with runtime monitoring.

import { render } from "solid-js/web";
import { createSignal, createResource } from "solid-js";
import { createRuntimeState, formatMemory, formatDuration, getStatusColor, getPriorityColor, createReviewState, createArtifactState } from "./runtime";

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

interface ReviewData {
  id: string;
  unit_id: string;
  review_type: string;
  status: string;
  created_at: string;
  decision?: string;
  notes?: string;
}

interface ArtifactData {
  id: string;
  unit_id: string;
  artifact_type: string;
  path: string;
  size: number;
  created_at: string;
  reviewed: boolean;
  review_status?: string;
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

// Review Inspector Component
function ReviewInspector() {
  const [reviews, reviewsResource] = createReviewState();

  return (
    <section style={{ "margin-bottom": "25px" }}>
      <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
        Review Sessions
      </h2>
      {!reviews() ? (
        <p style={{ color: "#888888" }}>Loading review sessions...</p>
      ) : reviews().length === 0 ? (
        <div style={{ 
          background: "#2a2a2a", 
          padding: "15px", 
          "border-radius": "6px",
          "border-left": "3px solid #4a9eff"
        }}>
          <p style={{ margin: "0", color: "#888888" }}>No active review sessions</p>
        </div>
      ) : (
        <div style={{ display: "flex", "flex-direction": "column", gap: "10px" }}>
          {reviews().map((review) => (
            <div 
              key={review.id}
              style={{ 
                background: "#2a2a2a", 
                padding: "15px", 
                "border-radius": "6px",
                "border-left": `3px solid ${getReviewStatusColor(review.status)}`
              }}
            >
              <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center", "margin-bottom": "8px" }}>
                <h3 style={{ margin: "0", "font-size": "14px", color: "#ffffff" }}>
                  {review.review_type} Review for {review.unit_id}
                </h3>
                <span 
                  style={{ 
                    "font-size": "10px", 
                    color: getReviewStatusColor(review.status),
                    "text-transform": "uppercase",
                    "font-weight": "bold"
                  }}
                >
                  {review.status}
                </span>
              </div>
              <div style={{ "font-size": "12px", color: "#888888" }}>
                <div>Created: {new Date(review.created_at).toLocaleString()}</div>
                {review.decision && <div>Decision: {review.decision}</div>}
                {review.notes && <div style={{ "margin-top": "4px" }}>Notes: {review.notes}</div>}
              </div>
              <div style={{ "margin-top": "8px", display: "flex", gap: "8px" }}>
                {review.status === "Pending" && (
                  <>
                    <button 
                      onClick={() => completeReview(review.id, "Approve")}
                      style={{
                        padding: "4px 8px",
                        "font-size": "10px",
                        border: "1px solid #50c878",
                        background: "transparent",
                        color: "#50c878",
                        "border-radius": "3px",
                        cursor: "pointer"
                      }}
                    >
                      Approve
                    </button>
                    <button 
                      onClick={() => completeReview(review.id, "RequestChanges")}
                      style={{
                        padding: "4px 8px",
                        "font-size": "10px",
                        border: "1px solid #ffa500",
                        background: "transparent",
                        color: "#ffa500",
                        "border-radius": "3px",
                        cursor: "pointer"
                      }}
                    >
                      Request Changes
                    </button>
                    <button 
                      onClick={() => completeReview(review.id, "Reject")}
                      style={{
                        padding: "4px 8px",
                        "font-size": "10px",
                        border: "1px solid #ff6b6b",
                        background: "transparent",
                        color: "#ff6b6b",
                        "border-radius": "3px",
                        cursor: "pointer"
                      }}
                    >
                      Reject
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

// Artifact Inspector Component
function ArtifactInspector() {
  const [artifacts, artifactsResource] = createArtifactState();

  return (
    <section style={{ "margin-bottom": "25px" }}>
      <h2 style={{ margin: "0 0 12px 0", "font-size": "16px", color: "#cccccc" }}>
        Generated Artifacts
      </h2>
      {!artifacts() ? (
        <p style={{ color: "#888888" }}>Loading artifacts...</p>
      ) : artifacts().length === 0 ? (
        <div style={{ 
          background: "#2a2a2a", 
          padding: "15px", 
          "border-radius": "6px",
          "border-left": "3px solid #50c878"
        }}>
          <p style={{ margin: "0", color: "#888888" }}>No artifacts generated yet</p>
        </div>
      ) : (
        <div style={{ display: "flex", "flex-direction": "column", gap: "10px" }}>
          {artifacts().map((artifact) => (
            <div 
              key={artifact.id}
              style={{ 
                background: "#2a2a2a", 
                padding: "15px", 
                "border-radius": "6px",
                "border-left": `3px solid ${getArtifactTypeColor(artifact.artifact_type)}`
              }}
            >
              <div style={{ display: "flex", "justify-content": "space-between", "align-items": "center", "margin-bottom": "8px" }}>
                <h3 style={{ margin: "0", "font-size": "14px", color: "#ffffff" }}>
                  {artifact.artifact_type}: {artifact.path.split('/').pop()}
                </h3>
                <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
                  {artifact.reviewed && (
                    <span 
                      style={{ 
                        "font-size": "10px", 
                        color: "#50c878",
                        "text-transform": "uppercase",
                        "font-weight": "bold"
                      }}
                    >
                      ✓ Reviewed
                    </span>
                  )}
                  <span style={{ "font-size": "12px", color: "#888888" }}>
                    {formatMemory(artifact.size)}
                  </span>
                </div>
              </div>
              <div style={{ "font-size": "12px", color: "#888888" }}>
                <div>Unit: {artifact.unit_id}</div>
                <div>Path: {artifact.path}</div>
                <div>Created: {new Date(artifact.created_at).toLocaleString()}</div>
                {artifact.review_status && <div>Review Status: {artifact.review_status}</div>}
              </div>
              <div style={{ "margin-top": "8px", display: "flex", gap: "8px" }}>
                <button 
                  onClick={() => viewArtifact(artifact.id)}
                  style={{
                    padding: "4px 8px",
                    "font-size": "10px",
                    border: "1px solid #4a9eff",
                    background: "transparent",
                    color: "#4a9eff",
                    "border-radius": "3px",
                    cursor: "pointer"
                  }}
                >
                  View
                </button>
                {!artifact.reviewed && (
                  <button 
                    onClick={() => requestArtifactReview(artifact.id)}
                    style={{
                      padding: "4px 8px",
                      "font-size": "10px",
                      border: "1px solid #ffa500",
                      background: "transparent",
                      color: "#ffa500",
                      "border-radius": "3px",
                      cursor: "pointer"
                    }}
                  >
                    Request Review
                  </button>
                )}
                <button 
                  onClick={() => verifyArtifact(artifact.id)}
                  style={{
                    padding: "4px 8px",
                    "font-size": "10px",
                    border: "1px solid #50c878",
                    background: "transparent",
                    color: "#50c878",
                    "border-radius": "3px",
                    cursor: "pointer"
                  }}
                >
                  Verify
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

// Helper functions
function getReviewStatusColor(status: string): string {
  switch (status) {
    case "Pending":
      return "#ffa500"; // orange
    case "InProgress":
      return "#4a9eff"; // blue
    case "Completed":
      return "#50c878"; // green
    case "Cancelled":
      return "#888888"; // gray
    case "Blocked":
      return "#ff6b6b"; // red
    default:
      return "#888888"; // gray
  }
}

function getArtifactTypeColor(type: string): string {
  switch (type) {
    case "CodeFile":
      return "#4a9eff"; // blue
    case "Documentation":
      return "#50c878"; // green
    case "Test":
      return "#ffa500"; // orange
    case "Config":
      return "#ff6b6b"; // red
    case "Build":
      return "#8b5cf6"; // purple
    case "Log":
      return "#888888"; // gray
    case "Image":
      return "#f59e0b"; // yellow
    case "Data":
      return "#06b6d4"; // cyan
    default:
      return "#888888"; // gray
  }
}

// Action functions (TODO: implement with Tauri IPC)
function completeReview(reviewId: string, decision: string) {
  console.log(`Completing review ${reviewId} with decision: ${decision}`);
  // TODO: Call Tauri command to complete review
}

function viewArtifact(artifactId: string) {
  console.log(`Viewing artifact ${artifactId}`);
  // TODO: Call Tauri command to view artifact
}

function requestArtifactReview(artifactId: string) {
  console.log(`Requesting review for artifact ${artifactId}`);
  // TODO: Call Tauri command to request artifact review
}

function verifyArtifact(artifactId: string) {
  console.log(`Verifying artifact ${artifactId}`);
  // TODO: Call Tauri command to verify artifact
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

        {/* Review and Artifact Inspection */}
        <ReviewInspector />
        <ArtifactInspector />

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
              <li>Review flow surfaces for human inspection</li>
              <li>Artifact tracking and verification</li>
              <li>Durable review sessions grounded in .mana/ runtime events</li>
            </ul>
            <p style={{ margin: "12px 0 0 0", "font-size": "12px", color: "#888888" }}>
              Full canvas interface and deeper agent orchestration coming in future iterations.
            </p>
          </div>
        </section>
      </main>
    </div>
  );
}

// Bootstrap the app
render(() => <WizardShell />, document.getElementById("root")!);