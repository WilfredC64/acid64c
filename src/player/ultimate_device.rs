// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::{str, thread, time};
use std::net::UdpSocket;
use std::path::Path;
use std::time::Instant;
use attohttpc::{Error, Multipart, MultipartBuilder, MultipartFile, Response};

use crate::utils::{sid_file, network};

use super::sid_device::{DeviceId, DeviceInfo, DeviceResponse, SamplingMethod, SidClock, SidDevice, SidModel};

const TOTAL_TIMEOUT: u64 = 5000;
const CONNECTION_TIMEOUT: u64 = 500;
const SOCKET_TIMEOUT: u128 = 100;
const MAX_DATA_SIZE: usize = 512;

const PAUSE_SID_FILE: &[u8] = include_bytes!("../../resources/acid64_pause.crt");
const MIN_WAIT_TIME_BUSY_MILLIS: u64 = 20;
const MIN_CYCLES_IN_FIFO: u32 = 4 * 312 * 63;

const GET_VERSION_ENDPOINT: &str = "/v1/version";
const SID_PLAY_ENDPOINT: &str = "/v1/runners:sidplay";
const RUN_PRG_ENDPOINT: &str = "/v1/runners:run_prg";
const RUN_CRT_ENDPOINT: &str = "/v1/runners:run_crt";

const SONG_NR_PARAM: &str = "songnr";

const MAGIC_ID: &[u8] = b"Any Ultimates around?";
const PING_RETRY_COUNT: i32 = 5;

pub struct UltimateDeviceFacade {
    pub us_device: UltimateDevice
}

impl SidDevice for UltimateDeviceFacade {
    fn get_device_id(&mut self, _dev_nr: i32) -> DeviceId { DeviceId::UltimateDevice }

    fn disconnect(&mut self, _dev_nr: i32) {
        self.us_device.disconnect();
    }

    fn is_connected(&mut self, _dev_nr: i32) -> bool {
        self.us_device.is_connected()
    }

    fn get_last_error(&mut self, _dev_nr: i32) -> Option<String> {
        self.us_device.get_last_error()
    }

    fn test_connection(&mut self, _dev_nr: i32) {
        self.us_device.test_connection();
    }

    fn can_pair_devices(&mut self, _dev1: i32, _dev2: i32) -> bool {
        true
    }

    fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.us_device.get_device_count()
    }

    fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo {
        self.us_device.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, _dev_nr: i32, _sid_count: i32) {
        // not supported
    }

    fn set_sid_position(&mut self, _dev_nr: i32, _sid_position: i8) {
        // not supported
    }

    fn set_sid_model(&mut self, _dev_nr: i32, _sid_socket: i32, _sid_model: SidModel) {
        // not supported
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.us_device.set_sid_clock(sid_clock);
    }

    fn set_sampling_method(&mut self, _dev_nr: i32, _sampling_method: SamplingMethod) {
        // not supported
    }

    fn set_sid_header(&mut self, _dev_nr: i32, _sid_header: Vec<u8>) {
        // not supported
    }

    fn set_fade_in(&mut self, _dev_nr: i32, _time_millis: u32) {
        // not supported
    }

    fn set_fade_out(&mut self, _dev_nr: i32, _time_millis: u32) {
        // not supported
    }

    fn silent_all_sids(&mut self, _dev_nr: i32, _write_volume: bool) {
        // not supported
    }

    fn silent_active_sids(&mut self, _dev_nr: i32, _write_volume: bool) {
        // not supported
    }

    fn reset_all_sids(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn reset_active_sids(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn reset_all_buffers(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn enable_turbo_mode(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn disable_turbo_mode(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn dummy_write(&mut self, _dev_nr: i32, _cycles: u32) {
        // not supported
    }

    fn write(&mut self, _dev_nr: i32, cycles: u32, _reg: u8, _data: u8) -> DeviceResponse {
        self.us_device.write(cycles);
        DeviceResponse::Ok
    }

    fn try_write(&mut self, _dev_nr: i32, cycles: u32, _reg: u8, _data: u8) -> DeviceResponse {
        self.us_device.write(cycles);
        DeviceResponse::Ok
    }

    fn retry_write(&mut self, _dev_nr: i32) -> DeviceResponse {
        DeviceResponse::Ok
    }

    fn force_flush(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn set_native_device_clock(&mut self, _enabled: bool) {
        // not supported
    }

    fn get_device_clock(&mut self, _dev_nr: i32) -> SidClock {
        self.us_device.get_device_clock()
    }

    fn has_remote_sidplayer(&mut self, _dev_nr: i32) -> bool {
        true
    }

    fn send_sid(&mut self, _dev_nr: i32, filename: &str, song_number: i32, sid_data: &[u8], ssl_data: &[u8]) {
        self.us_device.send_sid(filename, song_number, sid_data, ssl_data);
    }

    fn stop_sid(&mut self, _dev_nr: i32) {
        self.us_device.stop_sid();
    }

    fn set_cycles_in_fifo(&mut self, _dev_nr: i32, cycles: u32) {
        self.us_device.set_cycles_in_fifo(cycles);
    }
}

pub struct UltimateDevice {
    device_count: i32,
    cycles_in_fifo: u32,
    sid_clock: SidClock,
    last_error: Option<String>,
    server_url: Option<String>,
    socket: Option<UdpSocket>,
    socket_url: Option<String>,
    last_ping: Instant,
    retry_count: i32
}

impl UltimateDevice {
    pub fn new() -> UltimateDevice {
        UltimateDevice {
            device_count: 0,
            cycles_in_fifo: 0,
            sid_clock: SidClock::Pal,
            last_error: None,
            server_url: None,
            socket: None,
            socket_url: None,
            last_ping: Instant::now(),
            retry_count: 0
        }
    }

    pub fn connect(&mut self, ip_address: &str, port: &str) -> Result<(), String> {
        self.init_to_default();
        self.last_error = None;

        let server_url = format!("http://{}", [ip_address, port].join(":"));

        if network::is_local_ip_address(ip_address) {
            self.server_url = Some(server_url.clone());
        } else {
            self.server_url = None;
            return if ip_address.is_empty() {
                Err("No IP address configured for Ultimate device".to_string())
            } else {
                Err("IP is not a local IP address.".to_string())
            }
        }

        self.test_connection();

        if self.is_connected() {
            self.socket = Some(Self::bind_socket().map_err(|_| format!("Could not connect to: {}.", &ip_address))?);
            self.socket_url = Some([ip_address, "64"].join(":"));
            Ok(())
        } else {
            Err(format!("Could not connect to: {}.", &server_url))
        }
    }

    fn bind_socket() -> Result<UdpSocket, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        Ok(socket)
    }

    pub fn disconnect(&mut self) {
        self.init_to_default();
    }

    fn init_to_default(&mut self) {
        self.device_count = 0;
        self.sid_clock = SidClock::Pal;
        self.socket = None;
        self.socket_url = None;
        self.retry_count = 0;
    }

    fn disconnect_with_error(&mut self, error_message: String) {
        self.last_error = Some(error_message);
        self.disconnect();
    }

    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.device_count > 0
    }

    pub fn test_connection(&mut self) {
        self.device_count = 0;
        if let Some(server_url) = self.server_url.as_ref() {
            if let Ok(response) = Self::get_version(server_url) {
                if response.is_success() {
                    self.device_count = 1;
                } else {
                    self.disconnect();
                }
            } else {
                self.disconnect();
            }
        }
    }

    fn get_version(server_url: &str) -> Result<Response, Error> {
        attohttpc::get(format!("{server_url}{GET_VERSION_ENDPOINT}"))
            .timeout(time::Duration::from_millis(TOTAL_TIMEOUT))
            .read_timeout(time::Duration::from_millis(TOTAL_TIMEOUT))
            .connect_timeout(time::Duration::from_millis(CONNECTION_TIMEOUT)).send()
    }

    /// Checks connection status by sending a UDP packet to the device and inspecting response.
    /// Retries for a predefined number of times if no response received within the timeframe.
    /// Closes connection if no response is received after retries or on error.
    fn watchdog(&mut self) {
        if let Some(socket) = self.socket.as_ref() {
            if self.last_ping.elapsed().as_millis() > SOCKET_TIMEOUT {
                if self.retry_count <= PING_RETRY_COUNT && socket.send_to(MAGIC_ID, self.socket_url.as_ref().unwrap()).is_ok() {
                    self.last_ping = Instant::now();
                    self.retry_count += 1;
                    return;
                }
                self.disconnect();
            } else {
                let mut recv_buff = [0; MAX_DATA_SIZE];
                if let Ok((size, _)) = socket.recv_from(&mut recv_buff) {
                    if size >= MAGIC_ID.len() && recv_buff[0..MAGIC_ID.len()].eq(MAGIC_ID) {
                        self.retry_count = 0;
                    }
                }
            }
        }
    }

    pub fn get_device_count(&self) -> i32 {
        self.device_count
    }

    pub fn get_device_info(&mut self, _dev_nr: i32) -> DeviceInfo {
        DeviceInfo { id: "Ultimate Device".to_string(), name: "Ultimate Device".to_string() }
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
    }

    pub fn get_device_clock(&self) -> SidClock {
        self.sid_clock
    }

    fn write(&mut self, _cycles: u32) {
        if self.cycles_in_fifo > MIN_CYCLES_IN_FIFO {
            self.watchdog();

            if self.is_connected() {
                thread::sleep(time::Duration::from_millis(MIN_WAIT_TIME_BUSY_MILLIS));
            }
        }
    }

    fn send_sid(&mut self, filename: &str, song_number: i32, sid_data: &[u8], ssl_data: &[u8]) {
        let filename = Path::new(filename).file_name().unwrap().to_str().unwrap();

        if filename.ends_with(".mus") || filename.ends_with(".str") {
            let mut psid_header = [0; 0x7c];
            psid_header[0x00] = b'P';
            psid_header[0x01] = b'S';
            psid_header[0x02] = b'I';
            psid_header[0x03] = b'D';
            psid_header[0x05] = 0x02;
            psid_header[0x07] = 0x7c;
            psid_header[0x0f] = 0x01;
            psid_header[0x11] = 0x01;
            psid_header[0x77] = 0x29;

            let filename_only = filename.split('.').next().unwrap().to_string();

            for (i, char) in filename_only.as_bytes().iter().enumerate() {
                if *char == b'_' {
                    psid_header[0x16 + i] = b' ';
                } else {
                    psid_header[0x16 + i] = *char;
                }
            }

            self.send_sid_file(filename, song_number, [psid_header.as_slice(), sid_data].concat().as_slice(), ssl_data);
        } else if sid_file::is_sid_file(sid_data) {
            self.send_sid_file(filename, song_number, sid_data, ssl_data);
        } else if filename.ends_with(".prg") {
            let form = MultipartBuilder::new()
                .with_file(Self::create_part( "prg", filename, sid_data))
                .build().unwrap();

            let url = format!("{}{RUN_PRG_ENDPOINT}", &self.server_url.as_ref().unwrap());
            self.send_file(url, form);
        } else {
            self.disconnect_with_error("File type not supported".to_string());
        }
    }

    fn send_sid_file(&mut self, filename: &str, song_number: i32, sid_data: &[u8], ssl_data: &[u8]) {
        let filename = filename.split('.').next().unwrap().to_string() + ".sid";
        let ssl_filename = Self::get_ssl_filename(&filename);
        let form = MultipartBuilder::new()
            .with_file(Self::create_part("ssl", &ssl_filename, ssl_data))
            .with_file(Self::create_part("sid", &filename, sid_data))
            .build().unwrap();

        let url = format!("{}{SID_PLAY_ENDPOINT}?{SONG_NR_PARAM}={}", &self.server_url.as_ref().unwrap(), song_number + 1);
        self.send_file(url, form);
    }

    pub fn stop_sid(&mut self) {
        let form = MultipartBuilder::new()
            .with_file(Self::create_part( "crt", "acid64_pause.crt", PAUSE_SID_FILE))
            .build().unwrap();

        let url = format!("{}{RUN_CRT_ENDPOINT}", &self.server_url.as_ref().unwrap());
        self.send_file(url, form);
    }

    fn set_cycles_in_fifo(&mut self, cycles: u32) {
        self.cycles_in_fifo = cycles;
    }

    fn send_file(&mut self, url: String, form: Multipart) {
        let response = attohttpc::post(url).body(form)
            .timeout(time::Duration::from_millis(TOTAL_TIMEOUT))
            .read_timeout(time::Duration::from_millis(TOTAL_TIMEOUT))
            .connect_timeout(time::Duration::from_millis(CONNECTION_TIMEOUT))
            .send();
        self.handle_response(response);
    }

    fn handle_response(&mut self, response: Result<Response, Error>) {
        match response {
            Ok(response) => if let Err(error) = response.error_for_status() {
                self.disconnect_with_error(format!("HTTP error with status: {error}"));
            },
            Err(_) => {
                self.disconnect_with_error("Could not send SID file. Connection failed.".to_string());
            }
        }
    }

    fn create_part<'a>(name: &'a str, filename: &'a str, data: &'a [u8]) -> MultipartFile<'a, 'a> {
        MultipartFile::new(name, data)
            .with_type("application/octet-stream").unwrap()
            .with_filename(filename)
    }

    fn get_ssl_filename(sid_filename: &str) -> String {
        sid_filename.replace(".sid", ".ssl")
    }
}
