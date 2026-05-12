use gorust::net::AsyncTcpStream;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct MyColumnInfo {
    pub name: String,
    pub data_type: u8,
    pub flags: u16,
    pub decimals: u8,
}

pub enum MyResult {
    Rows(Vec<Vec<String>>, Vec<MyColumnInfo>),
    Ok(u64, Option<i64>),
    Err(String),
}

pub struct MyConnection {
    stream: AsyncTcpStream,
    sequence_id: u8,
}

impl MyConnection {
    pub fn connect(addr: SocketAddr, username: &str, password: &str, database: &str) -> Result<Self, crate::error::Error> {
        let stream = AsyncTcpStream::connect(addr)?;
        let mut conn = MyConnection { stream, sequence_id: 0 };
        conn.read_handshake()?;
        conn.send_handshake_response(username, password, database)?;
        conn.read_auth_result()?;
        Ok(conn)
    }

    fn read_handshake(&mut self) -> Result<(), crate::error::Error> {
        let pkt_len = self.read_u24()?;
        let seq = self.read_u8()?;
        self.sequence_id = seq.wrapping_add(1);

        let _protocol_version = self.read_u8()?;
        let mut version = String::new();
        loop {
            let b = self.read_u8()?;
            if b == 0 { break; }
            version.push(b as char);
        }
        let _thread_id = self.read_u32()?;
        let mut auth_plugin_data = vec![0u8; 8];
        self.stream_read(&mut auth_plugin_data)?;
        self.read_u16()?;
        let _charset = self.read_u8()?;
        let _status = self.read_u16()?;
        let _auth_plugin_len = self.read_u8()?;
        self.skip(10)?;

        let remaining = pkt_len as usize - 36;
        if remaining > 0 {
            self.skip(remaining)?;
        }
        Ok(())
    }

    fn send_handshake_response(&mut self, username: &str, password: &str, database: &str) -> Result<(), crate::error::Error> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0x0285a2ffu32.to_le_bytes());
        payload.extend_from_slice(&0x21u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x21u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());
        payload.extend_from_slice(&0x00u32.to_le_bytes());

        payload.extend_from_slice(username.as_bytes());
        payload.push(0);

        let auth_response = if password.is_empty() {
            vec![0u8]
        } else {
            let mut resp = vec![0x14u8];
            let mut hash = [0u8; 20];
            for (i, b) in password.bytes().enumerate() {
                if i < 20 { hash[i] = b; }
            }
            resp.extend_from_slice(&hash);
            resp
        };
        payload.push(auth_response.len() as u8);
        payload.extend_from_slice(&auth_response);

        payload.extend_from_slice(database.as_bytes());
        payload.push(0);

        payload.extend_from_slice(b"mysql_native_password\0");

        self.send_packet(1, &payload)?;
        Ok(())
    }

    fn read_auth_result(&mut self) -> Result<(), crate::error::Error> {
        let pkt_len = self.read_u24()?;
        let _seq = self.read_u8()?;
        self.sequence_id = 2;

        let header = self.read_u8()?;
        match header {
            0x00 => {
                self.skip((pkt_len - 1) as usize)?;
                Ok(())
            }
            0xFF => {
                let _code = self.read_u16()?;
                let mut msg = String::new();
                for _ in 0..(pkt_len - 3) {
                    let b = self.read_u8()?;
                    msg.push(b as char);
                }
                Err(format!("MySQL error: {}", msg).into())
            }
            _ => {
                self.skip((pkt_len - 1) as usize)?;
                self.read_auth_result()
            }
        }
    }

    pub fn execute_query(&mut self, sql: &str) -> Result<MyResult, crate::error::Error> {
        self.sequence_id = 0;
        self.send_command(0x03, sql)?;

        let pkt_len = self.read_u24()?;
        let _seq = self.read_u8()?;
        self.sequence_id = 1;

        let first_byte = self.read_u8()?;
        match first_byte {
            0x00 => {
                let affected_rows = self.read_lenenc()?;
                let last_insert_id = self.read_lenenc()?;
                let _status = self.read_u16()?;
                let _warnings = self.read_u16()?;
                Ok(MyResult::Ok(affected_rows, Some(last_insert_id as i64)))
            }
            0xFF => {
                let _code = self.read_u16()?;
                let mut msg = String::new();
                for _ in 0..(pkt_len - 3) {
                    let b = self.read_u8()?;
                    msg.push(b as char);
                }
                Err(format!("MySQL error: {}", msg).into())
            }
            _ => {
                let num_cols = self.read_lenenc_from_byte(first_byte, &mut vec![0u8; 0])?;
                let mut columns = Vec::with_capacity(num_cols as usize);
                for _ in 0..num_cols {
                    let col = self.read_column_definition()?;
                    columns.push(col);
                }
                self.read_eof()?;

                let mut rows = Vec::new();
                loop {
                    let pkt_len = self.read_u24()?;
                    let _seq = self.read_u8()?;
                    if pkt_len < 9 {
                        let first = self.read_u8()?;
                        if first == 0xFE {
                            self.skip((pkt_len - 1) as usize)?;
                            break;
                        }
                        let mut buf = vec![first];
                        for _ in 0..(pkt_len - 1) {
                            buf.push(self.read_u8()?);
                        }
                        let mut row = Vec::with_capacity(columns.len());
                        let mut pos = 0;
                        for _ in 0..columns.len() {
                            if pos < buf.len() {
                                let len = buf[pos] as usize;
                                pos += 1;
                                if len < 251 {
                                    let val = String::from_utf8_lossy(&buf[pos..pos + len]).to_string();
                                    row.push(val);
                                    pos += len;
                                } else {
                                    row.push("NULL".to_string());
                                }
                            } else {
                                row.push("NULL".to_string());
                            }
                        }
                        rows.push(row);
                    } else {
                        let mut buf = vec![0u8; pkt_len as usize];
                        self.stream_read(&mut buf)?;
                        let mut row = Vec::with_capacity(columns.len());
                        let mut pos = 0;
                        for _ in 0..columns.len() {
                            if pos < buf.len() {
                                let len = buf[pos] as usize;
                                pos += 1;
                                if len < 251 {
                                    let val = String::from_utf8_lossy(&buf[pos..pos + len]).to_string();
                                    row.push(val);
                                    pos += len;
                                } else {
                                    row.push("NULL".to_string());
                                }
                            } else {
                                row.push("NULL".to_string());
                            }
                        }
                        rows.push(row);
                    }
                }
                Ok(MyResult::Rows(rows, columns))
            }
        }
    }

    fn read_column_definition(&mut self) -> Result<MyColumnInfo, crate::error::Error> {
        let pkt_len = self.read_u24()?;
        let _seq = self.read_u8()?;

        let mut buf = vec![0u8; pkt_len as usize];
        self.stream_read(&mut buf)?;

        let mut pos = 0;
        while pos < buf.len() && buf[pos] != b'c' { pos += 1; }
        pos += 1;
        let catalog_len = buf[pos] as usize;
        pos += 1 + catalog_len;
        let schema_len = buf[pos] as usize;
        pos += 1 + schema_len;
        let table_len = buf[pos] as usize;
        pos += 1 + table_len;
        let org_table_len = buf[pos] as usize;
        pos += 1 + org_table_len;
        let name_len = buf[pos] as usize;
        pos += 1;
        let name = String::from_utf8_lossy(&buf[pos..pos + name_len]).to_string();
        pos += name_len;
        let org_name_len = buf[pos] as usize;
        pos += 1 + org_name_len;
        pos += 1;
        let _charset = u16::from_le_bytes([buf[pos], buf[pos + 1]]);
        pos += 2;
        let _col_len = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        pos += 4;
        let data_type = buf[pos];
        pos += 1;
        let flags = u16::from_le_bytes([buf[pos], buf[pos + 1]]);
        pos += 2;
        let decimals = buf[pos];

        Ok(MyColumnInfo { name, data_type, flags, decimals })
    }

    fn read_eof(&mut self) -> Result<(), crate::error::Error> {
        let pkt_len = self.read_u24()?;
        let _seq = self.read_u8()?;
        self.skip(pkt_len as usize)?;
        Ok(())
    }

    fn send_command(&mut self, cmd: u8, arg: &str) -> Result<(), crate::error::Error> {
        let mut payload = vec![cmd];
        payload.extend_from_slice(arg.as_bytes());
        self.send_packet(0, &payload)?;
        Ok(())
    }

    fn send_packet(&mut self, seq: u8, payload: &[u8]) -> Result<(), crate::error::Error> {
        let len = payload.len() as u32;
        let mut header = Vec::new();
        header.extend_from_slice(&len.to_le_bytes()[..3]);
        header.push(seq);
        self.stream.write_all(&header)?;
        self.stream.write_all(payload)?;
        Ok(())
    }

    fn read_u8(&mut self) -> Result<u8, crate::error::Error> {
        let mut buf = [0u8; 1];
        self.stream.read(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16(&mut self) -> Result<u16, crate::error::Error> {
        let mut buf = [0u8; 2];
        self.stream.read(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24(&mut self) -> Result<u32, crate::error::Error> {
        let mut buf = [0u8; 3];
        self.stream.read(&mut buf)?;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32(&mut self) -> Result<u32, crate::error::Error> {
        let mut buf = [0u8; 4];
        self.stream.read(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_lenenc(&mut self) -> Result<u64, crate::error::Error> {
        let first = self.read_u8()?;
        self.read_lenenc_from_byte(first, &mut vec![0u8; 0])
    }

    fn read_lenenc_from_byte(&mut self, first: u8, _buf: &mut Vec<u8>) -> Result<u64, crate::error::Error> {
        match first {
            0xFB => Ok(0),
            0xFC => {
                let mut b = [0u8; 2];
                self.stream.read(&mut b)?;
                Ok(u16::from_le_bytes(b) as u64)
            }
            0xFD => {
                let mut b = [0u8; 3];
                self.stream.read(&mut b)?;
                Ok(u32::from_le_bytes([b[0], b[1], b[2], 0]) as u64)
            }
            0xFE => {
                let mut b = [0u8; 8];
                self.stream.read(&mut b)?;
                Ok(u64::from_le_bytes(b))
            }
            v => Ok(v as u64),
        }
    }

    fn stream_read(&mut self, buf: &mut [u8]) -> Result<(), crate::error::Error> {
        self.stream.read(buf)?;
        Ok(())
    }

    fn skip(&mut self, count: usize) -> Result<(), crate::error::Error> {
        let mut buf = vec![0u8; count];
        self.stream.read(&mut buf)?;
        Ok(())
    }
}