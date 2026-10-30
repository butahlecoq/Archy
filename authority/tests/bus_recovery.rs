use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use zbus::connection::Builder;

fn start_bus(config: &Path) -> (Child, String) {
    let mut child = Command::new("dbus-daemon")
        .arg("--nofork")
        .arg(format!("--config-file={}", config.display()))
        .arg("--print-address=1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.starts_with("unix:"), "private bus failed to start");
    (child, line.trim().to_owned())
}

#[test]
fn real_private_bus_disconnect_and_restart_reacquires_service_name() {
    let base = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join(format!("archy-bus-restart-{}", std::process::id()));
    fs::create_dir(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    let config = base.join("bus.conf");
    fs::write(&config, format!(r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig><type>session</type><listen>unix:path={}</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>"#, base.join("bus.sock").display())).unwrap();
    let (mut first_bus, address) = start_bus(&config);
    zbus::block_on(async {
        let first = Builder::address(address.as_str())
            .unwrap()
            .name("org.archy.RecoveryProbe")
            .unwrap()
            .build()
            .await
            .unwrap();
        let old_guid = first.server_guid().to_string();
        first_bus.kill().unwrap();
        first_bus.wait().unwrap();
        fs::remove_file(base.join("bus.sock")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !first.is_closed() && Instant::now() < deadline {
            async_io::Timer::after(Duration::from_millis(10)).await;
        }
        assert!(first.is_closed(), "service must observe bus disconnect");
        drop(first);
        let (mut restarted, new_address) = start_bus(&config);
        assert_eq!(
            new_address.split(",guid=").next(),
            address.split(",guid=").next(),
            "bus must return on the same socket"
        );
        let second = Builder::address(new_address.as_str())
            .unwrap()
            .name("org.archy.RecoveryProbe")
            .unwrap()
            .build()
            .await
            .unwrap();
        assert_ne!(second.server_guid().to_string(), old_guid);
        assert!(!second.is_closed());
        drop(second);
        restarted.kill().unwrap();
        restarted.wait().unwrap();
    });
    fs::remove_dir_all(base).unwrap();
}
