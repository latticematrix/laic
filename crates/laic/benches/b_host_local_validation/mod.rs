mod child;
mod measure;
mod process;
mod roundtrip;
mod types;

use std::env;

use child::run_child;
use measure::{
    measure_cross_process_ipc, measure_fanout_ipc, measure_fanout_quic, measure_same_host_quic,
    measure_soak_ipc, measure_soak_quic,
};
use roundtrip::print_row;
use types::{make_error, BenchError, Settings, ValidationCase, ValidationRow};
pub fn main() {
    if let Err(err) = run() {
        eprintln!("Windows local validation failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), BenchError> {
    let args = env::args().collect::<Vec<_>>();
    if let Some(pos) = args.iter().position(|arg| arg == "--laic-child") {
        return run_child(&args[(pos + 1)..]);
    }

    let settings = Settings::from_env()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let rows = runtime.block_on(async {
        let mut rows = Vec::new();
        for case in &settings.cases {
            eprintln!("LAIC_BHOST_CASE_START {}", case.name());
            match case {
                ValidationCase::CrossProcessIpc => {
                    rows.push(measure_cross_process_ipc(&settings).await?);
                }
                ValidationCase::SameHostQuic => {
                    rows.push(measure_same_host_quic(&settings).await?);
                }
                ValidationCase::FanoutIpc => {
                    rows.push(measure_fanout_ipc(&settings).await?);
                }
                ValidationCase::FanoutQuic => {
                    rows.push(measure_fanout_quic(&settings).await?);
                }
                ValidationCase::LocalSoakIpc => {
                    rows.push(measure_soak_ipc(&settings).await?);
                }
                ValidationCase::LocalSoakQuic => {
                    rows.push(measure_soak_quic(&settings).await?);
                }
            }
        }
        Ok::<Vec<ValidationRow>, BenchError>(rows)
    })?;

    println!("LAIC_B_HOST_VALIDATION_START");
    println!(
        "case,slice,test_case,path,metric,expected,observed,setup_us,total_us,p50_us,p95_us,p99_us,messages_per_sec,bytes_per_sec,duration_ms,status,detail"
    );
    for row in &rows {
        print_row(row);
    }
    println!("LAIC_B_HOST_VALIDATION_END");

    if rows.iter().all(|row| row.status == "PASS") {
        Ok(())
    } else {
        Err(make_error(
            "one or more Windows local validation rows failed",
        ))
    }
}
