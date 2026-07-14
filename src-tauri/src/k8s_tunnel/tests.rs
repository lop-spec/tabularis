use super::command::{kubectl_command, run_kubectl};
use super::*;
use crate::models::{ConnectionParams, K8sConnection};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

mod build_tunnel_key_tests {
    use super::*;

    #[test]
    fn test_default_key_format() {
        let options = K8sCommandOptions::default();
        let key = build_tunnel_key("my-cluster", "default", "service", "my-db", 3306, &options);
        let kubeconfig = std::env::var_os("KUBECONFIG")
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<kubectl default>".to_string());
        assert_eq!(
            key,
            format!(
                "my-cluster:default:service/my-db:3306:kubectl=kubectl:kubeconfig={}",
                kubeconfig
            )
        );
    }

    #[test]
    fn test_key_includes_explicit_overrides() {
        let options = K8sCommandOptions::new(
            Some(" custom-kubectl ".to_string()),
            Some(" custom-kubeconfig ".to_string()),
        );
        let key = build_tunnel_key("prod", "database", "pod", "mysql-0", 5432, &options);
        assert_eq!(
            key,
            "prod:database:pod/mysql-0:5432:kubectl=custom-kubectl:kubeconfig=custom-kubeconfig"
        );
    }

    #[test]
    fn test_inherited_kubeconfig_is_part_of_effective_selection() {
        let options = K8sCommandOptions::default();
        assert_eq!(
            options.inherited_kubeconfig_label_for_test(OsString::from("inherited-config")),
            "inherited-config"
        );
    }

    #[test]
    fn test_pod_resource_type() {
        let options = K8sCommandOptions::default();
        let key = build_tunnel_key("prod-cluster", "database", "pod", "mysql-0", 5432, &options);
        assert!(
            key.starts_with("prod-cluster:database:pod/mysql-0:5432:kubectl=kubectl:kubeconfig=")
        );
    }

    #[test]
    fn test_special_characters() {
        let options = K8sCommandOptions::default();
        let key = build_tunnel_key(
            "gke_project_us-central1_cluster",
            "my-namespace",
            "service",
            "my-db-svc",
            3306,
            &options,
        );
        assert!(key.starts_with(
            "gke_project_us-central1_cluster:my-namespace:service/my-db-svc:3306:kubectl=kubectl:kubeconfig="
        ));
    }

    #[test]
    fn test_empty_context() {
        let options = K8sCommandOptions::default();
        let key = build_tunnel_key("", "default", "service", "db", 80, &options);
        assert!(key.starts_with(":default:service/db:80:kubectl=kubectl:kubeconfig="));
    }
}

mod command_options_tests {
    use super::*;

    #[test]
    fn test_empty_and_whitespace_overrides_use_defaults() {
        let options = K8sCommandOptions::new(Some("  ".to_string()), Some(String::new()));
        assert_eq!(options.kubectl_program(), OsStr::new("kubectl"));
        assert_eq!(options.explicit_kubeconfig_path(), None);
    }

    #[test]
    fn test_overrides_are_trimmed() {
        let options = K8sCommandOptions::new(
            Some("  custom-kubectl  ".to_string()),
            Some("  custom-kubeconfig  ".to_string()),
        );
        assert_eq!(options.kubectl_program(), OsStr::new("custom-kubectl"));
        assert_eq!(
            options.explicit_kubeconfig_path(),
            Some(Path::new("custom-kubeconfig"))
        );
    }

    #[test]
    fn test_tilde_paths_are_expanded_from_base_dirs() {
        let base_dirs = directories::BaseDirs::new().expect("home directory should be available");
        let options = K8sCommandOptions::new(
            Some("~/bin/kubectl".to_string()),
            Some("~/.kube/config".to_string()),
        );
        assert_eq!(
            Path::new(options.kubectl_program()),
            base_dirs.home_dir().join("bin/kubectl")
        );
        assert_eq!(
            options.explicit_kubeconfig_path(),
            Some(base_dirs.home_dir().join(".kube/config").as_path())
        );

        let home = K8sCommandOptions::new(None, Some("~".to_string()));
        assert_eq!(home.explicit_kubeconfig_path(), Some(base_dirs.home_dir()));
    }
}

mod command_construction_tests {
    use super::*;

    #[test]
    fn test_default_command_uses_kubectl_without_overriding_environment() {
        let command = kubectl_command(&K8sCommandOptions::default());
        assert_eq!(command.get_program(), OsStr::new("kubectl"));
        assert!(command
            .get_envs()
            .all(|(name, _)| name != OsStr::new("KUBECONFIG")));
    }

    #[test]
    fn test_explicit_command_sets_program_and_kubeconfig_environment() {
        let options = K8sCommandOptions::new(
            Some("custom-kubectl".to_string()),
            Some("custom-kubeconfig".to_string()),
        );
        let command = kubectl_command(&options);
        assert_eq!(command.get_program(), OsStr::new("custom-kubectl"));
        let kubeconfig = command
            .get_envs()
            .find(|(name, _)| *name == OsStr::new("KUBECONFIG"))
            .and_then(|(_, value)| value);
        assert_eq!(kubeconfig, Some(OsStr::new("custom-kubeconfig")));
    }

    #[test]
    fn test_spawn_error_identifies_configured_program() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing-kubectl");
        let options = K8sCommandOptions::new(Some(missing.display().to_string()), None);
        let error = run_kubectl(&["version", "--client"], &options, "version failed")
            .expect_err("missing executable should fail to spawn");
        assert!(error.contains("Failed to launch configured kubectl executable"));
        assert!(error.contains(&missing.display().to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_successful_empty_context_output_returns_empty_list() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let kubectl = dir.path().join("kubectl");
        std::fs::write(&kubectl, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&kubectl).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&kubectl, permissions).unwrap();

        let options = K8sCommandOptions::new(Some(kubectl.display().to_string()), None);
        let contexts = get_k8s_contexts(&options).expect("empty output is still successful");
        assert!(contexts.is_empty());
    }
}

mod path_validation_tests {
    use super::*;

    fn write_temp_file(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, "test").unwrap();
        path
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(windows)]
    fn make_executable(_path: &Path) {}

    #[test]
    fn test_empty_paths_are_valid_defaults() {
        assert!(validate_k8s_path("", "kubectl").is_ok());
        assert!(validate_k8s_path("   ", "kubeconfig").is_ok());
    }

    #[test]
    fn test_missing_paths_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");

        let kubectl_error = validate_k8s_path(missing.to_str().unwrap(), "kubectl").unwrap_err();
        assert!(kubectl_error.contains("kubectl executable was not found"));

        let kubeconfig_error =
            validate_k8s_path(missing.to_str().unwrap(), "kubeconfig").unwrap_err();
        assert!(kubeconfig_error.contains("Kubeconfig file was not found"));
    }

    #[test]
    fn test_directories_are_rejected() {
        let dir = tempfile::tempdir().unwrap();

        let kubectl_error = validate_k8s_path(dir.path().to_str().unwrap(), "kubectl").unwrap_err();
        assert!(kubectl_error.contains("must point to an executable file"));

        let kubeconfig_error =
            validate_k8s_path(dir.path().to_str().unwrap(), "kubeconfig").unwrap_err();
        assert!(kubeconfig_error.contains("must point to a file"));
    }

    #[test]
    fn test_kubeconfig_file_is_valid_after_trimming() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "config");
        let padded = format!("  {}  ", path.display());
        assert!(validate_k8s_path(&padded, "kubeconfig").is_ok());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn test_executable_kubectl_file_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let file_name = if cfg!(windows) {
            "kubectl.exe"
        } else {
            "kubectl"
        };
        let path = write_temp_file(&dir, file_name);
        make_executable(&path);
        assert!(validate_k8s_path(path.to_str().unwrap(), "kubectl").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_non_executable_kubectl_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "kubectl");
        let error = validate_k8s_path(path.to_str().unwrap(), "kubectl").unwrap_err();
        assert!(error.contains("kubectl path is not executable"));
    }

    #[cfg(windows)]
    #[test]
    fn test_non_executable_kubectl_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "kubectl.txt");
        let error = validate_k8s_path(path.to_str().unwrap(), "kubectl").unwrap_err();
        assert!(error.contains("kubectl path is not executable"));
    }

    #[test]
    fn test_bare_executable_name_resolves_from_path_when_available() {
        #[cfg(windows)]
        let candidates = ["cmd", "where"];
        #[cfg(not(windows))]
        let candidates = ["sh", "env"];

        let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| which::which(candidate).is_ok())
        else {
            eprintln!("Skipping PATH resolution assertion: no stable executable candidate found");
            return;
        };

        assert!(validate_k8s_path(candidate, "kubectl").is_ok());
    }

    #[test]
    fn test_unsupported_validation_kind_is_rejected() {
        let error = validate_k8s_path("anything", "certificate").unwrap_err();
        assert!(error.contains("Unsupported Kubernetes path validation kind 'certificate'"));
        assert!(error.contains("'kubectl' or 'kubeconfig'"));
    }
}

mod persisted_model_compatibility_tests {
    use super::*;

    #[test]
    fn test_old_connection_params_json_defaults_override_fields() {
        let params: ConnectionParams =
            serde_json::from_str(r#"{"driver":"mysql","database":"example"}"#).unwrap();
        assert_eq!(params.k8s_kubectl_path, None);
        assert_eq!(params.k8s_kubeconfig_path, None);

        let serialized = serde_json::to_value(params).unwrap();
        assert!(serialized.get("k8s_kubectl_path").is_none());
        assert!(serialized.get("k8s_kubeconfig_path").is_none());
    }

    #[test]
    fn test_old_saved_k8s_json_defaults_override_fields() {
        let connection: K8sConnection = serde_json::from_str(
            r#"{
                "id":"saved-id",
                "name":"Saved cluster",
                "context":"context",
                "namespace":"default",
                "resource_type":"service",
                "resource_name":"database",
                "port":5432
            }"#,
        )
        .unwrap();
        assert_eq!(connection.kubectl_path, None);
        assert_eq!(connection.kubeconfig_path, None);

        let serialized = serde_json::to_value(connection).unwrap();
        assert!(serialized.get("kubectl_path").is_none());
        assert!(serialized.get("kubeconfig_path").is_none());
    }
}

mod parse_lines_tests {
    use super::*;

    #[test]
    fn test_basic_lines() {
        let output = "line1\nline2\nline3\n";
        let result = parse_lines(output);
        assert_eq!(result, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_empty_output() {
        let result = parse_lines("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_whitespace_handling() {
        let output = "  line1  \n\n  line2  \n";
        let result = parse_lines(output);
        assert_eq!(result, vec!["line1", "line2"]);
    }
}

mod parse_lines_with_prefix_tests {
    use super::*;

    #[test]
    fn test_namespace_prefix() {
        let output = "namespace/default\nnamespace/kube-system\nnamespace/my-ns\n";
        let result = parse_lines_with_prefix(output, "namespace/");
        assert_eq!(result, vec!["default", "kube-system", "my-ns"]);
    }

    #[test]
    fn test_service_prefix() {
        let output = "service/my-db\nservice/api-gateway\n";
        let result = parse_lines_with_prefix(output, "service/");
        assert_eq!(result, vec!["my-db", "api-gateway"]);
    }

    #[test]
    fn test_pod_prefix() {
        let output = "pod/mysql-0\npod/mysql-1\n";
        let result = parse_lines_with_prefix(output, "pod/");
        assert_eq!(result, vec!["mysql-0", "mysql-1"]);
    }

    #[test]
    fn test_no_match_returns_full_line() {
        let output = "something/else\n";
        let result = parse_lines_with_prefix(output, "namespace/");
        assert_eq!(result, vec!["something/else"]);
    }

    #[test]
    fn test_empty_output() {
        let result = parse_lines_with_prefix("", "namespace/");
        assert!(result.is_empty());
    }
}

mod parse_resource_ports_tests {
    use super::*;

    #[test]
    fn test_single_port() {
        let result = parse_resource_ports("5432");
        assert_eq!(result, vec![5432]);
    }

    #[test]
    fn test_multiple_ports() {
        let result = parse_resource_ports("80 443 5432");
        assert_eq!(result, vec![80, 443, 5432]);
    }

    #[test]
    fn test_ignores_invalid_values() {
        let result = parse_resource_ports("abc 3306 70000 8123");
        assert_eq!(result, vec![3306, 8123]);
    }

    #[test]
    fn test_empty_output() {
        let result = parse_resource_ports("");
        assert!(result.is_empty());
    }
}
