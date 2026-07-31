use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;
use sysinfo::{get_current_pid, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

static SYSTEM: Lazy<Mutex<System>> = Lazy::new(|| {
    Mutex::new(System::new_with_specifics(
        RefreshKind::new().with_processes(
            ProcessRefreshKind::new()
                .with_cpu()
                .with_disk_usage()
                .with_memory(),
        ),
    ))
});

#[derive(Debug, Serialize, Clone)]
pub struct TabularisChildProcess {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct TabularisSelfStats {
    pub pid: u32,
    pub self_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub cpu_percent: f32,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub child_count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub process_count: usize,
    pub tabularis: Option<TabularisSelfStats>,
}

fn descendants_of(sys: &System, root: Pid) -> HashSet<Pid> {
    let mut descendants = HashSet::new();
    let mut queue = vec![root];
    while let Some(current) = queue.pop() {
        for (pid, process) in sys.processes() {
            if process.parent() == Some(current) && descendants.insert(*pid) {
                queue.push(*pid);
            }
        }
    }
    descendants
}

fn refresh_and_collect_system_stats() -> SystemStats {
    let mut sys = SYSTEM.lock().expect("system mutex poisoned");
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new()
            .with_cpu()
            .with_disk_usage()
            .with_memory(),
    );
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let (disk_read_bytes, disk_write_bytes) =
        sys.processes()
            .values()
            .fold((0_u64, 0_u64), |(read, write), process| {
                let usage = process.disk_usage();
                (
                    read.saturating_add(usage.read_bytes),
                    write.saturating_add(usage.written_bytes),
                )
            });

    let tabularis = get_current_pid().ok().map(|self_pid| {
        let descendants = descendants_of(&sys, self_pid);
        let self_process = sys.process(self_pid);
        let self_memory_bytes = self_process.map(|p| p.memory()).unwrap_or_default();
        let mut total_memory_bytes = self_memory_bytes;
        let mut cpu_percent = self_process.map(|p| p.cpu_usage()).unwrap_or_default();
        let mut process_read_bytes = self_process
            .map(|p| p.disk_usage().read_bytes)
            .unwrap_or_default();
        let mut process_write_bytes = self_process
            .map(|p| p.disk_usage().written_bytes)
            .unwrap_or_default();

        for pid in &descendants {
            if let Some(process) = sys.process(*pid) {
                let usage = process.disk_usage();
                cpu_percent += process.cpu_usage();
                total_memory_bytes = total_memory_bytes.saturating_add(process.memory());
                process_read_bytes = process_read_bytes.saturating_add(usage.read_bytes);
                process_write_bytes = process_write_bytes.saturating_add(usage.written_bytes);
            }
        }

        TabularisSelfStats {
            pid: self_pid.as_u32(),
            self_memory_bytes,
            total_memory_bytes,
            cpu_percent,
            disk_read_bytes: process_read_bytes,
            disk_write_bytes: process_write_bytes,
            child_count: descendants.len(),
        }
    });

    SystemStats {
        cpu_percent: sys.global_cpu_usage(),
        memory_used: sys.used_memory(),
        memory_total: sys.total_memory(),
        disk_read_bytes,
        disk_write_bytes,
        process_count: sys.processes().len(),
        tabularis,
    }
}

fn collect_tabularis_children() -> Vec<TabularisChildProcess> {
    let mut sys = SYSTEM.lock().expect("system mutex poisoned");
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cpu().with_memory(),
    );

    let Ok(self_pid) = get_current_pid() else {
        return Vec::new();
    };
    let mut children: Vec<_> = descendants_of(&sys, self_pid)
        .into_iter()
        .filter_map(|pid| {
            sys.process(pid).map(|process| TabularisChildProcess {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
            })
        })
        .collect();
    children.sort_by_key(|child| child.pid);
    children
}

#[tauri::command]
pub async fn get_system_stats() -> Result<SystemStats, String> {
    tokio::task::spawn_blocking(refresh_and_collect_system_stats)
        .await
        .map_err(|error| format!("Failed to collect system stats: {error}"))
}

#[tauri::command]
pub async fn get_tabularis_children() -> Result<Vec<TabularisChildProcess>, String> {
    tokio::task::spawn_blocking(collect_tabularis_children)
        .await
        .map_err(|error| format!("Failed to collect Tabularis child processes: {error}"))
}

#[tauri::command]
pub async fn open_task_manager_window(app: AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("task-manager") {
        existing
            .set_focus()
            .map_err(|error| format!("Failed to focus task manager window: {error}"))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "task-manager",
        WebviewUrl::App("/task-manager".into()),
    )
    .title("Tabularis - Task Manager")
    .inner_size(900.0, 600.0)
    .min_inner_size(700.0, 450.0)
    .center()
    .build()
    .map_err(|error| format!("Failed to create task manager window: {error}"))?;

    Ok(())
}
