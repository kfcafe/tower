use sysinfo::System;

/// Returns available system memory in MB, or None if it can't be determined.
pub fn available_memory_mb() -> Option<u64> {
    let mut sys = System::new();
    sys.refresh_memory();
    let available = sys.available_memory();
    if available == 0 {
        None
    } else {
        Some(available / (1024 * 1024))
    }
}

/// Check if there's enough available memory to spawn another agent.
/// Returns true if reserve_mb is 0 (disabled) or enough memory is available.
pub fn has_sufficient_memory(reserve_mb: u64) -> bool {
    has_sufficient_memory_with(reserve_mb, available_memory_mb)
}

fn has_sufficient_memory_with(reserve_mb: u64, get_available: impl Fn() -> Option<u64>) -> bool {
    if reserve_mb == 0 {
        return true;
    }
    get_available().map_or(true, |available| available >= reserve_mb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_when_reserve_is_zero() {
        assert!(has_sufficient_memory_with(0, || Some(100)));
        assert!(has_sufficient_memory_with(0, || None));
    }

    #[test]
    fn allows_when_enough_memory() {
        assert!(has_sufficient_memory_with(2048, || Some(4096)));
        assert!(has_sufficient_memory_with(2048, || Some(2048)));
    }

    #[test]
    fn blocks_when_memory_low() {
        assert!(!has_sufficient_memory_with(2048, || Some(1024)));
        assert!(!has_sufficient_memory_with(2048, || Some(2047)));
    }

    #[test]
    fn allows_when_unavailable() {
        // Can't determine memory — allow spawn (don't block on uncertainty)
        assert!(has_sufficient_memory_with(2048, || None));
    }

    #[test]
    fn available_memory_returns_something() {
        // Smoke test: on any real system this should return Some
        let mem = available_memory_mb();
        assert!(mem.is_some(), "expected to read system memory");
        assert!(mem.unwrap() > 0, "expected non-zero available memory");
    }
}
