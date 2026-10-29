// Mirrors nix's test_recvmsg_rxq_ovfl and prints what the kernel reports.
use nix::sys::socket::sockopt::{RcvBuf, RxqOvfl};
use nix::sys::socket::*;
use std::io::{IoSlice, IoSliceMut};
use std::os::fd::AsRawFd;
use std::process::Command;
use std::str::FromStr;

fn skmem(port: u16) -> String {
    let out = Command::new("ss")
        .args(["-u", "-a", "-m", "-n", &format!("sport = :{port}")])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("skmem"))
        .map(|l| l.trim().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn run(msg_len: usize) {
    let message = vec![0u8; msg_len];
    let bufsize = 4096;
    let in_socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).unwrap();
    let out_socket = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).unwrap();
    let localhost = SockaddrIn::from_str("127.0.0.1:0").unwrap();
    bind(in_socket.as_raw_fd(), &localhost).unwrap();
    let address: SockaddrIn = getsockname(in_socket.as_raw_fd()).unwrap();
    connect(out_socket.as_raw_fd(), &address).unwrap();
    setsockopt(&in_socket, RxqOvfl, &1).unwrap();
    setsockopt(&in_socket, RcvBuf, &bufsize).unwrap();
    println!("-- msg_len={msg_len} SO_RCVBUF={}", getsockopt(&in_socket, RcvBuf).unwrap());
    println!("   start: {}", skmem(address.port()));
    let iov = [IoSlice::new(&message)];
    for i in 1..=4 {
        sendmsg(out_socket.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&address)).unwrap();
        println!("   after send {i}: {}", skmem(address.port()));
    }
    let mut received = 0;
    loop {
        let mut buffer = vec![0u8; msg_len];
        let mut cmsgspace = nix::cmsg_space!(u32);
        let mut iov = [IoSliceMut::new(&mut buffer)];
        match recvmsg::<()>(in_socket.as_raw_fd(), &mut iov, Some(&mut cmsgspace), MsgFlags::MSG_DONTWAIT) {
            Ok(_) => received += 1,
            Err(nix::Error::EAGAIN) => break,
            Err(e) => panic!("{e}"),
        }
    }
    println!("   received {received} of 4; after recv: {}", skmem(address.port()));
}

fn main() {
    println!("kernel={}", std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim());
    for len in [100, 1000, 1400, 1500, 2048, 4000] {
        run(len);
    }
}
