// Copyright (C) 2019 - 2021 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use get_if_addrs::IfAddr;
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::str::FromStr;

pub fn is_local_ip_address(host_name: &str) -> bool {
    if let Some(local_ip_address) = resolve_local_ip(host_name) {
        is_ip_in_local_network(&local_ip_address)
    } else {
        false
    }
}

fn is_ip_in_local_network(local_ip_address: &str) -> bool {
    for if_addr in get_if_addrs::get_if_addrs().unwrap() {
        if let IfAddr::V4(ref ip_addr) = if_addr.addr {
            let ip_addr_netmask = ip_addr.netmask.to_string();
            let masked_local_ip = mask_ip_address(&ip_addr.ip.to_string(), &ip_addr_netmask);
            let masked_host_ip = mask_ip_address(local_ip_address, &ip_addr_netmask);
            if masked_host_ip == masked_local_ip {
                return true;
            }
        }
    }
    false
}

fn resolve_local_ip(host_name: &str) -> Option<String> {
    let ip_addresses = (host_name, 0).to_socket_addrs()
        .map(|iter| iter.filter(|socket_address| socket_address.is_ipv4())
            .map(|socket_address| socket_address.ip().to_string()).collect::<Vec<_>>());

    if ip_addresses.is_ok() {
        for ip_address in ip_addresses.unwrap() {
            if is_local(&ip_address) {
                return Some(ip_address);
            }
        }
    }
    None
}

fn is_local(host_name: &str) -> bool {
    if let Ok(localhost) = Ipv4Addr::from_str(host_name) {
        localhost.is_loopback() || localhost.is_private()
    } else {
        false
    }
}

fn mask_ip_address(ip_address: &str, netmask: &str) -> Result<String, String> {
    let ip_address: Vec<&str> = ip_address.split('.').collect();
    let netmask: Vec<&str> = netmask.split('.').collect();

    if ip_address.len() == netmask.len() {
        let mut masked_ip = Vec::new();
        for i in 0..ip_address.len() {
            masked_ip.push((text_to_u8(ip_address[i]) & text_to_u8(netmask[i])).to_string());
        }
        return Ok(masked_ip.join("."));
    }

    Err("Invalid ip or netmask.".to_string())
}

fn text_to_u8(text: &str) -> u8 {
    text.parse::<u8>().unwrap_or(0)
}
