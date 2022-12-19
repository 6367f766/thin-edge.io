use crate::{cli::common::Cloud, command::Command};
use std::sync::Arc;
use tedge_config::system_services::SystemServiceManager;

#[derive(Debug)]
pub struct ReconnectCommand {
    pub cloud: Cloud,
    pub service_manager: Arc<dyn SystemServiceManager>,
}

impl ReconnectCommand {}

impl Command for ReconnectCommand {
    fn description(&self) -> String {
        format!("reconnect {:?} cloud", self.cloud)
    }

    fn execute(&self) -> anyhow::Result<()> {
        self.service_manager
            .apply_changes_to_mosquitto(self.cloud.as_str())?;

        self.service_manager.restart_service_if_running(
            tedge_config::system_services::SystemService::TEdgeSMAgent,
        )?;
        self.service_manager.restart_service_if_running(
            tedge_config::system_services::SystemService::TEdgeMapperC8y,
        )?;

        Ok(())
    }
}
