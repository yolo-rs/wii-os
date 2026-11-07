use super::Bluetooth;

pub(super) const L2CAP_CID_SIGNALING: u16 = 0x0001;
pub(super) const L2CAP_CID_HID_CONTROL: u16 = 0x0040;
pub(super) const L2CAP_CID_HID_INTERRUPT: u16 = 0x0041;
pub(super) const L2CAP_PSM_HID_CONTROL: u16 = 0x0011;
pub(super) const L2CAP_PSM_HID_INTERRUPT: u16 = 0x0013;

const L2CAP_CONNECT_REQ: u8 = 0x02;
const L2CAP_CONNECT_RSP: u8 = 0x03;
const L2CAP_CONFIG_REQ: u8 = 0x04;
const L2CAP_CONFIG_RSP: u8 = 0x05;
const L2CAP_DISCONNECT_REQ: u8 = 0x06;

impl Bluetooth {
    pub(super) fn handle_acl_out(&mut self, packet: &[u8]) {
        if packet.len() < 8 {
            return;
        }

        let l2cap_len = u16::from_le_bytes([packet[4], packet[5]]) as usize;
        let cid = u16::from_le_bytes([packet[6], packet[7]]);
        let Some(payload) = packet.get(8..8 + l2cap_len.min(packet.len().saturating_sub(8))) else {
            return;
        };

        match cid {
            L2CAP_CID_SIGNALING => self.handle_l2cap_signaling(payload),
            L2CAP_CID_HID_CONTROL | L2CAP_CID_HID_INTERRUPT => self.handle_wiimote_hid(payload),
            _ => tracing::debug!(
                cid = format!("{cid:#06x}"),
                "ignored ACL payload for unknown L2CAP channel"
            ),
        }
    }

    fn handle_l2cap_signaling(&mut self, mut payload: &[u8]) {
        while payload.len() >= 4 {
            let code = payload[0];
            let ident = payload[1];
            let len = u16::from_le_bytes([payload[2], payload[3]]) as usize;
            if payload.len() < 4 + len {
                return;
            }

            let data = &payload[4..4 + len];
            match code {
                L2CAP_CONNECT_REQ if data.len() >= 4 => {
                    let psm = u16::from_le_bytes([data[0], data[1]]);
                    let source_cid = u16::from_le_bytes([data[2], data[3]]);

                    match psm {
                        L2CAP_PSM_HID_CONTROL => {
                            self.host_hid_control_cid = Some(source_cid);
                            tracing::debug!(
                                host_cid = format!("{source_cid:#06x}"),
                                "guest connected Wiimote HID control channel"
                            );

                            self.queue_l2cap_connect_rsp(ident, source_cid, L2CAP_CID_HID_CONTROL, 0);
                            self.queue_l2cap_config_req(source_cid);
                        }
                        L2CAP_PSM_HID_INTERRUPT => {
                            self.host_hid_interrupt_cid = Some(source_cid);
                            tracing::debug!(
                                host_cid = format!("{source_cid:#06x}"),
                                "guest connected Wiimote HID interrupt channel"
                            );

                            self.queue_l2cap_connect_rsp(ident, source_cid, L2CAP_CID_HID_INTERRUPT, 0);
                            self.queue_l2cap_config_req(source_cid);
                        }
                        _ => {
                            tracing::debug!(psm = format!("{psm:#06x}"), "rejected unknown L2CAP PSM");
                            self.queue_l2cap_connect_rsp(ident, source_cid, 0, 2);
                        }
                    }
                }
                L2CAP_CONNECT_RSP if data.len() >= 8 => {
                    let dest_cid = u16::from_le_bytes([data[0], data[1]]);
                    let source_cid = u16::from_le_bytes([data[2], data[3]]);
                    let result = u16::from_le_bytes([data[4], data[5]]);
                    let status = u16::from_le_bytes([data[6], data[7]]);

                    if result == 0 {
                        if source_cid == L2CAP_CID_HID_CONTROL {
                            self.host_hid_control_cid = Some(dest_cid);
                        } else if source_cid == L2CAP_CID_HID_INTERRUPT {
                            self.host_hid_interrupt_cid = Some(dest_cid);
                        }

                        tracing::debug!(
                            source_cid = format!("{source_cid:#06x}"),
                            dest_cid = format!("{dest_cid:#06x}"),
                            "Wiimote L2CAP channel connected"
                        );

                        self.queue_l2cap_config_req(source_cid);
                    } else {
                        tracing::warn!(
                            source_cid = format!("{source_cid:#06x}"),
                            dest_cid = format!("{dest_cid:#06x}"),
                            result = format!("{result:#06x}"),
                            status = format!("{status:#06x}"),
                            "guest rejected Wiimote L2CAP connect request"
                        );
                    }
                }
                L2CAP_CONFIG_REQ if data.len() >= 4 => {
                    let channel = u16::from_le_bytes([data[0], data[1]]);
                    self.queue_l2cap_config_rsp(ident, channel);
                }
                L2CAP_CONFIG_RSP if data.len() >= 2 => {
                    let channel = u16::from_le_bytes([data[0], data[1]]);
                    if Some(channel) == self.host_hid_interrupt_cid {
                        tracing::debug!("Wiimote HID interrupt channel configured");
                        let report = self.wiimote.make_input_report();
                        self.queue_hid_input_report(report);
                    } else if Some(channel) == self.host_hid_control_cid {
                        tracing::debug!("Wiimote HID control channel configured");
                        if self.host_hid_interrupt_cid.is_none() {
                            self.queue_l2cap_connect_request(L2CAP_PSM_HID_INTERRUPT, L2CAP_CID_HID_INTERRUPT);
                        }
                    }
                }
                L2CAP_DISCONNECT_REQ => tracing::debug!("ignored Wiimote L2CAP disconnect request"),
                _ => tracing::debug!(code = format!("{code:#04x}"), "ignored L2CAP signaling command"),
            }

            payload = &payload[4 + len..];
        }
    }

    fn handle_wiimote_hid(&mut self, payload: &[u8]) {
        if payload.len() < 2 {
            return;
        }

        for report in self.wiimote.handle_output_report(payload) {
            self.queue_hid_input_report(report);
        }
    }

    pub(super) fn queue_hid_input_report(&mut self, report: Vec<u8>) {
        tracing::debug!(
            bytes = format!("{report:02X?}"),
            "sending Wiimote input report to guest"
        );

        let cid = self.host_hid_interrupt_cid.unwrap_or(L2CAP_CID_HID_INTERRUPT);
        self.queue_acl_l2cap(cid, &report);
    }

    fn queue_l2cap_connect_rsp(&mut self, ident: u8, source_cid: u16, dest_cid: u16, result: u16) {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&[L2CAP_CONNECT_RSP, ident, 0x08, 0x00]);
        payload.extend_from_slice(&dest_cid.to_le_bytes());
        payload.extend_from_slice(&source_cid.to_le_bytes());
        payload.extend_from_slice(&result.to_le_bytes());
        payload.extend_from_slice(&[0x00, 0x00]);
        self.queue_acl_l2cap(L2CAP_CID_SIGNALING, &payload);
    }

    pub(super) fn queue_l2cap_connect_request(&mut self, psm: u16, source_cid: u16) {
        let ident = self.next_l2cap_ident();
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&[L2CAP_CONNECT_REQ, ident, 0x04, 0x00]);
        payload.extend_from_slice(&psm.to_le_bytes());
        payload.extend_from_slice(&source_cid.to_le_bytes());
        self.queue_acl_l2cap(L2CAP_CID_SIGNALING, &payload);
    }

    fn queue_l2cap_config_req(&mut self, channel: u16) {
        let ident = self.next_l2cap_ident();
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&[L2CAP_CONFIG_REQ, ident, 0x08, 0x00]);
        payload.extend_from_slice(&channel.to_le_bytes());
        payload.extend_from_slice(&[0x00, 0x00, 0x01, 0x02, 0xB9, 0x00]);
        self.queue_acl_l2cap(L2CAP_CID_SIGNALING, &payload);
    }

    fn queue_l2cap_config_rsp(&mut self, ident: u8, channel: u16) {
        let mut payload = Vec::with_capacity(18);
        payload.extend_from_slice(&[L2CAP_CONFIG_RSP, ident, 0x0E, 0x00]);
        payload.extend_from_slice(&channel.to_le_bytes());
        payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x80, 0x02, 0x02, 0x02, 0xFF, 0xFF]);
        self.queue_acl_l2cap(L2CAP_CID_SIGNALING, &payload);
    }

    fn queue_acl_l2cap(&mut self, cid: u16, payload: &[u8]) {
        let l2cap_len = payload.len() as u16;
        let acl_len = l2cap_len + 4;
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&[0x00, 0x21]);
        frame.extend_from_slice(&acl_len.to_le_bytes());
        frame.extend_from_slice(&l2cap_len.to_le_bytes());
        frame.extend_from_slice(&cid.to_le_bytes());
        frame.extend_from_slice(payload);
        tracing::debug!(bytes = format!("{frame:02X?}"), "queued Bluetooth ACL frame");
        super::push_capped(&mut self.pending_acl, frame);
    }

    fn next_l2cap_ident(&mut self) -> u8 {
        let ident = self.next_l2cap_ident;
        self.next_l2cap_ident = self.next_l2cap_ident.wrapping_add(1).max(1);
        ident
    }
}
