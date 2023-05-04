use std::os::unix::net::UnixDatagram;

// https://systemd.io/JOURNAL_NATIVE_PROTOCOL/
// https://www.freedesktop.org/software/systemd/man/systemd.journal-fields.html
//
// Fields:
// MESSAGE=This is the message\n
// //MESSAGE_ID=<128-bit-UUID-for-types-of-messages-in-hex>
// PRIORITY=0..7 (emerg..debug)
// //CODE_FILE=, CODE_LINE=, CODE_FUNC=
// //ERRNO=last_errno()
// //INVOCATION_ID=, USER_INVOCATION_ID=
// SYSLOG_FACILITY=24..128 (daemon..user0)
// SYSLOG_IDENTIFIER=this-awesome-program
// SYSLOG_PID=12345,
// SYSLOG_TIMESTAMP=<which-format??>
// //SYSLOG_RAW=
// //DOCUMENTATION=
// //TID=12346
// //UNIT=
// //USER_UNIT=

static JOURNALD_SOCK: &str = "/run/systemd/journal/socket";

// $ sudo journalctl -t this-awesome-program
// mei 04 17:32:54 hostname this-awesome-program[639175]: This is the message
// $ sudo journalctl -t this-awesome-program -o verbose
// Thu 2023-05-04 17:32:54.095778 CEST [s=b127..;i=2d141;b=af4..;m=89b..;t=5fa..;x=5ef..]
//     _TRANSPORT=journal
//     _UID=1000
//     _GID=1000
//     _BOOT_ID=af4..
//     _MACHINE_ID=4cf..
//     _HOSTNAME=hostname
//     MESSAGE=This is the message
//     PRIORITY=3
//     SYSLOG_FACILITY=128
//     SYSLOG_IDENTIFIER=this-awesome-program
//     SYSLOG_PID=12345
//     _PID=639175
//     _SOURCE_REALTIME_TIMESTAMP=1683214374095778

fn main() -> std::io::Result<()> {
    let socket = UnixDatagram::unbound()?;

    // NOTE: For MESSAGE= with embedded LFs (or binary data), we need a
    // slightly altered syntax. 
    let out = b"MESSAGE=This is the message\nPRIORITY=3\nSYSLOG_FACILITY=128\nSYSLOG_IDENTIFIER=this-awesome-program\nSYSLOG_PID=12345\n";
    socket.send_to(out, JOURNALD_SOCK)?;
    // "return 0"
    Ok(())
}
