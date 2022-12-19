use tedge_config::system_services::service_manager;

use crate::{cli::common::Cloud, command::*};

use super::reconnect::ReconnectCommand;

const C8Y_CONFIG_FILENAME: &str = "c8y-bridge.conf";
const AZURE_CONFIG_FILENAME: &str = "az-bridge.conf";

#[derive(clap::Subcommand, Debug)]
pub enum TEdgeReconnectCli {
    /// Remove bridge connection to Cumulocity.
    C8y,
    /// Remove bridge connection to Azure.
    Az,
}

impl BuildCommand for TEdgeReconnectCli {
    fn build_command(self, context: BuildContext) -> Result<Box<dyn Command>, crate::ConfigError> {
        let cmd = match self {
            TEdgeReconnectCli::C8y => ReconnectCommand {
                cloud: Cloud::C8y,
                service_manager: service_manager(context.config_location.tedge_config_root_path)?,
            },
            TEdgeReconnectCli::Az => {
                todo!()
            }
        };
        Ok(cmd.into_boxed())
    }
}
