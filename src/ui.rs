use crate::{
    constant::{G_BITS_PER_SEC, GB, K_BITS_PER_SEC, KB, M_BITS_PER_SEC, MB, T_BITS_PER_SEC, TB},
    message::StreamStats,
    opts::{ClientOpts, CommonOpts},
};

pub fn print_header() {
    println!(
        "[ ID]     Index   Duration        Transfer            Bitrate     Retr            Cwnd"
    );
}

pub fn print_client_banner(common_opt: &CommonOpts, client_opts: &ClientOpts) {
    println!("-------------------------------------------------");
    println!(
        "Client connecting to {}:{} {}",
        client_opts.client.clone().unwrap(),
        common_opt.port,
        common_opt
            .bind
            .clone()
            .map_or_else(String::default, |bind| format!("(bind {bind})"))
    );
    println!("-------------------------------------------------");
}

pub fn print_server_banner(common_opt: &CommonOpts) {
    println!("-------------------------------------------------");
    println!(
        "Server listening on port {} {}",
        common_opt.port,
        common_opt
            .bind
            .clone()
            .map_or_else(String::default, |bind| format!("(bind {bind})"))
    );
    println!("-------------------------------------------------");
}

pub fn print_accept(peer_addr: String) {
    println!("*************************************************");
    println!("Accepted connection from {peer_addr}");
    println!("*************************************************");
}

pub fn print_stats(stats: &StreamStats) {
    if stats.is_summary {
        print_summary_stats(stats);
    } else {
        print_normal_stats(stats);
    }
}

fn print_normal_stats(stats: &StreamStats) {
    println!(
        "[{:>3}]     {:>5} {:>10} {:>15} {:>18} {:>8} {:>15} {}",
        stats.id,
        stats.index.unwrap_or(0),
        humanize_duration(stats.duration),
        humanize_bytes(stats.bytes_transferred),
        humanize_bitrate(stats.bytes_transferred, stats.duration),
        stats
            .retransmits
            .map_or("None".to_string(), |v| v.to_string()),
        stats.cwnd.map_or("None".to_string(), humanize_bytes),
        if stats.is_sending {
            "Sender"
        } else {
            "Receiver"
        }
    );
}

fn print_summary_stats(stats: &StreamStats) {
    println!(
        "- - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -"
    );

    println!(
        "[{:>3}]     {:>5} {:>10} {:>15} {:>18} {:>8} {:>15} {}",
        stats.id,
        "TOTAL",
        humanize_duration(stats.duration),
        humanize_bytes(stats.bytes_transferred),
        humanize_bitrate(stats.bytes_transferred, stats.duration),
        stats
            .retransmits
            .map_or("None".to_string(), |v| v.to_string()),
        stats.cwnd.map_or("None".to_string(), humanize_bytes),
        if stats.is_sending {
            "Sender"
        } else {
            "Receiver"
        }
    );
}

fn humanize_bytes(bytes: usize) -> String {
    if bytes < KB {
        format!("{bytes} Bytes")
    } else if bytes < MB {
        format!("{:.2} KBytes", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MBytes", bytes as f64 / MB as f64)
    } else if bytes < TB {
        format!("{:.2} GBytes", bytes as f64 / GB as f64)
    } else {
        format!("{:.2} TBytes", bytes as f64 / TB as f64)
    }
}

fn humanize_bitrate(bytes: usize, duration_millis: u64) -> String {
    // For higher accuracy we are getting the actual millis of the duration rather than the
    // rounded seconds.
    let bits = bytes * 8;
    // rate as fraction in seconds;
    let rate = (bits as f64 / duration_millis as f64) * 1000f64;
    if rate < K_BITS_PER_SEC as f64 {
        format!("{rate} Bits/sec")
    } else if bytes < M_BITS_PER_SEC {
        format!("{:.2} Kbits/sec", rate / K_BITS_PER_SEC as f64)
    } else if bytes < G_BITS_PER_SEC {
        format!("{:.2} Mbits/sec", rate / M_BITS_PER_SEC as f64)
    } else if bytes < T_BITS_PER_SEC {
        format!("{:.2} Gbits/sec", rate / G_BITS_PER_SEC as f64)
    } else {
        format!("{:.2} Tbits/sec", rate / T_BITS_PER_SEC as f64)
    }
}

fn humanize_duration(duration_millis: u64) -> String {
    format!("{:.2} sec", (duration_millis as f64 / 1000f64))
}
