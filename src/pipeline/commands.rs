use std::path::Path;
use std::time::Duration;

use crate::config::Config;
use crate::docker::{self, ContainerCommand};

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
pub fn generator_command(
    cfg: &Config,
    spec_path: &Path,
    work_dir: &Path,
    generator: &str,
    scope: &str,
) -> ContainerCommand {
    let spec_name = spec_path.file_name().unwrap_or_default().to_string_lossy();
    let output_dir = format!("/work/.generated/{generator}-{scope}");

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
    let source_dir = format!("/work/.generated/{generator}-{scope}");

    let mut args = vec![
        "run".into(),
        "--rm".into(),
        "-v".into(),
        format!("{}:/work", work_dir.display()),
    ];
    args.extend(docker::user_args());

    // Use generator override image if configured, otherwise use the generator image.
    let image = cfg
        .generator_overrides
        .get(generator)
        .cloned()
        .unwrap_or_else(|| cfg.generator_image.clone());

    args.extend([image, "batch".into(), "--includes".into(), source_dir]);

    ContainerCommand {
        args,
        timeout: Duration::from_secs(cfg.docker_timeout),
        log_path: None,
    }
}

/// Resolve the full list of `(generator, scope)` pairs from config.
pub fn build_generator_list(cfg: &Config) -> Vec<(String, String)> {
    let mut pairs = Vec::new();

    let add_for_scope = |pairs: &mut Vec<(String, String)>, generators: &[String], scope: &str| {
        for generator in generators {
            pairs.push((generator.clone(), scope.to_string()));
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
        );
        assert!(cmd.args.contains(&cfg.generator_image));
        assert!(cmd.args.contains(&"generate".into()));
        assert!(cmd.args.contains(&"-g".into()));
        assert!(cmd.args.contains(&"spring".into()));
        assert!(cmd.args.contains(&"/work/.generated/spring-server".into()));
    }

    #[test]
    fn compile_command_builds_correct_args() {
        let cfg = test_config();
        let cmd = compile_command(&cfg, Path::new("/tmp"), "spring", "server");
        assert!(cmd.args.contains(&"batch".into()));
        assert!(cmd.args.contains(&"/work/.generated/spring-server".into()));
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
    fn build_generator_list_empty_generators() {
        let mut cfg = test_config();
        cfg.server_generators.clear();
        cfg.client_generators.clear();
        let pairs = build_generator_list(&cfg);
        assert!(pairs.is_empty());
    }

    #[test]
    fn compile_command_uses_override_image() {
        let mut cfg = test_config();
        cfg.generator_overrides
            .insert("spring".into(), "custom/spring:latest".into());
        let cmd = compile_command(&cfg, Path::new("/tmp"), "spring", "server");
        assert!(cmd.args.contains(&"custom/spring:latest".into()));
    }

    #[test]
    fn spectral_command_timeout_from_config() {
        let mut cfg = test_config();
        cfg.docker_timeout = 60;
        let cmd = spectral_command(&cfg, Path::new("/tmp/spec.yaml"), Path::new("/tmp"));
        assert_eq!(cmd.timeout, Duration::from_secs(60));
    }
}
