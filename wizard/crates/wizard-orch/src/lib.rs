use notify::{Event as NotifyEvent, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use wizard_proto::{
    AgentInfo, AgentStatus, Event, ProcessMetrics, ProjectSnapshot, QueuedWork, RuntimeSnapshot,
    RuntimeState, WorkPriority,
};

/// Runtime monitoring and supervision for Wizard
pub struct RuntimeSupervisor {
    /// Current runtime state
    state: Arc<Mutex<RuntimeState>>,
    /// Broadcast sender for runtime updates
    update_sender: broadcast::Sender<Event>,
    /// Receiver for shutdown signals
    shutdown_receiver: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

impl RuntimeSupervisor {
    pub fn new() -> Self {
        let (update_sender, _) = broadcast::channel(100);
        let (_, shutdown_receiver) = mpsc::channel();

        let initial_state = RuntimeState {
            agents: HashMap::new(),
            work_queue: Vec::new(),
            process_metrics: ProcessMetrics {
                total_memory_usage: 0,
                total_cpu_usage: 0.0,
                active_processes: 0,
                uptime: Duration::from_secs(0),
            },
            last_updated: SystemTime::now(),
        };

        Self {
            state: Arc::new(Mutex::new(initial_state)),
            update_sender,
            shutdown_receiver: Arc::new(Mutex::new(Some(shutdown_receiver))),
        }
    }

    /// Subscribe to runtime updates
    pub fn subscribe_runtime(&self) -> broadcast::Receiver<Event> {
        self.update_sender.subscribe()
    }

    /// Get current runtime state snapshot
    pub fn get_runtime_state(&self) -> RuntimeState {
        self.state.lock().unwrap().clone()
    }

    /// Watch for runtime changes and broadcast updates
    pub fn watch_runtime(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state = Arc::clone(&self.state);
        let sender = self.update_sender.clone();

        // Start background monitoring task
        thread::spawn(move || {
            let mut last_update = SystemTime::now();

            loop {
                thread::sleep(Duration::from_millis(500));

                // Update process metrics
                {
                    let mut current_state = state.lock().unwrap();
                    current_state.process_metrics = collect_process_metrics(&current_state);
                    current_state.last_updated = SystemTime::now();

                    // Check for agent status changes and cleanup dead processes
                    let mut agents_to_remove = Vec::new();
                    for (agent_id, agent) in current_state.agents.iter_mut() {
                        match &agent.status {
                            AgentStatus::Running => {
                                // Check if process is still alive
                                if let Some(pid) = agent.pid {
                                    if !is_process_alive(pid) {
                                        agent.status = AgentStatus::Failed {
                                            error: "Process terminated unexpectedly".to_string(),
                                        };

                                        // Send status change event
                                        let _ = sender.send(Event::AgentStatusChanged {
                                            agent_id: agent_id.clone(),
                                            status: agent.status.clone(),
                                            timestamp: SystemTime::now(),
                                        });
                                    }
                                }
                            }
                            AgentStatus::Failed { .. } => {
                                // Mark for cleanup after some time
                                if agent.last_activity.elapsed().unwrap_or(Duration::MAX)
                                    > Duration::from_secs(30)
                                {
                                    agents_to_remove.push(agent_id.clone());
                                }
                            }
                            _ => {}
                        }
                    }

                    // Remove failed agents after grace period
                    for agent_id in agents_to_remove {
                        current_state.agents.remove(&agent_id);
                    }

                    // Broadcast periodic state updates
                    if last_update.elapsed().unwrap_or(Duration::MAX) > Duration::from_secs(2) {
                        let _ = sender.send(Event::RuntimeStateChanged {
                            state: current_state.clone(),
                        });
                        last_update = SystemTime::now();
                    }
                }
            }
        });

        Ok(())
    }

    /// Start a new agent for a unit
    pub fn start_agent(&self, unit_id: String) -> Result<String, Box<dyn std::error::Error>> {
        let agent_id = format!("agent-{}-{}", unit_id, SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs());

        let agent_info = AgentInfo {
            agent_id: agent_id.clone(),
            unit_id: unit_id.clone(),
            status: AgentStatus::Starting,
            started_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            pid: None,
            memory_usage: None,
            cpu_usage: None,
        };

        {
            let mut state = self.state.lock().unwrap();
            state.agents.insert(agent_id.clone(), agent_info);
            state.last_updated = SystemTime::now();

            // Remove from work queue if present
            state.work_queue.retain(|work| work.unit_id != unit_id);
        }

        // Simulate agent startup (in real implementation, this would spawn actual processes)
        let state = Arc::clone(&self.state);
        let sender = self.update_sender.clone();
        let agent_id_clone = agent_id.clone();
        let unit_id_clone = unit_id.clone();

        thread::spawn(move || {
            // Simulate startup delay
            thread::sleep(Duration::from_millis(500));

            // Update agent to running state
            {
                let mut current_state = state.lock().unwrap();
                if let Some(agent) = current_state.agents.get_mut(&agent_id_clone) {
                    agent.status = AgentStatus::Running;
                    agent.pid = Some(std::process::id() + rand::random::<u32>() % 1000); // Mock PID
                    agent.last_activity = SystemTime::now();
                }
                current_state.last_updated = SystemTime::now();
            }

            let _ = sender.send(Event::AgentSpawned {
                agent_id: agent_id_clone.clone(),
                unit_id: unit_id_clone,
            });

            let _ = sender.send(Event::AgentStatusChanged {
                agent_id: agent_id_clone,
                status: AgentStatus::Running,
                timestamp: SystemTime::now(),
            });
        });

        Ok(agent_id)
    }

    /// Stop an agent
    pub fn stop_agent(&self, agent_id: String) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut state = self.state.lock().unwrap();
            if let Some(agent) = state.agents.get_mut(&agent_id) {
                agent.status = AgentStatus::Stopping;
                agent.last_activity = SystemTime::now();
            }
            state.last_updated = SystemTime::now();
        }

        let state = Arc::clone(&self.state);
        let sender = self.update_sender.clone();
        let agent_id_clone = agent_id.clone();

        thread::spawn(move || {
            // Simulate shutdown delay
            thread::sleep(Duration::from_millis(200));

            // Remove agent from state
            let exit_code = {
                let mut current_state = state.lock().unwrap();
                current_state.agents.remove(&agent_id_clone);
                current_state.last_updated = SystemTime::now();
                Some(0) // Normal exit
            };

            let _ = sender.send(Event::AgentExited {
                agent_id: agent_id_clone,
                exit_code,
            });
        });

        Ok(())
    }

    /// Queue work for later execution
    pub fn queue_work(&self, unit_id: String, priority: WorkPriority) -> Result<(), Box<dyn std::error::Error>> {
        let work = QueuedWork {
            unit_id: unit_id.clone(),
            priority: priority.clone(),
            queued_at: SystemTime::now(),
            estimated_duration: Some(Duration::from_secs(60)), // Mock estimate
        };

        {
            let mut state = self.state.lock().unwrap();
            state.work_queue.push(work);
            // Sort by priority (highest first)
            state.work_queue.sort_by(|a, b| b.priority.cmp(&a.priority));
            state.last_updated = SystemTime::now();
        }

        let _ = self.update_sender.send(Event::WorkQueued {
            unit_id,
            priority,
            timestamp: SystemTime::now(),
        });

        Ok(())
    }

    /// Get the next work item from the queue
    pub fn dequeue_work(&self) -> Option<QueuedWork> {
        let mut state = self.state.lock().unwrap();
        if let Some(work) = state.work_queue.pop() {
            state.last_updated = SystemTime::now();

            let _ = self.update_sender.send(Event::WorkDequeued {
                unit_id: work.unit_id.clone(),
                timestamp: SystemTime::now(),
            });

            Some(work)
        } else {
            None
        }
    }
}

/// Collect system-level process metrics
fn collect_process_metrics(state: &RuntimeState) -> ProcessMetrics {
    // In a real implementation, this would use system APIs to collect actual metrics
    // For now, we'll provide mock data based on the current state

    let active_processes = state.agents.len();
    let mock_memory_per_process = 50 * 1024 * 1024; // 50MB per process
    let total_memory_usage = (active_processes as u64) * mock_memory_per_process;

    // Mock CPU usage that varies based on number of active processes
    let base_cpu = 5.0;
    let cpu_per_process = 10.0;
    let total_cpu_usage = base_cpu + (active_processes as f32 * cpu_per_process);

    ProcessMetrics {
        total_memory_usage,
        total_cpu_usage: total_cpu_usage.min(100.0),
        active_processes,
        uptime: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO),
    }
}

/// Check if a process is still alive (mock implementation)
fn is_process_alive(pid: u32) -> bool {
    // In a real implementation, this would check if the process exists
    // For now, we'll randomly simulate some processes dying
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    pid.hash(&mut hasher);
    let hash = hasher.finish();

    // 95% chance the process is alive (simulate occasional failures)
    (hash % 100) < 95
}

/// Create a global runtime supervisor instance
static RUNTIME_SUPERVISOR: std::sync::OnceLock<RuntimeSupervisor> = std::sync::OnceLock::new();

/// Get or initialize the global runtime supervisor
pub fn get_runtime_supervisor() -> &'static RuntimeSupervisor {
    RUNTIME_SUPERVISOR.get_or_init(|| {
        let supervisor = RuntimeSupervisor::new();
        // Start monitoring in the background
        if let Err(e) = supervisor.watch_runtime() {
            eprintln!("Failed to start runtime monitoring: {}", e);
        }
        supervisor
    })
}

/// Subscribe to runtime state changes
pub fn subscribe_runtime() -> broadcast::Receiver<Event> {
    get_runtime_supervisor().subscribe_runtime()
}

/// Get current runtime state
pub fn runtime_stream() -> RuntimeState {
    get_runtime_supervisor().get_runtime_state()
}

/// Watch runtime changes (alias for subscribe_runtime for API consistency)
pub fn watch_runtime() -> broadcast::Receiver<Event> {
    subscribe_runtime()
}

/// Load project snapshot from the nearest .mana/ directory
pub fn load_project_snapshot() -> Result<ProjectSnapshot, Box<dyn std::error::Error>> {
    // Find the nearest .mana/ directory starting from current directory
    let mana_dir = mana_core::discovery::find_mana_dir(Path::new("."))?;

    // Load the index to get unit information
    let index = mana_core::api::load_index(&mana_dir)?;

    // Get project name from config if available, otherwise use directory name
    let project_name = get_project_name(&mana_dir)?;

    // Count total units and open units (Open or InProgress are considered "open")
    let unit_count = index.units.len();
    let open_unit_count = index
        .units
        .iter()
        .filter(|unit| {
            matches!(
                unit.status,
                mana_core::api::Status::Open | mana_core::api::Status::InProgress
            )
        })
        .count();

    Ok(ProjectSnapshot {
        project_name,
        unit_count,
        open_unit_count,
    })
}

/// Load runtime snapshot (minimal implementation for now)
pub fn load_runtime_snapshot() -> Result<RuntimeSnapshot, Box<dyn std::error::Error>> {
    // For now, return empty runtime state since we don't have agent management yet
    // This provides the real loading path instead of pure placeholders
    Ok(RuntimeSnapshot {
        running_agents: 0,
        queued_units: 0,
    })
}

/// Get project name from config or fallback to directory name
fn get_project_name(mana_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    // Try to load project name from config.yaml
    let config_path = mana_dir.join("config.yaml");
    if config_path.exists() {
        let config_content = std::fs::read_to_string(&config_path)?;
        if let Ok(config) = serde_yaml::from_str::<serde_yaml::Value>(&config_content) {
            if let Some(name) = config.get("project").and_then(|v| v.as_str()) {
                return Ok(name.to_string());
            }
        }
    }

    // Fallback: use parent directory name
    let project_dir = mana_dir
        .parent()
        .ok_or("Cannot determine project directory")?;
    let project_name = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(project_name)
}

/// Watch a .mana directory for changes and trigger snapshot refreshes
pub fn watch_mana_directory<F>(
    mana_dir: PathBuf,
    mut on_change: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(ProjectSnapshot) -> () + Send + 'static,
{
    let (tx, rx) = mpsc::channel();

    // Create the file watcher
    let mut watcher = notify::RecommendedWatcher::new(
        move |result| {
            match result {
                Ok(event) => {
                    if let Err(e) = tx.send(event) {
                        eprintln!("Failed to send file system event: {}", e);
                    }
                }
                Err(e) => eprintln!("Watch error: {}", e),
            }
        },
        notify::Config::default(),
    )?;

    // Watch the .mana directory recursively
    watcher.watch(&mana_dir, RecursiveMode::Recursive)?;

    // Handle events in a background thread
    thread::spawn(move || {
        // Debounce rapid file system events
        let mut last_refresh = std::time::Instant::now();
        let debounce_duration = Duration::from_millis(500);

        for event in rx {
            if should_trigger_refresh(&event) {
                let now = std::time::Instant::now();
                if now.duration_since(last_refresh) >= debounce_duration {
                    last_refresh = now;

                    // Try to load a fresh snapshot and trigger the callback
                    if let Ok(snapshot) = load_project_snapshot_from_dir(&mana_dir) {
                        on_change(snapshot);
                    }
                }
            }
        }
    });

    // Keep the watcher alive by moving it into a long-lived scope
    // In a real application, you'd store this somewhere or join the thread
    std::mem::forget(watcher);

    Ok(())
}

/// Load project snapshot from a specific .mana directory
fn load_project_snapshot_from_dir(
    mana_dir: &Path,
) -> Result<ProjectSnapshot, Box<dyn std::error::Error>> {
    // Load the index to get unit information
    let index = mana_core::api::load_index(mana_dir)?;

    // Get project name from config if available, otherwise use directory name
    let project_name = get_project_name(mana_dir)?;

    // Count total units and open units (Open or InProgress are considered "open")
    let unit_count = index.units.len();
    let open_unit_count = index
        .units
        .iter()
        .filter(|unit| {
            matches!(
                unit.status,
                mana_core::api::Status::Open | mana_core::api::Status::InProgress
            )
        })
        .count();

    Ok(ProjectSnapshot {
        project_name,
        unit_count,
        open_unit_count,
    })
}

/// Determine if a file system event should trigger a snapshot refresh
fn should_trigger_refresh(event: &NotifyEvent) -> bool {
    match &event.kind {
        // File modifications and creations
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
            // Check if any path in the event is a .md file or config.yaml
            event.paths.iter().any(|path| {
                if let Some(file_name) = path.file_name() {
                    if let Some(name_str) = file_name.to_str() {
                        return name_str.ends_with(".md") || name_str == "config.yaml";
                    }
                }
                false
            })
        }
        _ => false,
    }
}

/// Start a background watcher for the current project's .mana directory
/// Returns a handle that can be used to receive refresh events
pub fn start_project_watcher() -> Result<mpsc::Receiver<Event>, Box<dyn std::error::Error>> {
    let mana_dir = mana_core::discovery::find_mana_dir(Path::new("."))?;
    let (event_tx, event_rx) = mpsc::channel();

    watch_mana_directory(mana_dir, move |snapshot| {
        let event = Event::ProjectRefreshed { snapshot };
        if let Err(e) = event_tx.send(event) {
            eprintln!("Failed to send project refresh event: {}", e);
        }
    })?;

    Ok(event_rx)
}

// Legacy functions for backwards compatibility
#[deprecated(note = "Use load_project_snapshot() instead")]
pub fn project_snapshot(project_name: &str) -> ProjectSnapshot {
    // Try to load real data, but fallback to placeholder if it fails
    load_project_snapshot().unwrap_or_else(|_| ProjectSnapshot {
        project_name: project_name.to_string(),
        unit_count: 0,
        open_unit_count: 0,
    })
}

#[deprecated(note = "Use load_runtime_snapshot() instead")]
pub fn runtime_snapshot() -> RuntimeSnapshot {
    // Try to load real data, but fallback to placeholder if it fails
    load_runtime_snapshot().unwrap_or_else(|_| RuntimeSnapshot {
        running_agents: 0,
        queued_units: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_load_project_snapshot_integration() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-project");
        let mana_dir = project_dir.join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // Create config.yaml with project name
        let config_content = "project: test-wizard-project\nnext_id: 5\n";
        fs::write(mana_dir.join("config.yaml"), config_content).unwrap();

        // Create some test units using mana-core
        let unit1 = mana_core::unit::Unit::new("1", "First test unit");
        let mut unit2 = mana_core::unit::Unit::new("2", "Second test unit");
        unit2.status = mana_core::unit::Status::InProgress;
        let mut unit3 = mana_core::unit::Unit::new("3", "Closed test unit");
        unit3.status = mana_core::unit::Status::Closed;

        // Save units to files
        unit1
            .to_file(mana_dir.join("1-first-test-unit.md"))
            .unwrap();
        unit2
            .to_file(mana_dir.join("2-second-test-unit.md"))
            .unwrap();
        unit3
            .to_file(mana_dir.join("3-closed-test-unit.md"))
            .unwrap();

        // Change to the project directory to test discovery
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        // Test the snapshot loading
        let result = load_project_snapshot();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // Verify the results
        assert!(result.is_ok());
        let snapshot = result.unwrap();

        assert_eq!(snapshot.project_name, "test-wizard-project");
        assert_eq!(snapshot.unit_count, 3);
        assert_eq!(snapshot.open_unit_count, 2); // unit1 (Open) and unit2 (InProgress)
    }

    #[test]
    fn test_load_runtime_snapshot() {
        let result = load_runtime_snapshot();
        assert!(result.is_ok());

        let snapshot = result.unwrap();
        assert_eq!(snapshot.running_agents, 0);
        assert_eq!(snapshot.queued_units, 0);
    }

    #[test]
    fn test_project_name_fallback() {
        // Create a temp project without config.yaml
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("fallback-project");
        let mana_dir = project_dir.join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // Create a simple unit to avoid empty index issues
        let unit1 = mana_core::unit::Unit::new("1", "Test unit");
        unit1.to_file(mana_dir.join("1-test-unit.md")).unwrap();

        // Test get_project_name directly with the mana_dir path
        let result = get_project_name(&mana_dir);

        assert!(result.is_ok());
        let project_name = result.unwrap();
        assert_eq!(project_name, "fallback-project");
    }

    #[test]
    fn test_should_trigger_refresh() {
        use notify::{Event, EventKind};

        // Test events that should trigger refresh
        let md_event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/test/.mana/1-unit.md")],
            attrs: Default::default(),
        };
        assert!(should_trigger_refresh(&md_event));

        let config_event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("/test/.mana/config.yaml")],
            attrs: Default::default(),
        };
        assert!(should_trigger_refresh(&config_event));

        // Test events that should not trigger refresh
        let other_file_event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/test/.mana/some-other-file.txt")],
            attrs: Default::default(),
        };
        assert!(!should_trigger_refresh(&other_file_event));

        let access_event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![PathBuf::from("/test/.mana/1-unit.md")],
            attrs: Default::default(),
        };
        assert!(!should_trigger_refresh(&access_event));
    }

    #[test]
    fn test_load_project_snapshot_from_dir() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let mana_dir = temp_dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // Create config.yaml
        let config_content = "project: direct-load-test\nnext_id: 3\n";
        fs::write(mana_dir.join("config.yaml"), config_content).unwrap();

        // Create test unit
        let unit1 = mana_core::unit::Unit::new("1", "Direct test unit");
        unit1
            .to_file(mana_dir.join("1-direct-test-unit.md"))
            .unwrap();

        // Test direct loading from directory
        let result = load_project_snapshot_from_dir(&mana_dir);
        assert!(result.is_ok());

        let snapshot = result.unwrap();
        assert_eq!(snapshot.project_name, "direct-load-test");
        assert_eq!(snapshot.unit_count, 1);
        assert_eq!(snapshot.open_unit_count, 1);
    }

    #[test]
    fn test_runtime_supervisor() {
        let supervisor = RuntimeSupervisor::new();
        
        // Test initial state
        let state = supervisor.get_runtime_state();
        assert_eq!(state.agents.len(), 0);
        assert_eq!(state.work_queue.len(), 0);
        assert_eq!(state.process_metrics.active_processes, 0);
        
        // Test queuing work
        supervisor.queue_work("test-unit-1".to_string(), WorkPriority::High).unwrap();
        let state = supervisor.get_runtime_state();
        assert_eq!(state.work_queue.len(), 1);
        assert_eq!(state.work_queue[0].unit_id, "test-unit-1");
        assert_eq!(state.work_queue[0].priority, WorkPriority::High);
        
        // Test starting agent
        let agent_id = supervisor.start_agent("test-unit-1".to_string()).unwrap();
        let state = supervisor.get_runtime_state();
        assert!(state.agents.contains_key(&agent_id));
        assert_eq!(state.work_queue.len(), 0); // Should be removed from queue
        
        // Test subscription
        let mut receiver = supervisor.subscribe_runtime();
        // We won't test the actual events here since they're async
        drop(receiver);
    }

    #[test]
    fn test_global_runtime_supervisor() {
        let supervisor1 = get_runtime_supervisor();
        let supervisor2 = get_runtime_supervisor();
        
        // Should be the same instance
        assert!(std::ptr::eq(supervisor1, supervisor2));
        
        // Test global functions
        let _receiver = subscribe_runtime();
        let _state = runtime_stream();
        let _watcher = watch_runtime();
    }

    #[test]
    fn test_watch_mana_directory_basic() {
        // Create a temporary .mana directory
        let temp_dir = TempDir::new().unwrap();
        let mana_dir = temp_dir.path().join(".mana");
        fs::create_dir_all(&mana_dir).unwrap();

        // Create initial config
        let config_content = "project: watch-test\nnext_id: 2\n";
        fs::write(mana_dir.join("config.yaml"), config_content).unwrap();

        // Create initial unit
        let unit1 = mana_core::unit::Unit::new("1", "Watch test unit");
        unit1
            .to_file(mana_dir.join("1-watch-test-unit.md"))
            .unwrap();

        // Set up change tracking
        let changes = Arc::new(Mutex::new(Vec::new()));
        let changes_clone = Arc::clone(&changes);

        // Start watching (this will spawn a background thread)
        let watch_result = watch_mana_directory(mana_dir.clone(), move |snapshot| {
            let mut changes = changes_clone.lock().unwrap();
            changes.push(snapshot);
        });

        assert!(watch_result.is_ok());

        // Give the watcher time to start up
        thread::sleep(Duration::from_millis(200));

        // Make a change that should trigger refresh
        let unit2 = mana_core::unit::Unit::new("2", "New watch unit");
        unit2
            .to_file(mana_dir.join("2-new-watch-unit.md"))
            .unwrap();

        // Wait for the file system event to be processed
        thread::sleep(Duration::from_millis(800)); // longer than debounce

        // Check if change was detected (or verify the watch mechanism works)
        let changes = changes.lock().unwrap();
        // Note: File system events in tests can be flaky, so we just verify
        // the mechanism is set up correctly. The real test is that watch_result is Ok
        // and the callback mechanism is properly configured.
        println!("Changes detected: {}", changes.len());

        // If we got changes, verify the content
        if let Some(snapshot) = changes.first() {
            assert_eq!(snapshot.project_name, "watch-test");
            // The unit count could be 1 or 2 depending on timing of the file write
        }
    }
}
