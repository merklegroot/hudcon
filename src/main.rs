use sysinfo::{CpuRefreshKind, RefreshKind, System};

fn main() {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();

    println!("Processor");
    println!("---------");

    if let Some(cpu) = sys.cpus().first() {
        println!("Brand:    {}", cpu.brand().trim());
        println!("Frequency: {} MHz", cpu.frequency());
    } else {
        println!("Brand:    (unavailable)");
    }

    if let Some(n) = sys.physical_core_count() {
        println!("Physical cores: {}", n);
    }
    println!("Logical cores:  {}", sys.cpus().len());
}
