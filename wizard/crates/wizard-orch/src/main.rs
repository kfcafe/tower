use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "status" => {
            if let Err(e) = status_command() {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("wiz - Wizard orchestration CLI");
    println!();
    println!("USAGE:");
    println!("    wiz <command>");
    println!();
    println!("COMMANDS:");
    println!("    status    Show project and runtime status");
    println!("    help      Show this help message");
}

fn status_command() -> Result<(), Box<dyn std::error::Error>> {
    // Load project snapshot using the existing snapshot loader
    let project_snapshot = wizard_orch::load_project_snapshot()?;
    let runtime_snapshot = wizard_orch::load_runtime_snapshot()?;

    // Display project information
    println!("Project Status");
    println!("==============");
    println!("Project:     {}", project_snapshot.project_name);
    println!("Total units: {}", project_snapshot.unit_count);
    println!("Open units:  {}", project_snapshot.open_unit_count);

    let completed_units = project_snapshot.unit_count - project_snapshot.open_unit_count;
    println!("Completed:   {}", completed_units);

    if project_snapshot.unit_count > 0 {
        let completion_rate = (completed_units as f64 / project_snapshot.unit_count as f64) * 100.0;
        println!("Progress:    {:.1}%", completion_rate);
    }

    println!();
    println!("Runtime Status");
    println!("==============");
    println!("Running agents: {}", runtime_snapshot.running_agents);
    println!("Queued units:   {}", runtime_snapshot.queued_units);

    Ok(())
}
