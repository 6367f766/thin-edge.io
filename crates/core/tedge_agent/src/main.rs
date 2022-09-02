use std::path::PathBuf;

use agent::SmAgentConfig;
use clap::Parser;
use tedge_config::DEFAULT_TEDGE_CONFIG_PATH;

mod agent;
mod error;
mod http_rest;
mod restart_operation_handler;
mod state;

#[derive(Debug, clap::Parser)]
#[clap(
name = clap::crate_name!(),
version = clap::crate_version!(),
about = clap::crate_description!()
)]
pub struct AgentOpt {
    /// Turn-on the debug log level.
    ///
    /// If off only reports ERROR, WARN, and INFO
    /// If on also reports DEBUG and TRACE
    #[clap(long)]
    pub debug: bool,

    /// Start the agent with clean session off, subscribe to the topics, so that no messages are lost
    #[clap(short, long)]
    pub init: bool,

    /// Start the agent with clean session on, drop the previous session and subscriptions
    ///
    /// WARNING: All pending messages will be lost.
    #[clap(short, long)]
    pub clear: bool,

    /// Start the agent from custom path
    ///
    /// WARNING: This is mostly used in testing.
    #[clap(long = "config-dir", default_value = DEFAULT_TEDGE_CONFIG_PATH)]
    pub config_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let agent_opt = AgentOpt::parse();
    tedge_utils::logging::initialise_tracing_subscriber(agent_opt.debug);

    let tedge_config_location =
        tedge_config::TEdgeConfigLocation::from_custom_root(agent_opt.config_dir.clone());
    let mut agent = agent::SmAgent::try_new(
        "tedge_agent",
        SmAgentConfig::try_new(tedge_config_location)?,
    )?;
    if agent_opt.init {
        agent.init(agent_opt.config_dir).await?;
    } else if agent_opt.clear {
        agent.clear_session().await?;
    } else {
        agent.start().await?;
    }
    Ok(())
}
