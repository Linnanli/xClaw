//! CLI subcommand definitions for `ironclaw service`.

use clap::Subcommand;

use crate::service::ServiceAction;

#[derive(Subcommand, Debug, Clone)]
pub enum ServiceCommand {
    /// Install the OS service (launchd on macOS, systemd on Linux).
    Install,
    /// Start the installed service.
    Start,
    /// Stop the running service.
    Stop,
    /// Show service status.
    Status,
    /// Uninstall the OS service and remove the unit file (also clears any
    /// legacy `ironclaw.*` unit left over from before ADR-114 Ⅳ).
    Uninstall,
    /// Migrate a pre-ADR-114 `ironclaw.*` service install to the renamed
    /// `dasclaw.*` family: stop legacy → uninstall legacy → install new → start.
    Migrate,
}

impl ServiceCommand {
    /// Convert the CLI variant into the domain action.
    pub fn to_action(&self) -> ServiceAction {
        match self {
            ServiceCommand::Install => ServiceAction::Install,
            ServiceCommand::Start => ServiceAction::Start,
            ServiceCommand::Stop => ServiceAction::Stop,
            ServiceCommand::Status => ServiceAction::Status,
            ServiceCommand::Uninstall => ServiceAction::Uninstall,
            ServiceCommand::Migrate => ServiceAction::Migrate,
        }
    }
}

/// Run the service command.
pub fn run_service_command(cmd: &ServiceCommand) -> anyhow::Result<()> {
    crate::service::handle_command(&cmd.to_action())
}
