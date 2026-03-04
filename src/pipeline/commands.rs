use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::docker::{self, ContainerCommand};
use crate::generators;

/// Build a `docker run` command for Spectral linting.
pub fn spectral_command(cfg: &Config, spec_path: &Path, work_dir: &Path) -> ContainerCommand {
    let spec_name = spec_path.file_name().unwrap_or_default().to_string_lossy();

    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/work", work_dir.display()),
    ];
    args.extend(docker::user_args());
    args.extend([
        cfg.spectral_image.clone(),
        "lint".into(),
        format!("/work/{spec_name}"),
        "--ruleset".into(),
        cfg.spectral_ruleset.clone(),
        "--fail-severity".into(),
        cfg.spectral_fail_severity.clone(),
        "-f".into(),
        "stylish".into(),
    ]);

    ContainerCommand {
        args,
        timeout: Duration::from_secs(cfg.docker_timeout),
        log_path: None,
    }
}

/// Build a `docker run` command for Redocly linting.
pub fn redocly_command(cfg: &Config, spec_path: &Path, work_dir: &Path) -> ContainerCommand {
    let spec_name = spec_path.file_name().unwrap_or_default().to_string_lossy();

    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/work", work_dir.display()),
    ];
    args.extend(docker::user_args());
    args.extend([
        cfg.redocly_image.clone(),
        "lint".into(),
        format!("/work/{spec_name}"),
        "--format".into(),
        "stylish".into(),
    ]);

    ContainerCommand {
        args,
        timeout: Duration::from_secs(cfg.docker_timeout),
        log_path: None,
    }
}

/// Build a `docker run` command for code generation.
///
/// If a config file path is provided (from builtin registry or user override),
/// it is passed via `-c` to the generator CLI.
pub fn generator_command(
    cfg: &Config,
    spec_path: &Path,
    work_dir: &Path,
    generator: &str,
    scope: &str,
    config_path: Option<&str>,
) -> ContainerCommand {
    let spec_name = spec_path.file_name().unwrap_or_default().to_string_lossy();
    let output_dir = format!("/work/.oav/generated/{scope}/{generator}");

    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/work", work_dir.display()),
    ];
    args.extend(docker::user_args());
    args.extend([
        cfg.generator_image.clone(),
        "generate".into(),
        "-i".into(),
        format!("/work/{spec_name}"),
        "-g".into(),
        generator.to_string(),
        "-o".into(),
        output_dir,
    ]);

    if let Some(path) = config_path {
        args.extend(["-c".into(), path.to_string()]);
    }

    ContainerCommand {
        args,
        timeout: Duration::from_secs(cfg.docker_timeout),
        log_path: None,
    }
}

/// Build a `docker run` command for compiling generated code.
pub fn compile_command(
    cfg: &Config,
    work_dir: &Path,
    generator: &str,
    scope: &str,
) -> ContainerCommand {
    let source_dir = format!("/work/.oav/generated/{scope}/{generator}");

    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/work", work_dir.display()),
    ];
    args.extend(docker::user_args());
    args.extend([
        cfg.generator_image.clone(),
        "batch".into(),
        "--includes".into(),
        source_dir,
    ]);

    ContainerCommand {
        args,
        timeout: Duration::from_secs(cfg.docker_timeout),
        log_path: None,
    }
}

/// Resolve the config file path for a generator.
///
/// Resolution order:
/// 1. User override in `generator_config_overrides` → use that path directly
/// 2. Built-in registry match → `/work/.oav/configs/{scope}/{generator}.yaml`
/// 3. No match → `None` (bare `-g` only)
pub fn resolve_config_path(cfg: &Config, generator: &str, scope: &str) -> Option<String> {
    if let Some(user_path) = cfg.generator_config_overrides.get(generator) {
        return Some(user_path.clone());
    }
    if generators::find_builtin(generator, scope).is_some() {
        return Some(format!("/work/.oav/configs/{scope}/{generator}.yaml"));
    }
    None
}

/// Resolve the full list of `(generator, scope)` pairs from config.
///
/// When generator lists are empty and the mode includes that scope,
/// defaults to all builtin generators for that scope.
pub fn build_generator_list(cfg: &Config) -> Vec<(String, String)> {
    let mut pairs = Vec::new();

    let add_for_scope = |pairs: &mut Vec<(String, String)>, generators: &[String], scope: &str| {
        if generators.is_empty() {
            for def in generators::builtin_generators_for_scope(scope) {
                pairs.push((def.name.to_string(), scope.to_string()));
            }
        } else {
            for generator in generators {
                pairs.push((generator.clone(), scope.to_string()));
            }
        }
    };

    match cfg.mode {
        crate::config::Mode::Server => {
            add_for_scope(&mut pairs, &cfg.server_generators, "server");
        }
        crate::config::Mode::Client => {
            add_for_scope(&mut pairs, &cfg.client_generators, "client");
        }
        crate::config::Mode::Both => {
            add_for_scope(&mut pairs, &cfg.server_generators, "server");
            add_for_scope(&mut pairs, &cfg.client_generators, "client");
        }
    }

    pairs
}

/// Write builtin config files to `.oav/configs/{scope}/` on the host filesystem.
///
/// Called before the generate phase so Docker containers can mount them.
/// Only writes configs for generators that don't have a user override.
pub fn write_builtin_configs(cfg: &Config, work_dir: &Path, generators: &[(String, String)]) {
    for (name, scope) in generators {
        if cfg.generator_config_overrides.contains_key(name.as_str()) {
            continue;
        }
        if let Some(def) = crate::generators::find_builtin(name, scope) {
            let config_dir = work_dir.join(format!(".oav/configs/{scope}"));
            if std::fs::create_dir_all(&config_dir).is_ok() {
                let config_path = config_dir.join(format!("{name}.yaml"));
                let _ = std::fs::write(&config_path, def.config_yaml);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mode};

    fn test_config() -> Config {
        Config {
            spectral_image: "stoplight/spectral:6".into(),
            spectral_ruleset: "https://example.com/.spectral.yml".into(),
            spectral_fail_severity: "error".into(),
            redocly_image: "redocly/cli:1.25.5".into(),
            generator_image: "openapitools/openapi-generator-cli:v7.17.0".into(),
            docker_timeout: 120,
            mode: Mode::Both,
            server_generators: vec!["spring".into()],
            client_generators: vec!["typescript-axios".into()],
            ..Config::default()
        }
    }

    #[test]
    fn spectral_command_builds_correct_args() {
        let cfg = test_config();
        let cmd = spectral_command(&cfg, Path::new("/tmp/spec.yaml"), Path::new("/tmp"));
        assert!(cmd.args.contains(&"run".into()));
        assert!(cmd.args.contains(&"--rm".into()));
        assert!(cmd.args.contains(&cfg.spectral_image));
        assert!(cmd.args.contains(&"lint".into()));
        assert!(cmd.args.contains(&"/work/spec.yaml".into()));
        assert!(cmd.args.contains(&"--ruleset".into()));
        assert!(cmd.args.contains(&cfg.spectral_ruleset));
        assert!(cmd.args.contains(&"stylish".into()));
    }

    #[test]
    fn redocly_command_builds_correct_args() {
        let cfg = test_config();
        let cmd = redocly_command(&cfg, Path::new("/tmp/spec.yaml"), Path::new("/tmp"));
        assert!(cmd.args.contains(&cfg.redocly_image));
        assert!(cmd.args.contains(&"lint".into()));
        assert!(cmd.args.contains(&"/work/spec.yaml".into()));
    }

    #[test]
    fn generator_command_builds_correct_args() {
        let cfg = test_config();
        let cmd = generator_command(
            &cfg,
            Path::new("/tmp/spec.yaml"),
            Path::new("/tmp"),
            "spring",
            "server",
            None,
        );
        assert!(cmd.args.contains(&cfg.generator_image));
        assert!(cmd.args.contains(&"generate".into()));
        assert!(cmd.args.contains(&"-g".into()));
        assert!(cmd.args.contains(&"spring".into()));
        assert!(
            cmd.args
                .contains(&"/work/.oav/generated/server/spring".into())
        );
        assert!(!cmd.args.contains(&"-c".into()));
    }

    #[test]
    fn generator_command_with_config() {
        let cfg = test_config();
        let cmd = generator_command(
            &cfg,
            Path::new("/tmp/spec.yaml"),
            Path::new("/tmp"),
            "spring",
            "server",
            Some("/work/.oav/configs/server/spring.yaml"),
        );
        assert!(cmd.args.contains(&"-c".into()));
        assert!(
            cmd.args
                .contains(&"/work/.oav/configs/server/spring.yaml".into())
        );
    }

    #[test]
    fn compile_command_builds_correct_args() {
        let cfg = test_config();
        let cmd = compile_command(&cfg, Path::new("/tmp"), "spring", "server");
        assert!(cmd.args.contains(&"batch".into()));
        assert!(
            cmd.args
                .contains(&"/work/.oav/generated/server/spring".into())
        );
        assert!(cmd.args.contains(&cfg.generator_image));
    }

    #[test]
    fn build_generator_list_both_mode() {
        let cfg = test_config();
        let pairs = build_generator_list(&cfg);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("spring".into(), "server".into()));
        assert_eq!(pairs[1], ("typescript-axios".into(), "client".into()));
    }

    #[test]
    fn build_generator_list_server_only() {
        let mut cfg = test_config();
        cfg.mode = Mode::Server;
        let pairs = build_generator_list(&cfg);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1, "server");
    }

    #[test]
    fn build_generator_list_empty_defaults_to_builtins() {
        let mut cfg = test_config();
        cfg.server_generators.clear();
        cfg.client_generators.clear();
        let pairs = build_generator_list(&cfg);
        assert_eq!(pairs.len(), 14); // 6 server + 8 client
    }

    #[test]
    fn build_generator_list_empty_server_only_defaults() {
        let mut cfg = test_config();
        cfg.mode = Mode::Server;
        cfg.server_generators.clear();
        let pairs = build_generator_list(&cfg);
        assert_eq!(pairs.len(), 6);
        assert!(pairs.iter().all(|(_, s)| s == "server"));
    }

    #[test]
    fn resolve_config_path_builtin() {
        let cfg = test_config();
        let path = resolve_config_path(&cfg, "spring", "server");
        assert_eq!(
            path.as_deref(),
            Some("/work/.oav/configs/server/spring.yaml")
        );
    }

    #[test]
    fn resolve_config_path_user_override() {
        let mut cfg = test_config();
        cfg.generator_config_overrides
            .insert("spring".into(), "/work/custom/spring.yaml".into());
        let path = resolve_config_path(&cfg, "spring", "server");
        assert_eq!(path.as_deref(), Some("/work/custom/spring.yaml"));
    }

    #[test]
    fn resolve_config_path_unknown_generator() {
        let cfg = test_config();
        let path = resolve_config_path(&cfg, "unknown-gen", "server");
        assert!(path.is_none());
    }

    #[test]
    fn write_builtin_configs_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = test_config();
        let generators = vec![("spring".into(), "server".into())];
        write_builtin_configs(&cfg, tmp.path(), &generators);

        let config_path = tmp.path().join(".oav/configs/server/spring.yaml");
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("generatorName: spring"));
    }

    #[test]
    fn write_builtin_configs_skips_overridden() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = test_config();
        cfg.generator_config_overrides
            .insert("spring".into(), "/work/custom.yaml".into());
        let generators = vec![("spring".into(), "server".into())];
        write_builtin_configs(&cfg, tmp.path(), &generators);

        let config_path = tmp.path().join(".oav/configs/server/spring.yaml");
        assert!(!config_path.exists());
    }

    #[test]
    fn spectral_command_timeout_from_config() {
        let mut cfg = test_config();
        cfg.docker_timeout = 60;
        let cmd = spectral_command(&cfg, Path::new("/tmp/spec.yaml"), Path::new("/tmp"));
        assert_eq!(cmd.timeout, Duration::from_secs(60));
    }
}
