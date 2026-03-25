use std::path::Path;

use project_detect::{NodePM, ProjectKind};

/// Suggest a verify command based on detected project type.
///
/// Delegates detection to `project-detect` (29 ecosystems) and maps
/// each kind to a sensible default test/check command.
pub fn suggest_verify_command(project_dir: &Path) -> Option<&'static str> {
    let kind = project_detect::detect(project_dir)?;
    Some(match kind {
        ProjectKind::Cargo => "cargo test",
        ProjectKind::Go => "go test ./...",
        ProjectKind::Elixir { .. } => "mix test",
        ProjectKind::Python { uv: true } => "uv run pytest",
        ProjectKind::Python { uv: false } => "pytest",
        ProjectKind::Node {
            manager: NodePM::Bun,
        } => "bun test",
        ProjectKind::Node {
            manager: NodePM::Pnpm,
        } => "pnpm test",
        ProjectKind::Node {
            manager: NodePM::Yarn,
        } => "yarn test",
        ProjectKind::Node {
            manager: NodePM::Npm,
        } => "npm test",
        ProjectKind::Gradle { wrapper: true } => "./gradlew test",
        ProjectKind::Gradle { wrapper: false } => "gradle test",
        ProjectKind::Maven => "mvn test",
        ProjectKind::Ruby => "bundle exec rspec",
        ProjectKind::Swift => "swift test",
        ProjectKind::Zig => "zig build test",
        ProjectKind::DotNet { .. } => "dotnet test",
        ProjectKind::Php => "composer test",
        ProjectKind::Dart { flutter: true } => "flutter test",
        ProjectKind::Dart { flutter: false } => "dart test",
        ProjectKind::Sbt => "sbt test",
        ProjectKind::Haskell { stack: true } => "stack test",
        ProjectKind::Haskell { stack: false } => "cabal test",
        ProjectKind::Clojure { lein: true } => "lein test",
        ProjectKind::Clojure { lein: false } => "clj -M:test",
        ProjectKind::Rebar => "rebar3 eunit",
        ProjectKind::Dune => "dune test",
        ProjectKind::Perl => "prove -l",
        ProjectKind::Julia => "julia -e 'using Pkg; Pkg.test()'",
        ProjectKind::Nim => "nimble test",
        ProjectKind::Crystal => "crystal spec",
        ProjectKind::Vlang => "v test .",
        ProjectKind::Gleam => "gleam test",
        ProjectKind::Lua => "busted",
        ProjectKind::Bazel => "bazel test //...",
        ProjectKind::Meson => "meson test -C builddir",
        ProjectKind::CMake => "ctest --test-dir build",
        ProjectKind::Make => "make test",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn suggest_cargo_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("cargo test"));
    }

    #[test]
    fn suggest_npm_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("npm test"));
    }

    #[test]
    fn suggest_bun_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::write(dir.path().join("bun.lockb"), "").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("bun test"));
    }

    #[test]
    fn suggest_python_pytest() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]").unwrap();
        let result = suggest_verify_command(dir.path()).unwrap();
        assert!(result == "pytest" || result == "uv run pytest");
    }

    #[test]
    fn suggest_go_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("go.mod"), "module example").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("go test ./..."));
    }

    #[test]
    fn suggest_mix_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("mix.exs"), "").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("mix test"));
    }

    #[test]
    fn suggest_ruby_rspec() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Gemfile"), "source 'https://rubygems.org'").unwrap();
        assert_eq!(
            suggest_verify_command(dir.path()),
            Some("bundle exec rspec")
        );
    }

    #[test]
    fn suggest_zig_test() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("build.zig"), "").unwrap();
        assert_eq!(suggest_verify_command(dir.path()), Some("zig build test"));
    }

    #[test]
    fn empty_dir_returns_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(suggest_verify_command(dir.path()), None);
    }
}
