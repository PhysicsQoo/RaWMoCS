//! Research-only repeated library probe. No production allocator mutation.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use rawmocs_mpfr::{solve_file, Precision, SolveOptions};
    use std::io::{self, BufRead, Write};
    use std::time::Instant;

    #[repr(C)]
    struct MallInfo2 {
        arena: usize,
        ordblks: usize,
        smblks: usize,
        hblks: usize,
        hblkhd: usize,
        usmblks: usize,
        fsmblks: usize,
        uordblks: usize,
        fordblks: usize,
        keepcost: usize,
    }
    extern "C" {
        fn mallinfo2() -> MallInfo2;
    }

    let path = std::env::args_os().nth(1).ok_or("missing CNF path")?;
    let options = SolveOptions {
        precision: Precision::from_digits(130).ok_or("precision")?,
        timeout_ms: Some(120_000),
        ..SolveOptions::default()
    };
    for line in io::stdin().lock().lines() {
        if line? != "solve" {
            return Err("unknown request".into());
        }
        let start = Instant::now();
        let result = solve_file(&path, options)?;
        let real = result.real_mpfr().binary();
        let imag = result.imag_mpfr().binary();
        let fields = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            result.sat,
            result.real,
            result.imag,
            result.route.as_str(),
            result.precision.bits(),
            real.significand,
            real.exponent2,
            real.negative_zero,
            imag.significand,
            imag.exponent2,
            imag.negative_zero
        );
        drop((result, real, imag));
        let elapsed = start.elapsed().as_nanos();
        // SAFETY: glibc mallinfo2 returns a repr(C) value and takes no arguments.
        let m = unsafe { mallinfo2() };
        println!(
            "M0_ROW\t{fields}\t{elapsed}\t{}\t{}\t{}\t{}",
            m.uordblks, m.hblkhd, m.fordblks, m.arena
        );
        io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn main() {
    eprintln!("M0 mallinfo2 probe is Linux glibc only");
}
