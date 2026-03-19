#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn tui_subcommand_exits_cleanly_on_quit_command() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open PTY pair");

    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_redshank"));
    cmd.arg("tui");
    cmd.env("RUST_LOG", "error");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn redshank tui");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("clone PTY reader");
    let mut writer = pair.master.take_writer().expect("take PTY writer");
    let output = Arc::new(Mutex::new(Vec::new()));
    let output_reader = Arc::clone(&output);

    thread::spawn(move || {
        let mut buf = [0_u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => output_reader
                    .lock()
                    .expect("lock PTY output")
                    .extend_from_slice(&buf[..n]),
            }
        }
    });

    let startup_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < startup_deadline {
        if !output.lock().expect("lock PTY output").is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    for byte in b"/quit\r" {
        writer
            .write_all(std::slice::from_ref(byte))
            .expect("write quit command byte");
        writer.flush().expect("flush quit command byte");
        thread::sleep(Duration::from_millis(30));
    }
    writer.flush().expect("flush quit command");
    drop(writer);

    let (status_tx, status_rx) = mpsc::channel();
    thread::spawn(move || {
        let status = child.wait().expect("wait for tui process");
        let _ = status_tx.send(status);
    });

    let status = status_rx
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|_| {
            let captured = output.lock().expect("lock PTY output").clone();
            let output = String::from_utf8_lossy(&captured);
            panic!("tui process should exit after /quit; PTY output: {output}");
        });

    assert!(
        status.success(),
        "tui process exited unsuccessfully: {status:?}"
    );
}
