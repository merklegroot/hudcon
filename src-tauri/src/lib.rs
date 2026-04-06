use hudcon::cpu;
use hudcon::disk;
use hudcon::gpu;
use hudcon::machine;
use hudcon::memory;
use hudcon::package;
use hudcon::path;

#[tauri::command]
fn get_cpu_info() -> cpu::CpuSnapshot {
    cpu::gather_cpu_info()
}

#[tauri::command]
fn get_machine_info() -> machine::MachineInfo {
    machine::gather_machine_info()
}

#[tauri::command]
fn get_gpu_info() -> gpu::GpuInfoDto {
    gpu::gather_gpu_info_dto()
}

#[tauri::command]
fn get_memory_info() -> memory::MemoryInfo {
    memory::gather_memory_info()
}

#[tauri::command]
fn get_disk_info() -> disk::DiskGatherResult {
    disk::gather_disk_info()
}

#[tauri::command]
fn get_package_info() -> package::PackageInfo {
    package::gather_package_info()
}

#[tauri::command]
fn get_path_info() -> path::PathInfo {
    path::gather_path_info()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_cpu_info,
            get_machine_info,
            get_gpu_info,
            get_memory_info,
            get_disk_info,
            get_package_info,
            get_path_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
