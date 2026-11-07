use super::Bluetooth;
use super::l2cap::{L2CAP_CID_HID_CONTROL, L2CAP_PSM_HID_CONTROL};

const EV_DISCONNECTION_COMPLETE: u8 = 0x05;
const EV_NUMBER_OF_COMPLETED_PACKETS: u8 = 0x13;
const EV_COMMAND_COMPLETE: u8 = 0x0E;
const EV_COMMAND_STATUS: u8 = 0x0F;
const EV_CONNECTION_COMPLETE: u8 = 0x03;
const EV_CONNECTION_REQUEST: u8 = 0x04;
const EV_AUTHENTICATION_COMPLETE: u8 = 0x06;
const EV_REMOTE_NAME_REQUEST_COMPLETE: u8 = 0x07;
const EV_READ_REMOTE_SUPPORTED_FEATURES_COMPLETE: u8 = 0x0B;
const EV_READ_REMOTE_VERSION_INFORMATION_COMPLETE: u8 = 0x0C;
const EV_ROLE_CHANGE: u8 = 0x12;
const EV_MODE_CHANGE: u8 = 0x14;
const EV_RETURN_LINK_KEYS: u8 = 0x15;
const EV_READ_CLOCK_OFFSET_COMPLETE: u8 = 0x1C;
const EV_CONNECTION_PACKET_TYPE_CHANGED: u8 = 0x1D;

const OP_DISCONNECT: u16 = 0x0406;
const OP_RESET: u16 = 0x0C03;
const OP_READ_LOCAL_VERSION: u16 = 0x1001;
const OP_READ_LOCAL_FEATURES: u16 = 0x1003;
const OP_READ_BUFFER_SIZE: u16 = 0x1005;
const OP_READ_BD_ADDR: u16 = 0x1009;
const OP_WRITE_LOCAL_NAME: u16 = 0x0C13;
const OP_READ_STORED_LINK_KEY: u16 = 0x0C0D;
const OP_DELETE_STORED_LINK_KEY: u16 = 0x0C12;
const OP_WRITE_PIN_TYPE: u16 = 0x0C0A;
const OP_WRITE_PAGE_TIMEOUT: u16 = 0x0C18;
const OP_WRITE_SCAN_ENABLE: u16 = 0x0C1A;
const OP_WRITE_CLASS_OF_DEVICE: u16 = 0x0C24;
const OP_HOST_BUFFER_SIZE: u16 = 0x0C33;
const OP_WRITE_LINK_SUPERVISION_TIMEOUT: u16 = 0x0C37;
const OP_WRITE_INQUIRY_SCAN_TYPE: u16 = 0x0C43;
const OP_WRITE_INQUIRY_MODE: u16 = 0x0C45;
const OP_WRITE_PAGE_SCAN_TYPE: u16 = 0x0C47;
const OP_ACCEPT_CONNECTION_REQUEST: u16 = 0x0409;
const OP_CHANGE_CONNECTION_PACKET_TYPE: u16 = 0x040F;
const OP_AUTHENTICATION_REQUESTED: u16 = 0x0411;
const OP_REMOTE_NAME_REQUEST: u16 = 0x0419;
const OP_READ_REMOTE_SUPPORTED_FEATURES: u16 = 0x041B;
const OP_READ_REMOTE_VERSION_INFORMATION: u16 = 0x041D;
const OP_READ_CLOCK_OFFSET: u16 = 0x041F;
const OP_SNIFF_MODE: u16 = 0x0803;
const OP_WRITE_LINK_POLICY_SETTINGS: u16 = 0x080D;
const OP_VENDOR_SPECIFIC_4C: u16 = 0xFC4C;
const OP_VENDOR_SPECIFIC_4F: u16 = 0xFC4F;

pub(crate) const WIIMOTE_ADDR: [u8; 6] = [0x11, 0x02, 0x19, 0x79, 0x00, 0x00];
const WIIMOTE_LINK_KEY: [u8; 16] = [
    0x41, 0x20, 0x76, 0x69, 0x72, 0x74, 0x75, 0x61, 0x6C, 0x20, 0x57, 0x69, 0x69, 0x6D, 0x6F, 0x74,
];
pub(crate) const WIIMOTE_NAME: &[u8] = b"Nintendo RVL-CNT-01";

impl Bluetooth {
    pub(super) fn handle_hci_command(&mut self, packet: &[u8]) {
        let opcode = u16::from_le_bytes([packet[0], packet[1]]);
        let params = packet.get(3..).unwrap_or(&[]);
        tracing::debug!(
            opcode = format!("{opcode:#06x}"),
            params = format!("{params:02X?}"),
            "received Bluetooth HCI command"
        );

        match opcode {
            OP_WRITE_SCAN_ENABLE => {
                let scan_enable = params.first().copied().unwrap_or(0);
                self.scanning = (scan_enable & 0x02) != 0;
                tracing::debug!(
                    scan_enable = format!("{scan_enable:#04x}"),
                    page_scan = self.scanning,
                    "Bluetooth scan enable"
                );
                self.queue_command_complete(opcode);
                if self.scanning && !self.connection_requested && !self.connected {
                    self.queue_connection_request();
                }
            }
            OP_ACCEPT_CONNECTION_REQUEST => {
                tracing::debug!("guest accepted fake Wiimote Bluetooth connection");
                self.queue_command_status(opcode);
                self.queue_role_change();
                self.queue_connection_complete();
                self.connected = true;
                self.queue_l2cap_connect_request(L2CAP_PSM_HID_CONTROL, L2CAP_CID_HID_CONTROL);
            }
            OP_REMOTE_NAME_REQUEST => {
                self.queue_command_status(opcode);
                self.queue_remote_name_request_complete();
            }
            OP_READ_REMOTE_SUPPORTED_FEATURES => {
                self.queue_command_status(opcode);
                self.queue_read_remote_supported_features_complete();
            }
            OP_READ_REMOTE_VERSION_INFORMATION => {
                self.queue_command_status(opcode);
                self.queue_read_remote_version_information_complete();
            }
            OP_READ_CLOCK_OFFSET => {
                self.queue_command_status(opcode);
                self.queue_read_clock_offset_complete();
            }
            OP_AUTHENTICATION_REQUESTED => {
                self.queue_command_status(opcode);
                self.queue_authentication_complete();
            }
            OP_CHANGE_CONNECTION_PACKET_TYPE => {
                self.queue_command_status(opcode);
                self.queue_connection_packet_type_changed();
            }
            OP_SNIFF_MODE => {
                self.queue_command_status(opcode);
                self.queue_mode_change();
            }
            OP_READ_STORED_LINK_KEY => {
                self.queue_return_link_keys();
                self.queue_command_complete(opcode);
            }
            OP_WRITE_LINK_SUPERVISION_TIMEOUT => {
                self.queue_command_complete(opcode);
            }
            OP_DISCONNECT => {
                let reason = params.get(2).copied().unwrap_or(0x13);
                tracing::warn!(reason = format!("{reason:#04x}"), "guest requested HCI disconnect");
                self.queue_command_status(opcode);
                self.queue_disconnection_complete(reason);
                self.connected = false;
                self.host_hid_control_cid = None;
                self.host_hid_interrupt_cid = None;
                self.connection_requested = false;
            }
            _ => self.queue_command_complete(opcode),
        }
    }

    fn queue_command_complete(&mut self, opcode: u16) {
        let payload: &[u8] = match opcode {
            OP_RESET
            | OP_WRITE_LOCAL_NAME
            | OP_DELETE_STORED_LINK_KEY
            | OP_WRITE_PIN_TYPE
            | OP_WRITE_PAGE_TIMEOUT
            | OP_WRITE_SCAN_ENABLE
            | OP_WRITE_CLASS_OF_DEVICE
            | OP_HOST_BUFFER_SIZE
            | OP_WRITE_INQUIRY_SCAN_TYPE
            | OP_WRITE_INQUIRY_MODE
            | OP_WRITE_PAGE_SCAN_TYPE
            | OP_VENDOR_SPECIFIC_4C
            | OP_VENDOR_SPECIFIC_4F => &[0x00],
            OP_READ_STORED_LINK_KEY => &[
                0x00, // status
                0xFF, 0x00, // max_num_keys
                0x01, 0x00, // num_keys_read
            ],
            OP_WRITE_LINK_SUPERVISION_TIMEOUT | OP_WRITE_LINK_POLICY_SETTINGS => &[
                0x00, // status
                0x00, 0x01, // connection handle
            ],
            OP_READ_LOCAL_VERSION => &[
                0x00, // status
                0x03, // HCI v1.2
                0x40, 0x0E, // HCI revision
                0x03, // LMP v1.2
                0x0F, 0x00, // manufacturer = Broadcom
                0x40, 0x0E, // LMP subversion
            ],
            OP_READ_LOCAL_FEATURES => &[0x00, 0xBC, 0x02, 0x04, 0x38, 0x08, 0x08, 0x00, 0x00],
            OP_READ_BUFFER_SIZE => &[
                0x00, 0x53, 0x01, // ACL packet length = 339
                0x40, // SCO packet length = 64
                0x08, 0x00, // num ACL packets
                0x08, 0x00, // num SCO packets
            ],
            OP_READ_BD_ADDR => &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66],
            _ => &[0x00],
        };

        let mut buf = Vec::with_capacity(3 + payload.len());
        buf.push(0x01); // num_hci_command_packets
        buf.push(opcode as u8);
        buf.push((opcode >> 8) as u8);
        buf.extend_from_slice(payload);
        self.queue_hci_event(EV_COMMAND_COMPLETE, &buf);
    }

    pub(super) fn queue_hci_event(&mut self, code: u8, payload: &[u8]) {
        let mut event = Vec::with_capacity(2 + payload.len());
        event.push(code);
        event.push(payload.len() as u8);
        event.extend_from_slice(payload);
        super::push_capped(&mut self.pending_hci, event);
    }

    fn queue_command_status(&mut self, opcode: u16) {
        self.queue_hci_event(EV_COMMAND_STATUS, &[0x00, 0x01, opcode as u8, (opcode >> 8) as u8]);
    }

    fn queue_return_link_keys(&mut self) {
        tracing::debug!(
            addr = format!("{WIIMOTE_ADDR:02X?}"),
            "queueing fake Wiimote stored link key"
        );
        let mut payload = Vec::with_capacity(23);
        payload.push(0x01);
        payload.extend_from_slice(&WIIMOTE_ADDR);
        payload.extend_from_slice(&WIIMOTE_LINK_KEY);
        self.queue_hci_event(EV_RETURN_LINK_KEYS, &payload);
    }

    fn queue_connection_request(&mut self) {
        self.connection_requested = true;
        tracing::debug!("queueing fake Wiimote Bluetooth connection request");
        let mut payload = Vec::with_capacity(10);
        payload.extend_from_slice(&WIIMOTE_ADDR);
        payload.extend_from_slice(&[0x04, 0x25, 0x00, 0x01]);
        self.queue_hci_event(EV_CONNECTION_REQUEST, &payload);
    }

    fn queue_role_change(&mut self) {
        let mut payload = Vec::with_capacity(8);
        payload.push(0x00);
        payload.extend_from_slice(&WIIMOTE_ADDR);
        payload.push(0x00);
        self.queue_hci_event(EV_ROLE_CHANGE, &payload);
    }

    pub(super) fn queue_number_of_completed_packets(&mut self, count: u16) {
        self.queue_hci_event(
            EV_NUMBER_OF_COMPLETED_PACKETS,
            &[0x01, 0x00, 0x01, count as u8, (count >> 8) as u8],
        );
    }

    fn queue_disconnection_complete(&mut self, reason: u8) {
        self.queue_hci_event(EV_DISCONNECTION_COMPLETE, &[0x00, 0x00, 0x01, reason]);
    }

    fn queue_connection_complete(&mut self) {
        let mut payload = Vec::with_capacity(11);
        payload.extend_from_slice(&[0x00, 0x00, 0x01]);
        payload.extend_from_slice(&WIIMOTE_ADDR);
        payload.extend_from_slice(&[0x01, 0x00]);
        self.queue_hci_event(EV_CONNECTION_COMPLETE, &payload);
    }

    fn queue_remote_name_request_complete(&mut self) {
        let mut payload = vec![0u8; 255];
        payload[0] = 0x00;
        payload[1..7].copy_from_slice(&WIIMOTE_ADDR);
        payload[7..7 + WIIMOTE_NAME.len()].copy_from_slice(WIIMOTE_NAME);
        self.queue_hci_event(EV_REMOTE_NAME_REQUEST_COMPLETE, &payload);
    }

    fn queue_read_remote_supported_features_complete(&mut self) {
        self.queue_hci_event(
            EV_READ_REMOTE_SUPPORTED_FEATURES_COMPLETE,
            &[0x00, 0x00, 0x01, 0xBC, 0x02, 0x04, 0x38, 0x08, 0x00, 0x00, 0x00],
        );
    }

    fn queue_read_remote_version_information_complete(&mut self) {
        self.queue_hci_event(
            EV_READ_REMOTE_VERSION_INFORMATION_COMPLETE,
            &[0x00, 0x00, 0x01, 0x02, 0x0F, 0x00, 0x29, 0x02],
        );
    }

    fn queue_read_clock_offset_complete(&mut self) {
        self.queue_hci_event(EV_READ_CLOCK_OFFSET_COMPLETE, &[0x00, 0x00, 0x01, 0x18, 0x38]);
    }

    fn queue_authentication_complete(&mut self) {
        self.queue_hci_event(EV_AUTHENTICATION_COMPLETE, &[0x00, 0x00, 0x01]);
    }

    fn queue_connection_packet_type_changed(&mut self) {
        self.queue_hci_event(EV_CONNECTION_PACKET_TYPE_CHANGED, &[0x00, 0x00, 0x01, 0x18, 0xCC]);
    }

    fn queue_mode_change(&mut self) {
        self.queue_hci_event(EV_MODE_CHANGE, &[0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    }
}
