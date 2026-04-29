use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

const BAR_CHARS: &str = "█▓▒░ ";

pub fn worker_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{prefix:.bold.cyan} [{elapsed_precise}] {bar:40.cyan/blue} {percent:>3}%  {bytes:>10}/{total_bytes:<10}  {bytes_per_sec:>12}  ETA {eta:>5}  {msg}",
    )
    .expect("valid worker progress style")
    .progress_chars(BAR_CHARS)
}

pub fn overall_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{prefix:.bold.green} [{elapsed_precise}] {bar:40.green/black} {pos:>3}/{len:<3}  {msg}",
    )
    .expect("valid overall progress style")
    .progress_chars(BAR_CHARS)
}

pub fn install_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{prefix:.bold.magenta} [{elapsed_precise}] {bar:40.magenta/black} {percent:>3}%  {bytes:>10}/{total_bytes:<10}  {bytes_per_sec:>12}  ETA {eta:>5}",
    )
    .expect("valid install progress style")
    .progress_chars(BAR_CHARS)
}

pub fn make_worker_bar(mp: &MultiProgress, slot: usize) -> ProgressBar {
    let pb = mp.add(ProgressBar::new(0));
    pb.set_style(worker_style());
    pb.set_prefix(format!("[{slot}]"));
    pb.set_message("waiting…");
    pb
}

pub fn make_overall_bar(mp: &MultiProgress, total: u64) -> ProgressBar {
    let pb = mp.add(ProgressBar::new(total));
    pb.set_style(overall_style());
    pb.set_prefix("[all]".to_string());
    pb
}

pub fn make_install_bar(mp: &MultiProgress, label: &str) -> ProgressBar {
    let pb = mp.add(ProgressBar::new(0));
    pb.set_style(install_style());
    pb.set_prefix(format!("[{label}]"));
    pb
}
