//! Everything that depends on the machine and on how the app is launched is
//! read HERE and only here. The literal in each case is the default: without
//! the env var, behavior is exactly what it was before this file existed.

/// Empty string and whitespace are not a value: take the default, don't fail.
pub fn env_str(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

/// A non-number and a negative are also not a value. Zero is allowed.
pub fn env_int(name: &str, fallback: u64) -> u64 {
    let v = match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return fallback,
    };
    // u64 parse: negative and garbage both miss, which is exactly the contract.
    v.trim().parse::<u64>().unwrap_or(fallback)
}

/// The Claude Code binary. On Windows it's `claude.cmd` — hence configurable.
pub fn claude_bin() -> String {
    env_str("CLAUDE_BIN", "claude")
}

/// The RTK proxy that reports its own savings. On Windows it may be `rtk.cmd`.
pub fn rtk_bin() -> String {
    env_str("RTK_BIN", "rtk")
}

/// Installation goes to the network and clones someone else's repo — generous margin.
pub fn timeout_install_ms() -> u64 {
    env_int("CC_TIMEOUT_INSTALL", 180_000)
}

/// Live MCP probe: the other process must have time to start and respond.
pub fn timeout_probe_ms() -> u64 {
    env_int("CC_TIMEOUT_PROBE", 25_000)
}

/// A single HTTP request to the external index or to raw.githubusercontent.
pub fn timeout_net_ms() -> u64 {
    env_int("CC_TIMEOUT_NET", 15_000)
}

/// The MCP registry is measurably slower than the plugin index — 9–15 s per
/// search. Its own, generous budget.
pub fn timeout_mcp_ms() -> u64 {
    env_int("CC_TIMEOUT_MCP", 40_000)
}

pub fn remote_base() -> String {
    env_str("CC_REMOTE_BASE", "https://skillselion.com")
}

/// The official MCP registry. Read-only for us: we search it and install nothing from it.
pub fn mcp_registry_base() -> String {
    env_str(
        "CC_MCP_REGISTRY_BASE",
        "https://registry.modelcontextprotocol.io/v0",
    )
}

/// Index data updates once a day — no point going more often.
pub fn remote_ttl_ms() -> u64 {
    env_int("CC_REMOTE_TTL", 24 * 60 * 60 * 1000)
}

/// The host of a base url — what the catalog window prints as its source.
pub fn host_of(base: &str) -> String {
    let rest = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
        .unwrap_or(base);
    rest.split(['/', '?', ':'])
        .next()
        .unwrap_or(base)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_strips_scheme_and_path() {
        assert_eq!(host_of("https://skillselion.com"), "skillselion.com");
        assert_eq!(host_of("http://localhost:5181/api"), "localhost");
        assert_eq!(host_of("not a url"), "not a url");
    }

    #[test]
    fn env_int_rejects_garbage() {
        // Not the real var names, but the parse contract is the same.
        assert_eq!(env_int("CL_NOPE_A", 7), 7);
        assert_eq!(env_int("CL_NOPE_B", 7), 7);
    }
}
