pub mod loader;
pub mod migration;
pub mod runner;
pub mod types;

pub use loader::{
    discover, discover_all, DiscoveredExtension, DiscoveryOptions, DiscoveryResult, EventHandler,
    ExtensionError, ExtensionManifest, ExtensionMetadata, ExtensionRegistry, LegacyAgentSection,
    LegacyAgentToml, LegacyModelSection, LegacyPromptSection, ToolDefinition, ToolParameter,
};
pub use migration::{
    convert_legacy_to_extension_toml, is_legacy_agent_file, is_legacy_agent_format,
    load_registry_with_migration, reset_deprecation_warning_flag, DeprecationConfig, MigrationShim,
    MigrationStats,
};
pub use runner::{
    map_with_concurrency_limit, map_with_concurrency_limit_ordered, spawn_agent,
    spawn_agents_concurrent, spawn_agents_with_limit, spawn_subagent, AgentEvent, AgentResult,
    AgentRunner, ConcurrentSpawnConfig, ConcurrentSpawnResult, ExtensionRunner,
    ExtensionRunnerError, SpawnConfig, ToolCallResult,
};
pub use types::{ExtensionId, ToolFn};
