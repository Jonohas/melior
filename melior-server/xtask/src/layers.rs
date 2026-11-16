//! Dependency layering rules for the workspace.
//!
//! The workspace follows domain-driven design, and these rules are what keeps
//! the dependency direction from being a convention only:
//!
//! - Every crate has a layer.
//! - A crate depends only on workspace crates of a lower layer. Two crates of
//!   the same layer (the adapters: infrastructure and interfaces) therefore
//!   never depend on each other.
//! - A crate does not use an external crate that is banned for it. The domain
//!   uses nothing asynchronous and nothing that does I/O; the application uses
//!   no adapter library; each adapter crate keeps to its own side.
//!
//! The rule engine works on a small model that does not need `cargo metadata`,
//! so the unit tests can examine it. [`from_metadata`] makes the model from
//! the output of `cargo metadata`.

use serde_json::Value;

/// What the rules say about one workspace crate.
pub struct Rule {
    pub name: &'static str,
    pub layer: u8,
    /// External crates this crate must not depend on. An entry that ends in
    /// `*` is a prefix.
    pub banned: &'static [&'static str],
}

/// Libraries that talk to the outside world. Only an adapter may use one.
const ADAPTER_LIBRARIES: &[&str] = &[
    "axum*",
    "hyper*",
    "tower*",
    "webrtc*",
    "redis",
    "sqlx*",
    "reqwest",
    "jsonwebtoken",
    "opentelemetry*",
    "tokio-tungstenite",
];

/// The rules for each workspace crate. Exempt tooling is not in the table.
pub const TABLE: &[Rule] = &[
    Rule {
        name: "melior-domain",
        layer: 0,
        // Pure and synchronous: also no runtime, no async, no logging.
        banned: &[
            "tokio*",
            "async-trait",
            "futures*",
            "tracing*",
            "serde_json",
            "axum*",
            "hyper*",
            "tower*",
            "webrtc*",
            "redis",
            "sqlx*",
            "reqwest",
            "jsonwebtoken",
            "opentelemetry*",
        ],
    },
    Rule { name: "melior-application", layer: 1, banned: ADAPTER_LIBRARIES },
    // Driven adapters: no web framework.
    Rule { name: "melior-infrastructure", layer: 2, banned: &["axum*", "tower*"] },
    // Driving adapters: no media stack, storage client or HTTP client.
    Rule {
        name: "melior-interfaces",
        layer: 2,
        banned: &["webrtc*", "redis", "sqlx*", "reqwest", "jsonwebtoken", "opentelemetry*"],
    },
    // The composition root may know everything.
    Rule { name: "melior-sfu", layer: 3, banned: &[] },
];

/// Crates that are exempt from the rules.
pub const EXEMPT: &[&str] = &["xtask"];

/// One dependency edge of a workspace crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// The package that has the dependency.
    pub from: String,
    /// The package that `from` depends on.
    pub to: String,
    /// True if `to` is a workspace crate.
    pub to_workspace: bool,
    /// True if the dependency is a dev-dependency.
    pub dev: bool,
}

fn rule(name: &str) -> Option<&'static Rule> {
    TABLE.iter().find(|rule| rule.name == name)
}

fn is_banned(banned: &[&str], name: &str) -> bool {
    banned.iter().any(|pattern| match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *pattern,
    })
}

/// Returns one message for each crate or edge that breaks a rule.
///
/// Dev-dependencies on external crates are not checked: a test may use any
/// library. Dev-dependencies on workspace crates are, so no test makes a
/// lower layer know a higher one.
#[must_use]
pub fn violations(crates: &[String], edges: &[Edge]) -> Vec<String> {
    let mut out: Vec<String> = crates
        .iter()
        .filter(|name| rule(name).is_none() && !EXEMPT.contains(&name.as_str()))
        .map(|name| format!("{name} has no layer; add it to TABLE in xtask/src/layers.rs"))
        .collect();

    for edge in edges {
        let Some(from) = rule(&edge.from) else {
            continue;
        };
        if edge.to_workspace {
            match rule(&edge.to) {
                None => {
                    out.push(format!("{} depends on {}, which has no layer", edge.from, edge.to));
                }
                Some(to) if to.layer >= from.layer => out.push(format!(
                    "{} (L{}) depends on {} (L{}); a crate depends only on lower layers",
                    edge.from, from.layer, edge.to, to.layer
                )),
                Some(_) => {}
            }
        } else if !edge.dev && is_banned(from.banned, &edge.to) {
            out.push(format!(
                "{} (L{}) uses {}, which is not allowed in that crate",
                edge.from, from.layer, edge.to
            ));
        }
    }
    out
}

/// Makes the crate list and the edges from the JSON of `cargo metadata
/// --no-deps --format-version 1`.
#[must_use]
pub fn from_metadata(meta: &Value) -> (Vec<String>, Vec<Edge>) {
    let packages = meta.get("packages").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let names: Vec<String> = packages
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let mut edges = Vec::new();
    for package in packages {
        let Some(from) = package.get("name").and_then(Value::as_str) else {
            continue;
        };
        let deps =
            package.get("dependencies").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        for dep in deps {
            let Some(to) = dep.get("name").and_then(Value::as_str) else {
                continue;
            };
            edges.push(Edge {
                from: from.to_owned(),
                to: to.to_owned(),
                to_workspace: dep.get("path").is_some_and(|p| !p.is_null()),
                dev: dep.get("kind").and_then(Value::as_str) == Some("dev"),
            });
        }
    }
    (names, edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn edge(from: &str, to: &str, to_workspace: bool) -> Edge {
        Edge { from: from.into(), to: to.into(), to_workspace, dev: false }
    }

    fn check(edges: &[Edge]) -> Vec<String> {
        violations(&[], edges)
    }

    #[test]
    fn dependencies_on_lower_layers_are_fine() {
        assert_eq!(
            check(&[
                edge("melior-application", "melior-domain", true),
                edge("melior-infrastructure", "melior-application", true),
                edge("melior-interfaces", "melior-domain", true),
                edge("melior-sfu", "melior-infrastructure", true),
                edge("melior-sfu", "melior-interfaces", true),
            ]),
            [] as [std::string::String; 0]
        );
    }

    #[test]
    fn a_lower_layer_cannot_know_a_higher_one() {
        let found = check(&[edge("melior-domain", "melior-application", true)]);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("melior-domain (L0) depends on melior-application (L1)"));

        assert_eq!(check(&[edge("melior-application", "melior-infrastructure", true)]).len(), 1);
    }

    #[test]
    fn the_two_adapter_crates_cannot_know_each_other() {
        assert_eq!(check(&[edge("melior-interfaces", "melior-infrastructure", true)]).len(), 1);
        assert_eq!(check(&[edge("melior-infrastructure", "melior-interfaces", true)]).len(), 1);
    }

    #[test]
    fn a_dev_dependency_does_not_excuse_a_workspace_edge() {
        let mut upward = edge("melior-application", "melior-infrastructure", true);
        upward.dev = true;
        assert_eq!(check(&[upward]).len(), 1);
    }

    #[test]
    fn the_domain_stays_pure() {
        for banned in ["tokio", "tokio-util", "async-trait", "axum", "webrtc", "redis", "tracing"] {
            assert_eq!(check(&[edge("melior-domain", banned, false)]).len(), 1, "{banned}");
        }
        assert_eq!(
            check(&[edge("melior-domain", "thiserror", false)]),
            [] as [std::string::String; 0]
        );
    }

    #[test]
    fn the_application_uses_no_adapter_library() {
        for banned in ["axum", "axum-extra", "webrtc", "webrtc-ice", "redis", "reqwest"] {
            assert_eq!(check(&[edge("melior-application", banned, false)]).len(), 1, "{banned}");
        }
        assert_eq!(
            check(&[edge("melior-application", "tokio", false)]),
            [] as [std::string::String; 0]
        );
    }

    #[test]
    fn each_adapter_keeps_to_its_side() {
        assert_eq!(check(&[edge("melior-infrastructure", "axum", false)]).len(), 1);
        assert_eq!(check(&[edge("melior-interfaces", "webrtc", false)]).len(), 1);
        assert_eq!(check(&[edge("melior-interfaces", "redis", false)]).len(), 1);
        assert_eq!(
            check(&[edge("melior-interfaces", "axum", false)]),
            [] as [std::string::String; 0]
        );
        assert_eq!(
            check(&[edge("melior-infrastructure", "webrtc", false)]),
            [] as [std::string::String; 0]
        );
    }

    #[test]
    fn tests_may_use_any_external_library() {
        let mut dev = edge("melior-application", "reqwest", false);
        dev.dev = true;
        assert_eq!(check(&[dev]), [] as [std::string::String; 0]);
    }

    #[test]
    fn a_new_crate_must_be_given_a_layer() {
        let found = violations(&["melior-new".to_owned(), "xtask".to_owned()], &[]);
        assert_eq!(found, ["melior-new has no layer; add it to TABLE in xtask/src/layers.rs"]);

        let found = check(&[edge("melior-sfu", "melior-new", true)]);
        assert!(found[0].contains("which has no layer"));
    }

    #[test]
    fn reads_cargo_metadata() {
        let meta = json!({
            "packages": [{
                "name": "melior-application",
                "dependencies": [
                    { "name": "melior-domain", "kind": null, "path": "/ws/crates/melior-domain" },
                    { "name": "tokio", "kind": null },
                    { "name": "reqwest", "kind": "dev" }
                ]
            }]
        });

        let (crates, edges) = from_metadata(&meta);
        assert_eq!(crates, ["melior-application"]);
        assert_eq!(
            edges,
            [
                edge("melior-application", "melior-domain", true),
                edge("melior-application", "tokio", false),
                Edge {
                    from: "melior-application".into(),
                    to: "reqwest".into(),
                    to_workspace: false,
                    dev: true
                },
            ]
        );
        assert_eq!(violations(&crates, &edges), [] as [std::string::String; 0]);
    }
}
