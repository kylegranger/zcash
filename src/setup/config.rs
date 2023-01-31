use std::{
    collections::HashSet,
    ffi::OsString,
    fmt::Write,
    fs, io,
    io::{Error, ErrorKind},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::setup::node::Action;

// The names of the files the node configurations will be written to.
const ZEBRA_CONFIG: &str = "zebra.toml";
const ZCASHD_CONFIG: &str = "zcash.conf";
const ZCASHD_CACHE: &str = "testnet3";

// Ziggurat's configuration directory and file. Caches are written to this directory.
const CONFIG: &str = ".ziggurat";
const CONFIG_FILE: &str = "config.toml";

const DEFAULT_PORT: u16 = 8080;

/// Convenience struct for reading Ziggurat's configuration file.
#[derive(Deserialize)]
struct ConfigFile {
    kind: NodeKind,
    path: PathBuf,
    start_command: String,
}

/// Node configuration abstracted by a [`Node`] instance.
///
/// The information contained in this struct will be written to a config file read by the node at
/// start time and as such can only contain options shared by all types of node.
///
/// [`Node`]: struct@crate::setup::node::Node
pub(super) struct NodeConfig {
    /// The path of the cache directory of the node; this is `~/.ziggurat`.
    pub(super) path: PathBuf,
    /// The socket address of the node.
    pub(super) local_addr: SocketAddr,
    /// The initial peerset to connect to on node start.
    pub(super) initial_peers: HashSet<String>,
    /// The initial max number of peer connections to allow.
    pub(super) max_peers: usize,
    /// Setting this option to true will enable node logging to stdout.
    pub(super) log_to_stdout: bool,
    /// Defines the initial action to take once the node has started.
    pub(super) initial_action: Action,
}

impl NodeConfig {
    pub(super) fn new() -> io::Result<Self> {
        // Set the port explicitly.
        let mut local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        local_addr.set_port(DEFAULT_PORT);

        Ok(Self {
            path: home::home_dir()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "couldn't find home directory"))?
                .join(CONFIG),
            local_addr,
            initial_peers: HashSet::new(),
            max_peers: 50,
            log_to_stdout: false,
            initial_action: Action::None,
        })
    }
}

/// Describes the node kind, currently supports the two known variants.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all(deserialize = "lowercase"))]
pub(super) enum NodeKind {
    Zebra,
    Zcashd,
}

impl NodeKind {
    /// Path to the configuration file for this [NodeKind]
    pub(super) fn config_filepath(&self, wrapping_dir: &Path) -> PathBuf {
        match self {
            NodeKind::Zebra => wrapping_dir.join(ZEBRA_CONFIG),
            NodeKind::Zcashd => wrapping_dir.join(ZCASHD_CONFIG),
        }
    }

    pub(super) fn cache_path(&self, wrapping_dir: &Path) -> Option<PathBuf> {
        match self {
            NodeKind::Zebra => None,
            NodeKind::Zcashd => Some(wrapping_dir.join(ZCASHD_CACHE)),
        }
    }
}

/// Node configuration read from the `config.toml` file.
#[derive(Clone)]
pub(super) struct NodeMetaData {
    /// The node kind (one of `Zebra` or `Zcashd`).
    pub(super) kind: NodeKind,
    /// The path to run the node's commands in.
    pub(super) path: PathBuf,
    /// The command to run when starting a node.
    pub(super) start_command: OsString,
    /// The args to run with the start command.
    pub(super) start_args: Vec<OsString>,
}

impl NodeMetaData {
    pub(super) fn new(config_path: PathBuf) -> io::Result<Self> {
        // Read Ziggurat's configuration file.
        let path = config_path.join(CONFIG_FILE);
        let config_string = fs::read_to_string(path)?;
        let config_file: ConfigFile =
            toml::from_str(&config_string).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

        let args_from = |command: &str| -> Vec<OsString> {
            command.split_whitespace().map(OsString::from).collect()
        };

        let mut start_args = args_from(&config_file.start_command);
        let start_command = start_args.remove(0);

        // Insert the node's config file path into start args.
        let config_file_path = config_file.kind.config_filepath(&config_path);
        match config_file.kind {
            NodeKind::Zebra => {
                // Zebra's final arg must be `start`, so we insert the actual args before it.
                let n_args = start_args.len();
                if n_args < 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Expected at least one start_command arg for Zebra (`start`)",
                    ));
                }
                start_args.insert(n_args - 1, "--config".into());
                start_args.insert(n_args, config_file_path.into_os_string());
            }
            NodeKind::Zcashd => {
                start_args.push(format!("-datadir={}", config_path.to_str().unwrap()).into());
            }
        }

        Ok(Self {
            kind: config_file.kind,
            path: config_file.path,
            start_command,
            start_args,
        })
    }
}

/// Convenience struct for writing a zebra compatible configuration file.
#[derive(Serialize)]
pub(super) struct ZebraConfigFile {
    network: NetworkConfig,
    state: StateConfig,
    tracing: TracingConfig,
}

impl ZebraConfigFile {
    /// Generate the toml configuration as a string.
    pub(super) fn generate(config: &NodeConfig) -> Result<String, toml::ser::Error> {
        // Create the structs to prepare for encoding.
        let initial_testnet_peers: HashSet<String> = config
            .initial_peers
            .iter()
            .map(|addr| addr.to_string())
            .collect();

        let zebra_config = Self {
            network: NetworkConfig {
                // Set ip from config, port from assigned in `Config`.
                listen_addr: config.local_addr,
                initial_testnet_peers,
                peerset_initial_target_size: config.max_peers,
                network: String::from("Testnet"),
            },
            state: StateConfig {
                cache_dir: None,
                ephemeral: true,
            },
            tracing: TracingConfig {
                filter: Some("zebra_network=trace,zebrad=trace".to_string()),
            },
        };

        // Write the toml to a string.
        toml::to_string(&zebra_config)
    }
}

#[derive(Serialize)]
struct NetworkConfig {
    listen_addr: SocketAddr,
    initial_testnet_peers: HashSet<String>,
    peerset_initial_target_size: usize,
    network: String,
}

#[derive(Serialize)]
struct StateConfig {
    cache_dir: Option<String>,
    ephemeral: bool,
}

#[derive(Serialize)]
struct TracingConfig {
    filter: Option<String>,
}

/// Convenience struct for writing a zcashd compatible configuration file.
pub(super) struct ZcashdConfigFile;

impl ZcashdConfigFile {
    pub(super) fn generate(config: &NodeConfig) -> String {
        let mut contents = format!(
            "testnet=1\nwhitebind={}\nmaxconnections={}\n",
            config.local_addr, config.max_peers
        );

        if config.initial_peers.is_empty() {
            contents.push_str("addnode=\n")
        } else {
            for peer in &config.initial_peers {
                let _ = writeln!(contents, "addnode={peer}");
            }
        }

        contents
    }
}
