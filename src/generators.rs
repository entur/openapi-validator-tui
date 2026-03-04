//! Built-in generator registry.
//!
//! Embeds the curated generator config YAML files at compile time and
//! exposes lookup helpers used by the pipeline and default-population logic.

/// A built-in generator definition.
pub struct GeneratorDef {
    pub name: &'static str,
    pub scope: &'static str,
    pub config_yaml: &'static str,
}

// ── Server generators ────────────────────────────────────────────────

static SERVER_GENERATORS: &[GeneratorDef] = &[
    GeneratorDef {
        name: "spring",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/spring.yaml"),
    },
    GeneratorDef {
        name: "kotlin-spring",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/kotlin-spring.yaml"),
    },
    GeneratorDef {
        name: "go-server",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/go-server.yaml"),
    },
    GeneratorDef {
        name: "python-fastapi",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/python-fastapi.yaml"),
    },
    GeneratorDef {
        name: "aspnetcore",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/aspnetcore.yaml"),
    },
    GeneratorDef {
        name: "typescript-nestjs",
        scope: "server",
        config_yaml: include_str!("../assets/generators/server/typescript-nestjs.yaml"),
    },
];

// ── Client generators ────────────────────────────────────────────────

static CLIENT_GENERATORS: &[GeneratorDef] = &[
    GeneratorDef {
        name: "java",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/java.yaml"),
    },
    GeneratorDef {
        name: "kotlin",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/kotlin.yaml"),
    },
    GeneratorDef {
        name: "python",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/python.yaml"),
    },
    GeneratorDef {
        name: "go",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/go.yaml"),
    },
    GeneratorDef {
        name: "csharp",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/csharp.yaml"),
    },
    GeneratorDef {
        name: "typescript-axios",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/typescript-axios.yaml"),
    },
    GeneratorDef {
        name: "typescript-fetch",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/typescript-fetch.yaml"),
    },
    GeneratorDef {
        name: "typescript-node",
        scope: "client",
        config_yaml: include_str!("../assets/generators/client/typescript-node.yaml"),
    },
];

// ── Public API ───────────────────────────────────────────────────────

pub fn builtin_server_generators() -> &'static [GeneratorDef] {
    SERVER_GENERATORS
}

pub fn builtin_client_generators() -> &'static [GeneratorDef] {
    CLIENT_GENERATORS
}

pub fn builtin_generators_for_scope(scope: &str) -> &'static [GeneratorDef] {
    match scope {
        "server" => SERVER_GENERATORS,
        "client" => CLIENT_GENERATORS,
        _ => &[],
    }
}

pub fn find_builtin(name: &str, scope: &str) -> Option<&'static GeneratorDef> {
    builtin_generators_for_scope(scope)
        .iter()
        .find(|g| g.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_generator_count() {
        assert_eq!(builtin_server_generators().len(), 6);
    }

    #[test]
    fn client_generator_count() {
        assert_eq!(builtin_client_generators().len(), 8);
    }

    #[test]
    fn find_builtin_spring() {
        let def = find_builtin("spring", "server").unwrap();
        assert_eq!(def.name, "spring");
        assert!(def.config_yaml.contains("generatorName: spring"));
    }

    #[test]
    fn find_builtin_typescript_axios() {
        let def = find_builtin("typescript-axios", "client").unwrap();
        assert_eq!(def.name, "typescript-axios");
        assert!(def.config_yaml.contains("generatorName: typescript-axios"));
    }

    #[test]
    fn find_builtin_unknown_returns_none() {
        assert!(find_builtin("nonexistent", "server").is_none());
    }

    #[test]
    fn find_builtin_wrong_scope_returns_none() {
        assert!(find_builtin("spring", "client").is_none());
    }

    #[test]
    fn all_configs_contain_output_dir() {
        for def in SERVER_GENERATORS.iter().chain(CLIENT_GENERATORS.iter()) {
            assert!(
                def.config_yaml.contains("outputDir: .oav/generated/"),
                "config for {} missing .oav/generated/ outputDir",
                def.name
            );
        }
    }
}
